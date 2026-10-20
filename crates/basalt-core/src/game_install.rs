//! Catalog-driven ZIP installation. Downloads are staged beside the destination; an existing
//! installation is retained until both the replacement and registry write succeed.
use crate::{
    CoreError, CoreResult, GameEntry, RunnerKind,
    catalog::GameDefinition,
    catalog_launch::{MANIFEST, validate_launch},
    game_data::{self, Installation, OperationGuard},
    game_layout::{DataCategory, GameLayout},
    progress::Progress,
};
use serde_json::Value;
use ureq::ResponseExt;
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

/// Install the latest matching release into a registered library. Existing user data wins
/// over the archive; application files are replaced, not merged. For schema 1 entries the user
/// data is the persistent directory; for schema 2 it is everything outside the layout's `app`
/// paths, and it is backed up first.
pub fn install_game(
    game_id: &str,
    library_id: &str,
    progress: &Progress,
) -> CoreResult<GameInstallReport> {
    let game = crate::catalog::find_game(game_id)?;
    let target = crate::libraries::install_directory(library_id, &game.id)?;
    let library = target
        .parent()
        .ok_or_else(|| CoreError::new("Missing library root"))?
        .to_path_buf();
    // Reinstalling a standard game over itself is an update: same guards and backup.
    let existing = game.follows_standard() && target.join(MANIFEST).is_file();
    let _guard = if existing {
        let installation = Installation {
            definition: game.clone(),
            root: target.canonicalize().map_err(|e| CoreError::io("Resolve installation", &target, e))?,
        };
        Some(OperationGuard::acquire(&installation, true)?)
    } else {
        None
    };
    install_into(&game, &library, existing, progress)
}

/// The built-in `update` action: reinstall the latest release into the library the game is
/// already in. The caller holds the installation's operation guard.
pub(crate) fn update_installation(
    installation: &Installation,
    backup_first: bool,
    progress: &Progress,
) -> CoreResult<GameInstallReport> {
    let game = &installation.definition;
    let library = installation
        .root
        .parent()
        .ok_or_else(|| CoreError::new("Installation has no library"))?;
    if installation.root.file_name() != Some(std::ffi::OsStr::new(&game.id)) {
        return Err(CoreError::new(format!(
            "{} is installed at {}, not in a folder named {}; reinstall it from the Install page",
            game.name,
            installation.root.display(),
            game.id
        )));
    }
    install_into(game, library, backup_first, progress)
}

fn install_into(
    game: &GameDefinition,
    library: &Path,
    backup_first: bool,
    progress: &Progress,
) -> CoreResult<GameInstallReport> {
    let game = game.clone();
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
    let release = fetch_latest_release(&game, progress)?;
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
    let source = prepare_release(&archive, &stage.0.join("extracted"), &game, progress)?;
    progress.check_cancelled()?;
    if backup_first && game.follows_standard() && target.join(MANIFEST).is_file() {
        progress.report_step(format!("Backing up {} before updating", game.name));
        let (categories, keep) = crate::maintenance::backup_settings(&game);
        let installation = Installation {
            definition: game.clone(),
            root: target.clone(),
        };
        game_data::backup_installation(&installation, &categories, keep, "before-update", progress)?;
    }
    // Serializes competing installs and registry changes. Cancellation stops before commit.
    crate::storage::with_data_lock(|| {
        let mut entries = crate::registry::load_entries()?;
        let launch_target = target.join(MANIFEST).to_string_lossy().into_owned();
        if let Some(existing) = entries.iter().find(|entry| entry.name == game.name) {
            // Permit replacing an old bash entry only for this exact installation: its script is
            // the catalog executable, or (for a game whose layout changed) a script directly in
            // the installation folder, such as MattMC's run-mattmc.sh.
            let legacy = target
                .join(&game.launch.executable)
                .to_string_lossy()
                .into_owned();
            let bash_in_target = Path::new(&existing.launch_target).parent() == Some(&target);
            if !(existing.runner_kind == RunnerKind::Catalog
                && existing.launch_target == launch_target
                || existing.runner_kind == RunnerKind::Bash
                    && (existing.launch_target == legacy || bash_in_target))
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

fn fetch_latest_release(game: &GameDefinition, progress: &Progress) -> CoreResult<Value> {
    fetch_latest_release_at(
        game,
        progress,
        "https://api.github.com",
        "https://github.com",
    )
    .map_err(|e| match e {
        CoreError::Cancelled => e,
        _ => CoreError::new(format!("Cannot fetch {} release: {e}", game.name)),
    })
}

fn fetch_latest_release_at(
    game: &GameDefinition,
    progress: &Progress,
    api: &str,
    web: &str,
) -> CoreResult<Value> {
    progress.check_cancelled()?;
    let mut response = ureq::get(format!(
        "{api}/repos/{}/releases/latest",
        game.source.repository
    ))
    .header("Accept", "application/vnd.github+json")
    .header("User-Agent", "Basalt-Installer")
    .config()
    .http_status_as_error(false)
    .build()
    .call()
    .map_err(|e| CoreError::new(e.to_string()))?;
    let status = response.status();
    let limited = response
        .headers()
        .get("x-ratelimit-remaining")
        .is_some_and(|v| v == "0")
        || response.headers().contains_key("retry-after");
    let payload = response
        .body_mut()
        .read_to_string()
        .map_err(|e| CoreError::new(e.to_string()))?;
    if status.is_success() {
        return serde_json::from_str(&payload).map_err(|e| CoreError::new(e.to_string()));
    }
    let message = serde_json::from_str::<Value>(&payload)
        .ok()
        .and_then(|v| v["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| status.to_string());
    if !matches!(status.as_u16(), 403 | 429)
        || !(limited || message.to_ascii_lowercase().contains("rate limit"))
    {
        return Err(CoreError::new(format!("GitHub HTTP {status}: {message}")));
    }

    // Public release pages and asset downloads do not use the REST API quota. Resolve
    // GitHub's latest redirect, then read its asset fragment using the same strict selector.
    progress.report_step("GitHub API rate limit reached; checking public release page");
    progress.check_cancelled()?;
    let repo = format!("{web}/{}", game.source.repository);
    let response = ureq::get(format!("{repo}/releases/latest"))
        .header("User-Agent", "Basalt-Installer")
        .call()
        .map_err(|e| {
            CoreError::new(format!(
                "GitHub API rate limit reached; public release lookup failed: {e}"
            ))
        })?;
    let final_url = response.get_uri().to_string();
    let tag = final_url
        .strip_prefix(&format!("{repo}/releases/tag/"))
        .filter(|tag| !tag.is_empty() && !tag.contains(['?', '#']))
        .ok_or_else(|| CoreError::new("GitHub latest release did not resolve to a release tag"))?;
    progress.check_cancelled()?;
    let html = ureq::get(format!("{repo}/releases/expanded_assets/{tag}"))
        .header("User-Agent", "Basalt-Installer")
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .map_err(|e| {
            CoreError::new(format!(
                "GitHub API rate limit reached; public release assets failed: {e}"
            ))
        })?;
    release_from_html(&game.source.repository, tag, &html, web)
}

fn release_from_html(repository: &str, tag: &str, html: &str, web: &str) -> CoreResult<Value> {
    let prefix = format!("/{repository}/releases/download/{tag}/");
    let mut assets = Vec::new();
    for attribute in html.split("href=").skip(1) {
        let Some(quote @ ('\"' | '\'')) = attribute.chars().next() else {
            continue;
        };
        let Some(href) = attribute[1..].split(quote).next() else {
            continue;
        };
        let Some(name) = href.strip_prefix(&prefix) else {
            continue;
        };
        if name.is_empty() || name.contains(['/', '?', '#']) {
            continue;
        }
        let name = percent_encoding::percent_decode_str(name)
            .decode_utf8()
            .map_err(|e| CoreError::new(format!("Invalid GitHub asset name: {e}")))?;
        let asset =
            serde_json::json!({"name": name, "browser_download_url": format!("{web}{href}")});
        if !assets.contains(&asset) {
            assets.push(asset);
        }
    }
    let tag = percent_encoding::percent_decode_str(tag)
        .decode_utf8()
        .map_err(|e| CoreError::new(format!("Invalid GitHub release tag: {e}")))?;
    Ok(serde_json::json!({"tag_name": tag, "assets": assets}))
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

/// Unpacks a downloaded release and returns its validated release root, before anything in the
/// installation is touched.
fn prepare_release(
    archive: &Path,
    extracted: &Path,
    game: &GameDefinition,
    progress: &Progress,
) -> CoreResult<PathBuf> {
    extract_archive(archive, extracted, progress)?;
    let source = if game.follows_standard() {
        let source = standard_release_root(extracted, game)?;
        validate_standard_payload(&source, &GameLayout::for_game(game))?;
        source
    } else {
        payload_root(extracted, game)?
    };
    validate_launch(&source, game)?;
    Ok(source)
}

/// Schema 2: the archive root is the release root, unless `source.archive_root` names the one
/// folder the archive must consist of.
fn standard_release_root(extracted: &Path, game: &GameDefinition) -> CoreResult<PathBuf> {
    let mut entries: Vec<(String, bool)> = fs::read_dir(extracted)
        .map_err(|e| CoreError::io("Read extraction", extracted, e))?
        .map(|entry| {
            let entry = entry.map_err(|e| CoreError::io("Read extraction", extracted, e))?;
            let is_dir = fs::symlink_metadata(entry.path())
                .map_err(|e| CoreError::io("Inspect extraction", entry.path(), e))?
                .is_dir();
            Ok((entry.file_name().to_string_lossy().into_owned(), is_dir))
        })
        .collect::<CoreResult<_>>()?;
    entries.sort();
    let listing = || {
        entries
            .iter()
            .map(|(name, is_dir)| if *is_dir { format!("{name}/") } else { name.clone() })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let Some(wrapper) = &game.source.archive_root else {
        if let [(name, true)] = entries.as_slice()
            && name != "app"
            && extracted.join(name).join("app").is_dir()
        {
            return Err(CoreError::new(format!(
                "Release wraps its files in {name}/, but the catalog entry does not set \
                 source.archive_root"
            )));
        }
        return Ok(extracted.to_path_buf());
    };
    match entries.as_slice() {
        [(name, true)] if name == wrapper => Ok(extracted.join(wrapper)),
        [] => Err(CoreError::new("Release archive is empty")),
        _ if entries.iter().any(|(name, _)| name == "app") => Err(CoreError::new(format!(
            "Release is not wrapped in a {wrapper}/ folder (source.archive_root); its top level \
             contains {}",
            listing()
        ))),
        _ => Err(CoreError::new(format!(
            "Release must contain only the folder {wrapper}/ at its top level, but contains {}",
            listing()
        ))),
    }
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
    if game.follows_standard() {
        preserve_user_data(source, target, &GameLayout::for_game(game), progress)?;
    } else {
        preserve_legacy_data(source, target, game.legacy_persistent_directory(), progress)?;
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

/// Schema 1: the existing persistent directory replaces whatever the release shipped there.
fn preserve_legacy_data(
    source: &Path,
    target: &Path,
    persistent_directory: &str,
    progress: &Progress,
) -> CoreResult<()> {
    let persistent = target.join(persistent_directory);
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
        let staged = source.join(persistent_directory);
        if staged.exists() {
            fs::remove_dir_all(&staged)
                .map_err(|e| CoreError::io("Replace staged data", &staged, e))?;
        }
        copy_data(&persistent, &staged, progress)?;
    }
    Ok(())
}

/// Schema 2: everything in the existing installation outside the `app` paths carries over
/// unchanged (user data, uncategorized files and Basalt's `.basalt/` state), and the standard
/// data folders exist afterwards.
fn preserve_user_data(
    source: &Path,
    target: &Path,
    layout: &GameLayout,
    progress: &Progress,
) -> CoreResult<()> {
    if target.is_dir() {
        for relative in layout.non_app_entries(target)? {
            let staged = source.join(&relative);
            if let Some(parent) = staged.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| CoreError::io("Create data directory", parent, e))?;
            }
            copy_data(&target.join(&relative), &staged, progress)?;
        }
    }
    for category in [
        DataCategory::Save,
        DataCategory::SharedConfig,
        DataCategory::Config,
        DataCategory::Cache,
    ] {
        if let Some(path) = layout.primary(category) {
            let directory = source.join(path);
            if fs::symlink_metadata(&directory).is_err() {
                fs::create_dir_all(&directory)
                    .map_err(|e| CoreError::io("Create data directory", &directory, e))?;
            }
        }
    }
    Ok(())
}

/// A standard release contains only application paths, so installing it can never overwrite
/// user data.
fn validate_standard_payload(source: &Path, layout: &GameLayout) -> CoreResult<()> {
    for reserved in ["data", crate::game_layout::BASALT_DIR] {
        if fs::symlink_metadata(source.join(reserved)).is_ok() {
            return Err(CoreError::new(format!(
                "Release contains {reserved}/, which is reserved for user data and Basalt; \
                 releases may only ship application files"
            )));
        }
    }
    fn visit(root: &Path, relative: &Path, layout: &GameLayout) -> CoreResult<()> {
        let path = root.join(relative);
        let metadata =
            fs::symlink_metadata(&path).map_err(|e| CoreError::io("Inspect release", &path, e))?;
        if !relative.as_os_str().is_empty()
            && layout.category_of(relative) == Some(DataCategory::App)
        {
            return Ok(());
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(|e| CoreError::io("Read release", &path, e))? {
                let entry = entry.map_err(|e| CoreError::io("Read release", &path, e))?;
                visit(root, &relative.join(entry.file_name()), layout)?;
            }
            Ok(())
        } else {
            Err(CoreError::new(format!(
                "Release contains {}, which is outside the game's app folder. Releases may only \
                 ship application files (see STANDARDS.md).",
                relative.display()
            )))
        }
    }
    visit(source, Path::new(""), layout)
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
        // Unchanged timestamps keep Basalt's file index (and so sync and backup) fast.
        if let Ok(modified) = meta.modified()
            && let Ok(file) = fs::File::options().write(true).open(target)
        {
            let _ = file.set_modified(modified);
        }
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
    #[test]
    fn public_release_assets_keep_platform_filter_and_reject_ambiguity() {
        let game = crate::find_game("MicroMages").unwrap();
        let html = r#"
            <a href="/HungLo2020/MicroMages/releases/download/v1%2Fstable/MicroMages-v1-linux-x64.zip">Linux</a>
            <a href="/HungLo2020/MicroMages/releases/download/v1%2Fstable/MicroMages-v1-linux-x64.zip">Duplicate link</a>
            <a href='/HungLo2020/MicroMages/releases/download/v1%2Fstable/MicroMages-v1-windows-x64.zip'>Windows</a>
            <a href="/HungLo2020/Jackal/releases/download/v1%2Fstable/MicroMages-other-linux-x64.zip">Wrong repo</a>
            <a href="/HungLo2020/MicroMages/releases/download/old/MicroMages-old-linux-x64.zip">Old tag</a>
            <a href="https://example.com/MicroMages-other-linux-x64.zip">External</a>
        "#;
        let release = release_from_html(
            &game.source.repository,
            "v1%2Fstable",
            html,
            "https://github.com",
        )
        .unwrap();
        let (tag, url) = select_asset(&game, &release).unwrap();
        assert_eq!(tag, "v1/stable");
        assert_eq!(
            url,
            "https://github.com/HungLo2020/MicroMages/releases/download/v1%2Fstable/MicroMages-v1-linux-x64.zip"
        );
        let ambiguous = format!(
            "{html}<a href=\"/HungLo2020/MicroMages/releases/download/v1%2Fstable/MicroMages-other-linux-x64.zip\">"
        );
        let release = release_from_html(
            &game.source.repository,
            "v1%2Fstable",
            &ambiguous,
            "https://github.com",
        )
        .unwrap();
        assert!(select_asset(&game, &release).is_err());
        let empty = release_from_html(
            &game.source.repository,
            "v1",
            "Unexpected page",
            "https://github.com",
        )
        .unwrap();
        assert!(select_asset(&game, &empty).is_err());
    }

    #[test]
    fn rate_limited_api_falls_back_but_other_errors_do_not() {
        use std::io::{BufRead, BufReader};
        use std::net::TcpListener;
        use std::thread;
        for status in [403, 429, 404] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let server = thread::spawn(move || {
                let requests = if status == 404 { 1 } else { 4 };
                for index in 0..requests {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                        .unwrap();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let path = line.split_whitespace().nth(1).unwrap().to_owned();
                    loop {
                        line.clear();
                        reader.read_line(&mut line).unwrap();
                        if line == "\r\n" {
                            break;
                        }
                    }
                    let (code, headers, body) = match index {
                        0 => {
                            assert_eq!(path, "/repos/HungLo2020/MicroMages/releases/latest");
                            (
                                status,
                                "X-RateLimit-Remaining: 0\r\n",
                                r#"{"message":"Test API error"}"#,
                            )
                        }
                        1 => {
                            assert_eq!(path, "/HungLo2020/MicroMages/releases/latest");
                            (
                                302,
                                "Location: /HungLo2020/MicroMages/releases/tag/v2\r\n",
                                "",
                            )
                        }
                        2 => {
                            assert_eq!(path, "/HungLo2020/MicroMages/releases/tag/v2");
                            (200, "", "Release page")
                        }
                        _ => {
                            assert_eq!(path, "/HungLo2020/MicroMages/releases/expanded_assets/v2");
                            (
                                200,
                                "",
                                r#"<a href="/HungLo2020/MicroMages/releases/download/v2/MicroMages-v2-linux-x64.zip">Linux</a>"#,
                            )
                        }
                    };
                    write!(stream, "HTTP/1.1 {code} Test\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                }
            });
            let game = crate::find_game("MicroMages").unwrap();
            let result = fetch_latest_release_at(&game, &Progress::none(), &base, &base);
            if status == 404 {
                assert!(result.unwrap_err().to_string().contains("Test API error"));
            } else {
                let release = result.unwrap();
                assert_eq!(release["tag_name"], "v2");
                assert_eq!(release["assets"][0]["name"], "MicroMages-v2-linux-x64.zip");
            }
            server.join().unwrap();
        }
    }

    #[test]
    #[ignore = "queries live GitHub release metadata without installing anything"]
    fn live_public_release_lookup() {
        for id in ["MicroMages", "Jackal"] {
            let game = crate::find_game(id).unwrap();
            let release = fetch_latest_release(&game, &Progress::none()).unwrap();
            let (tag, url) = select_asset(&game, &release).unwrap();
            println!("{id}: {tag} {url}");
        }
    }

    fn write(path: &Path, value: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    fn fixture() -> (Stage, GameDefinition, PathBuf, PathBuf) {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = crate::catalog::legacy_mattmc();
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
        let game = crate::catalog::legacy_mattmc();
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
    #[test]
    fn standard_reinstall_replaces_app_and_keeps_everything_else() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = crate::catalog::standard_game(serde_json::json!({}));
        let source = stage.0.join("release");
        let target = stage.0.join("installed");
        write(&source.join("app/bin/sample"), "new binary");
        write(&source.join("app/defaults/settings.ini"), "defaults");
        write(&target.join("app/bin/sample"), "old binary");
        write(&target.join("app/obsolete.so"), "old");
        write(&target.join("data/save/slot1"), "progress");
        write(&target.join("data/config/video.ini"), "1080p");
        write(&target.join(".basalt/sync-state.json"), "{}");
        write(&target.join("notes.txt"), "uncategorized");
        commit_payload(
            &source,
            &target,
            &stage.0.join("previous"),
            &game,
            &Progress::none(),
            || Ok(()),
        )
        .unwrap();
        let read = |path: &str| fs::read_to_string(target.join(path)).unwrap();
        assert_eq!(read("app/bin/sample"), "new binary");
        assert!(!target.join("app/obsolete.so").exists());
        assert_eq!(read("data/save/slot1"), "progress");
        assert_eq!(read("data/config/video.ini"), "1080p");
        assert_eq!(read(".basalt/sync-state.json"), "{}");
        assert_eq!(read("notes.txt"), "uncategorized");
        assert!(target.join("data/config/shared").is_dir());
        assert!(target.join("data/cache").is_dir());
    }
    #[test]
    fn standard_fresh_install_creates_data_folders() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = crate::catalog::standard_game(serde_json::json!({}));
        let source = stage.0.join("release");
        let target = stage.0.join("installed");
        write(&source.join("app/bin/sample"), "binary");
        commit_payload(
            &source,
            &target,
            &stage.0.join("previous"),
            &game,
            &Progress::none(),
            || Ok(()),
        )
        .unwrap();
        for folder in ["data/save", "data/config", "data/config/shared", "data/cache"] {
            assert!(target.join(folder).is_dir(), "{folder}");
        }
    }
    #[test]
    fn standard_releases_may_only_ship_app_paths() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = crate::catalog::standard_game(serde_json::json!({}));
        let layout = GameLayout::for_game(&game);
        let release = stage.0.join("release");
        write(&release.join("app/bin/sample"), "binary");
        validate_standard_payload(&release, &layout).unwrap();
        write(&release.join("data/save/default.sav"), "would overwrite saves");
        assert!(validate_standard_payload(&release, &layout).is_err());
        let custom = crate::catalog::standard_game(
            serde_json::json!({"layout": {"app": ["app", "run-sample.sh"]}}),
        );
        let release = stage.0.join("custom");
        write(&release.join("app/bin/sample"), "binary");
        write(&release.join("run-sample.sh"), "launcher");
        validate_standard_payload(&release, &GameLayout::for_game(&custom)).unwrap();
    }
    /// The bundled MattMC entry (schema 2, wrapped in MattMC/).
    fn mattmc() -> GameDefinition {
        serde_json::from_str(include_str!("../../../resources/games/mattmc.json")).unwrap()
    }
    /// Builds a zip from (path, contents) pairs; entries ending in '/' are directories.
    fn zip(stage: &Stage, name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let archive = stage.0.join(format!("{name}.zip"));
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        for (path, contents) in entries {
            if path.ends_with('/') {
                writer
                    .add_directory(*path, zip::write::SimpleFileOptions::default())
                    .unwrap();
            } else {
                writer
                    .start_file(
                        *path,
                        zip::write::SimpleFileOptions::default().unix_permissions(0o755),
                    )
                    .unwrap();
                writer.write_all(contents.as_bytes()).unwrap();
            }
        }
        writer.finish().unwrap();
        archive
    }
    /// Unpacks and validates `entries` as a release, then installs it over `target`.
    fn install_release(
        stage: &Stage,
        game: &GameDefinition,
        name: &str,
        entries: &[(&str, &str)],
        target: &Path,
    ) -> CoreResult<()> {
        // install_into creates the library before this point.
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let archive = zip(stage, name, entries);
        let source = prepare_release(
            &archive,
            &stage.0.join(format!("extracted-{name}")),
            game,
            &Progress::none(),
        )?;
        commit_payload(
            &source,
            target,
            &stage.0.join(format!("previous-{name}")),
            game,
            &Progress::none(),
            || Ok(()),
        )
    }
    const MATTMC_V2: &[(&str, &str)] = &[
        ("MattMC/app/bin/mattmc", "launcher v2"),
        ("MattMC/app/lib/client.jar", "jar v2"),
        ("MattMC/app/scripts/backup.sh", "standalone script"),
    ];
    #[test]
    fn wrapped_archive_installs_the_wrapper_contents_as_the_install_root() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let target = stage.0.join("Games/MattMC");
        install_release(&stage, &mattmc(), "v2", MATTMC_V2, &target).unwrap();
        assert_eq!(fs::read_to_string(target.join("app/bin/mattmc")).unwrap(), "launcher v2");
        assert!(target.join("app/scripts/backup.sh").is_file());
        assert!(!target.join("MattMC").exists());
        assert!(target.join(MANIFEST).is_file());
        assert!(target.join("data/save").is_dir());
    }
    #[test]
    fn wrapped_archive_update_replaces_app_and_leaves_data_untouched() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = mattmc();
        let target = stage.0.join("Games/MattMC");
        let v1 = [
            ("MattMC/app/bin/mattmc", "launcher v1"),
            ("MattMC/app/lib/old.jar", "jar v1"),
        ];
        install_release(&stage, &game, "v1", &v1, &target).unwrap();
        write(&target.join("data/save/world/level.dat"), "world");
        write(&target.join("data/config/shared/options.txt"), "fov:90");
        write(&target.join("data/cache/logs/latest.log"), "log");
        install_release(&stage, &game, "v2", MATTMC_V2, &target).unwrap();
        let read = |path: &str| fs::read_to_string(target.join(path)).unwrap();
        assert_eq!(read("app/bin/mattmc"), "launcher v2");
        assert!(!target.join("app/lib/old.jar").exists());
        assert_eq!(read("data/save/world/level.dat"), "world");
        assert_eq!(read("data/config/shared/options.txt"), "fov:90");
        assert_eq!(read("data/cache/logs/latest.log"), "log");
    }
    #[test]
    fn unwrapped_archive_still_installs_without_archive_root() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = crate::catalog::standard_game(serde_json::json!({}));
        let target = stage.0.join("Games/Sample");
        install_release(&stage, &game, "flat", &[("app/bin/sample", "binary")], &target).unwrap();
        assert_eq!(fs::read_to_string(target.join("app/bin/sample")).unwrap(), "binary");
        // Without archive_root a wrapper is a mistake, with a hint at the fix.
        let error = install_release(
            &stage,
            &game,
            "wrapped",
            &[("Sample/app/bin/sample", "binary")],
            &target,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("source.archive_root"), "{error}");
    }
    #[test]
    fn mismatched_wrapped_archives_fail_before_touching_the_installation() {
        let stage = Stage::new(&std::env::temp_dir()).unwrap();
        let game = mattmc();
        let target = stage.0.join("Games/MattMC");
        install_release(&stage, &game, "good", MATTMC_V2, &target).unwrap();
        write(&target.join("data/save/world/level.dat"), "world");
        let cases: &[(&str, &[(&str, &str)], &str)] = &[
            (
                "wrong-name",
                &[("MattMC-1.0/app/bin/mattmc", "x")],
                "only the folder MattMC/",
            ),
            (
                "extra-entry",
                &[("MattMC/app/bin/mattmc", "x"), ("README.txt", "x")],
                "README.txt",
            ),
            (
                "data-in-wrapper",
                &[("MattMC/app/bin/mattmc", "x"), ("MattMC/data/save/default", "x")],
                "data/",
            ),
            (
                "basalt-in-wrapper",
                &[("MattMC/app/bin/mattmc", "x"), ("MattMC/.basalt/state", "x")],
                ".basalt/",
            ),
            (
                "unwrapped",
                &[("app/bin/mattmc", "x")],
                "not wrapped in a MattMC/ folder",
            ),
            (
                "wrapper-holds-more-than-app",
                &[("MattMC/app/bin/mattmc", "x"), ("MattMC/run-mattmc.sh", "x")],
                "run-mattmc.sh",
            ),
            ("wrapper-is-a-file", &[("MattMC", "x")], "only the folder MattMC/"),
            ("empty", &[], "empty"),
        ];
        for (name, entries, reason) in cases {
            let error = install_release(&stage, &game, name, entries, &target)
                .expect_err(name)
                .to_string();
            assert!(error.contains(reason), "{name}: {error}");
            assert_eq!(
                fs::read_to_string(target.join("app/bin/mattmc")).unwrap(),
                "launcher v2",
                "{name} touched app/"
            );
            assert_eq!(
                fs::read_to_string(target.join("data/save/world/level.dat")).unwrap(),
                "world"
            );
        }
    }
}
