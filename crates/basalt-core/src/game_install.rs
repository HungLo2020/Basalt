//! Catalog-driven ZIP installation. Downloads are staged beside the destination; an existing
//! installation is retained until both the replacement and registry write succeed.
use crate::{
    CoreError, CoreResult, GameEntry, RunnerKind,
    catalog::GameDefinition,
    catalog_launch::{MANIFEST, validate_launch},
    progress::Progress,
};
use serde_json::Value;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::{
    fs,
    io::copy,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

pub struct GameInstallReport {
    pub game_name: String,
    pub release_tag: String,
    pub install_dir: PathBuf,
}

/// Install the latest matching release into a registered library. Existing persistent data
/// wins over the archive in its entirety; application files are replaced, not merged.
pub fn install_game(
    game_id: &str,
    library_id: &str,
    progress: &Progress,
) -> CoreResult<GameInstallReport> {
    let game = crate::catalog::find_game(game_id)?;
    let target = crate::libraries::install_directory(library_id, &game.id)?;
    let library = target
        .parent()
        .ok_or_else(|| CoreError::new("Missing library root"))?;
    fs::create_dir_all(library).map_err(|e| CoreError::io("Create library", library, e))?;
    let library = library
        .canonicalize()
        .map_err(|e| CoreError::io("Resolve library", library, e))?;
    let target = library.join(&game.id);
    if target
        .to_str()
        .is_none_or(|p| p.contains(['\t', '\n', '\r']))
    {
        return Err(CoreError::InvalidInput(
            "Library path cannot be represented in the game registry".into(),
        ));
    }
    progress.report_step(format!("Checking latest {} release", game.name));
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        game.source.repository
    );
    let mut response = ureq::get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Basalt-Installer")
        .call()
        .map_err(|e| CoreError::new(format!("Cannot fetch {} release: {e}", game.name)))?;
    let payload = response
        .body_mut()
        .read_to_string()
        .map_err(|e| CoreError::new(e.to_string()))?;
    let release: Value =
        serde_json::from_str(&payload).map_err(|e| CoreError::new(e.to_string()))?;
    let (tag, archive_url) = select_asset(&game, &release)?;
    let stage = Stage::new(&library)?;
    let archive = stage.0.join("release.zip");
    let mut file =
        fs::File::create(&archive).map_err(|e| CoreError::io("Create archive", &archive, e))?;
    crate::download::download_to_writer(
        &archive_url,
        "Basalt-Installer",
        &mut file,
        progress,
        &format!("Downloading {} {tag}", game.name),
    )?;
    drop(file);
    let extracted = stage.0.join("extracted");
    extract_archive(&archive, &extracted, progress)?;
    let source = payload_root(&extracted, &game)?;
    validate_launch(&source, &game)?;
    progress.check_cancelled()?;
    // Serializes competing installs and registry changes. Cancellation stops before commit.
    crate::storage::with_data_lock(|| {
        let mut entries = crate::registry::load_entries()?;
        let launch_target = target.join(MANIFEST).to_string_lossy().into_owned();
        if let Some(existing) = entries.iter().find(|entry| entry.name == game.name) {
            // Permit replacing the old bash entry only for this exact installation.
            let legacy = target
                .join(&game.launch.executable)
                .to_string_lossy()
                .into_owned();
            if !(existing.runner_kind == RunnerKind::Catalog
                && existing.launch_target == launch_target
                || existing.runner_kind == RunnerKind::Bash && existing.launch_target == legacy)
            {
                return Err(CoreError::InvalidInput(format!(
                    "{} is registered at another location. Remove its Basalt entry before installing elsewhere; files will be retained.",
                    game.name
                )));
            }
        }
        if crate::registry::load_blacklisted_names()?.contains(&game.name.to_lowercase()) {
            return Err(CoreError::Blacklisted(game.name.clone()));
        }
        entries.retain(|entry| entry.name != game.name);
        entries.push(GameEntry {
            name: game.name.clone(),
            runner_kind: RunnerKind::Catalog,
            launch_target,
        });
        progress.report_step(format!("Installing {} {tag}", game.name));
        commit_payload(
            &source,
            &target,
            &stage.0.join("previous"),
            &game,
            progress,
            || crate::registry::save_entries(&entries),
        )?;
        if let Err(error) = crate::playlist_service::sync_automatic_playlists() {
            crate::warnings::warn(format!(
                "Game installed, but playlist refresh failed: {error}"
            ));
        }
        Ok(())
    })?;
    Ok(GameInstallReport {
        game_name: game.name,
        release_tag: tag,
        install_dir: target,
    })
}

fn select_asset(game: &GameDefinition, release: &Value) -> CoreResult<(String, String)> {
    let tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| CoreError::new("Release has no tag_name"))?;
    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| CoreError::new("Release has no assets"))?;
    let matches: Vec<_> = assets
        .iter()
        .filter(|asset| {
            let name = asset["name"].as_str().unwrap_or("");
            name.starts_with(&game.source.asset_prefix)
                && name.ends_with(&game.source.asset_suffix)
                && game
                    .source
                    .asset_contains
                    .iter()
                    .all(|part| name.contains(part))
        })
        .collect();
    if matches.len() != 1 {
        return Err(CoreError::new(format!(
            "Expected one release asset for {}, found {}. Check the catalog filename/platform pattern.",
            game.name,
            matches.len()
        )));
    }
    let url = matches[0]["browser_download_url"]
        .as_str()
        .filter(|url| url.starts_with("https://"))
        .ok_or_else(|| CoreError::new("Release asset has no HTTPS download URL"))?;
    Ok((tag.into(), url.into()))
}

struct Stage(PathBuf);
impl Stage {
    fn new(library: &Path) -> CoreResult<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| CoreError::new(e.to_string()))?
            .as_nanos();
        let path = library.join(format!(".basalt-install-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).map_err(|e| CoreError::io("Create staging directory", &path, e))?;
        Ok(Self(path))
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        // If rollback failed, keep the old installation for manual recovery.
        if self.0.join("previous").exists() {
            crate::warnings::warn(format!(
                "Previous installation retained at {}",
                self.0.join("previous").display()
            ));
        } else if let Err(error) = fs::remove_dir_all(&self.0) {
            crate::warnings::warn(format!(
                "Could not clean staging directory {}: {error}",
                self.0.display()
            ));
        }
    }
}

fn extract_archive(archive: &Path, root: &Path, progress: &Progress) -> CoreResult<()> {
    fs::create_dir(root).map_err(|e| CoreError::io("Create extraction directory", root, e))?;
    let file = fs::File::open(archive).map_err(|e| CoreError::io("Open archive", archive, e))?;
    let mut zip = ZipArchive::new(file).map_err(|e| CoreError::new(e.to_string()))?;
    let count = zip.len() as u64;
    for index in 0..zip.len() {
        progress.check_cancelled()?;
        progress.report_items("Extracting game", index as u64, count);
        let mut entry = zip
            .by_index(index)
            .map_err(|e| CoreError::new(e.to_string()))?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| CoreError::new("ZIP entry escapes extraction directory"))?;
        let output = root.join(relative);
        let mode = entry.unix_mode().unwrap_or(0o644);
        // Never materialize archive symlinks or special files.
        let kind = mode & 0o170000;
        if kind != 0 && kind != 0o100000 && kind != 0o040000 {
            return Err(CoreError::new(
                "Release ZIP contains an unsupported symlink or special file",
            ));
        }
        if entry.is_dir() {
            fs::create_dir_all(&output)
                .map_err(|e| CoreError::io("Create extracted directory", &output, e))?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| CoreError::io("Create extraction parent", parent, e))?;
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)
                .map_err(|e| CoreError::io("Create extracted file", &output, e))?;
            copy(&mut entry, &mut file).map_err(|e| CoreError::io("Extract file", &output, e))?;
            fs::set_permissions(&output, fs::Permissions::from_mode(mode & 0o777))
                .map_err(|e| CoreError::io("Set executable permissions", &output, e))?;
        }
    }
    Ok(())
}

fn payload_root(extracted: &Path, game: &GameDefinition) -> CoreResult<PathBuf> {
    if extracted.join(&game.launch.executable).is_file() {
        return Ok(extracted.into());
    }
    let entries: Vec<_> = fs::read_dir(extracted)
        .map_err(|e| CoreError::io("Read extraction", extracted, e))?
        .collect::<Result<_, _>>()
        .map_err(|e| CoreError::new(e.to_string()))?;
    if entries.len() == 1
        && entries[0].path().is_dir()
        && entries[0].path().join(&game.launch.executable).is_file()
    {
        return Ok(entries[0].path());
    }
    Err(CoreError::new(
        "Release ZIP does not contain the catalog launch executable",
    ))
}

fn commit_payload(
    source: &Path,
    target: &Path,
    backup: &Path,
    game: &GameDefinition,
    progress: &Progress,
    register: impl FnOnce() -> CoreResult<()>,
) -> CoreResult<()> {
    if let Ok(metadata) = fs::symlink_metadata(target)
        && (!metadata.is_dir() || metadata.file_type().is_symlink())
    {
        return Err(CoreError::new(
            "Installation destination must be a real directory",
        ));
    }
    let persistent = target.join(&game.persistent_directory);
    if fs::symlink_metadata(&persistent).is_ok() {
        // Refuse a symlink at the persistent root; nested user symlinks are copied, never followed.
        if !fs::symlink_metadata(&persistent)
            .map_err(|e| CoreError::io("Inspect game data", &persistent, e))?
            .is_dir()
        {
            return Err(CoreError::new(
                "Persistent data root must be a real directory",
            ));
        }
        let staged = source.join(&game.persistent_directory);
        if staged.exists() {
            fs::remove_dir_all(&staged)
                .map_err(|e| CoreError::io("Replace staged data", &staged, e))?;
        }
        copy_data(&persistent, &staged, progress)?;
    }
    validate_launch(source, game)?;
    let manifest = serde_json::to_vec_pretty(game).map_err(|e| CoreError::new(e.to_string()))?;
    fs::write(source.join(MANIFEST), manifest)
        .map_err(|e| CoreError::io("Write installed manifest", source, e))?;
    progress.check_cancelled()?;
    let existed = target.exists();
    if existed {
        fs::rename(target, backup)
            .map_err(|e| CoreError::io("Retain previous installation", target, e))?;
    }
    if let Err(error) = fs::rename(source, target) {
        if existed {
            fs::rename(backup, target)
                .map_err(|e| CoreError::io("Restore previous installation", backup, e))?;
        }
        return Err(CoreError::io("Activate installation", target, error));
    }
    if let Err(error) = register() {
        fs::rename(target, source)
            .map_err(|e| CoreError::io("Withdraw failed installation", target, e))?;
        if existed {
            fs::rename(backup, target)
                .map_err(|e| CoreError::io("Restore previous installation", backup, e))?;
        }
        return Err(error);
    }
    if existed && let Err(error) = fs::remove_dir_all(backup) {
        crate::warnings::warn(format!(
            "Installed successfully; could not remove old copy {}: {error}",
            backup.display()
        ));
    }
    Ok(())
}

fn copy_data(source: &Path, target: &Path, progress: &Progress) -> CoreResult<()> {
    progress.check_cancelled()?;
    let meta = fs::symlink_metadata(source)
        .map_err(|e| CoreError::io("Inspect persistent data", source, e))?;
    if meta.file_type().is_symlink() {
        let link = fs::read_link(source).map_err(|e| CoreError::io("Read data link", source, e))?;
        symlink(link, target).map_err(|e| CoreError::io("Preserve data link", target, e))?;
    } else if meta.is_dir() {
        fs::create_dir_all(target)
            .map_err(|e| CoreError::io("Create data directory", target, e))?;
        for entry in
            fs::read_dir(source).map_err(|e| CoreError::io("Read persistent data", source, e))?
        {
            let entry = entry.map_err(|e| CoreError::new(e.to_string()))?;
            copy_data(&entry.path(), &target.join(entry.file_name()), progress)?;
        }
        fs::set_permissions(target, meta.permissions())
            .map_err(|e| CoreError::io("Preserve data permissions", target, e))?;
    } else if meta.is_file() {
        fs::copy(source, target).map_err(|e| CoreError::io("Preserve data file", source, e))?;
    } else {
        return Err(CoreError::new(
            "Persistent data contains an unsupported special file",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn write(path: &Path, value: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    fn fixture() -> (Stage, GameDefinition, PathBuf, PathBuf) {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = crate::catalog::find_game("MattMC").unwrap();
        let source = stage.0.join("release");
        let target = stage.0.join("installed");
        write(&source.join("run-mattmc.sh"), "new");
        write(&source.join("lib/new.jar"), "new jar");
        write(&source.join("run/settings.txt"), "release defaults");
        (stage, game, source, target)
    }
    #[test]
    fn fresh_install_seeds_data_and_registers_catalog_target() {
        let (stage, game, source, target) = fixture();
        commit_payload(
            &source,
            &target,
            &stage.0.join("previous"),
            &game,
            &Progress::none(),
            || {
                assert!(target.join(MANIFEST).is_file());
                assert!(target.join("run-mattmc.sh").is_file());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(target.join("run/settings.txt")).unwrap(),
            "release defaults"
        );
    }
    #[test]
    fn reinstall_replaces_application_and_preserves_all_existing_run_data() {
        let (stage, game, source, target) = fixture();
        write(&target.join("lib/obsolete.jar"), "old");
        write(&target.join("run/settings.txt"), "user settings");
        write(&target.join("run/server/world/level.dat"), "world");
        write(&target.join("run/jdk/bin/java"), "existing runtime");
        symlink("settings.txt", target.join("run/settings-link")).unwrap();
        commit_payload(
            &source,
            &target,
            &stage.0.join("previous"),
            &game,
            &Progress::none(),
            || Ok(()),
        )
        .unwrap();
        assert!(!target.join("lib/obsolete.jar").exists());
        assert!(target.join("lib/new.jar").exists());
        assert_eq!(
            fs::read_to_string(target.join("run/settings.txt")).unwrap(),
            "user settings"
        );
        assert_eq!(
            fs::read_to_string(target.join("run/server/world/level.dat")).unwrap(),
            "world"
        );
        assert_eq!(
            fs::read_to_string(target.join("run/jdk/bin/java")).unwrap(),
            "existing runtime"
        );
        assert_eq!(
            fs::read_link(target.join("run/settings-link")).unwrap(),
            Path::new("settings.txt")
        );
        assert!(!stage.0.join("previous").exists());
    }
    #[test]
    fn registry_failure_restores_previous_installation() {
        let (stage, game, source, target) = fixture();
        write(&target.join("run-mattmc.sh"), "old");
        write(&target.join("run/settings.txt"), "user settings");
        assert!(
            commit_payload(
                &source,
                &target,
                &stage.0.join("previous"),
                &game,
                &Progress::none(),
                || Err(CoreError::new("injected registry failure"))
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(target.join("run-mattmc.sh")).unwrap(),
            "old"
        );
        assert_eq!(
            fs::read_to_string(target.join("run/settings.txt")).unwrap(),
            "user settings"
        );
    }
    #[test]
    fn cancellation_does_not_touch_existing_installation() {
        let (stage, game, source, target) = fixture();
        write(&target.join("run-mattmc.sh"), "old");
        let cancel = crate::CancelToken::new();
        cancel.cancel();
        let progress = Progress::new(|_| {}, cancel);
        assert!(matches!(
            commit_payload(
                &source,
                &target,
                &stage.0.join("previous"),
                &game,
                &progress,
                || panic!("must not register cancelled install")
            ),
            Err(CoreError::Cancelled)
        ));
        assert_eq!(
            fs::read_to_string(target.join("run-mattmc.sh")).unwrap(),
            "old"
        );
    }
    #[test]
    fn archive_validation_rejects_traversal_and_preserves_executable_bits() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let archive = stage.0.join("valid.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        writer
            .start_file(
                "app/bin/game",
                zip::write::SimpleFileOptions::default().unix_permissions(0o755),
            )
            .unwrap();
        writer.write_all(b"#!/bin/sh\nexit 0\n").unwrap();
        writer.finish().unwrap();
        let root = stage.0.join("valid");
        extract_archive(&archive, &root, &Progress::none()).unwrap();
        assert_eq!(
            fs::metadata(root.join("app/bin/game"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        let archive = stage.0.join("invalid.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        writer
            .start_file("../escape", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bad").unwrap();
        writer.finish().unwrap();
        assert!(extract_archive(&archive, &stage.0.join("invalid"), &Progress::none()).is_err());
        assert!(!stage.0.join("escape").exists());
    }
    #[test]
    fn release_selection_filters_platform_and_rejects_ambiguity() {
        let game = crate::catalog::find_game("MattMC").unwrap();
        let asset = |name| serde_json::json!({"name": name, "browser_download_url": "https://github.com/example/game.zip"});
        let release = serde_json::json!({"tag_name": "v1", "assets": [
            asset("MattMC-Client-v1-windows-x64.zip"), asset("MattMC-Client-v1-linux-x64.zip")]});
        assert_eq!(select_asset(&game, &release).unwrap().0, "v1");
        let mut ambiguous = release.clone();
        ambiguous["assets"]
            .as_array_mut()
            .unwrap()
            .push(asset("MattMC-Client-v2-linux-x64.zip"));
        assert!(select_asset(&game, &ambiguous).is_err());
        let missing = serde_json::json!({"tag_name": "v1", "assets": []});
        assert!(select_asset(&game, &missing).is_err());
    }
    #[test]
    fn payload_can_be_flat_or_wrapped_without_stripping_app_directory() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = crate::catalog::find_game("Jackal").unwrap();
        let flat = stage.0.join("flat");
        write(&flat.join("app/bin/jackal"), "binary");
        assert_eq!(payload_root(&flat, &game).unwrap(), flat);
        let wrapped = stage.0.join("wrapped");
        write(&wrapped.join("Jackal-v1/app/bin/jackal"), "binary");
        assert_eq!(
            payload_root(&wrapped, &game).unwrap(),
            wrapped.join("Jackal-v1")
        );
    }
}
