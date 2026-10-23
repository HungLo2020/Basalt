//! Catalog Flatpaks use the user's installation. Flatpak owns packages and runtimes;
//! Basalt stores a validated app/branch reference rather than a filesystem launch path.
use crate::{
    CoreError, CoreResult, GameDefinition, GameEntry, GameInstallReport, Progress, RunnerKind,
};
use crate::{platform, registry, storage};
use serde::{Deserialize, Serialize};

pub(crate) fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub(crate) fn valid_app_id(value: &str) -> bool {
    value.split('.').count() >= 3
        && value.split('.').all(|part| {
            !part.is_empty()
                && (part.as_bytes()[0].is_ascii_alphabetic() || part.starts_with('_'))
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    app_id: String,
    branch: String,
}

impl Target {
    fn parse(value: &str) -> CoreResult<Self> {
        let target: Self = serde_json::from_str(value)
            .map_err(|_| CoreError::InvalidInput("Invalid registered Flatpak target".into()))?;
        if !valid_app_id(&target.app_id) || !valid_component(&target.branch) {
            return Err(CoreError::InvalidInput(
                "Invalid registered Flatpak target".into(),
            ));
        }
        Ok(target)
    }
}

pub(crate) fn target(game: &GameDefinition) -> String {
    serde_json::to_string(&Target {
        app_id: game.source.app_id.clone(),
        branch: game.flatpak_branch().into(),
    })
    .expect("Flatpak target serialization")
}

fn require_flatpak() -> CoreResult<()> {
    if !platform::command_exists("flatpak") {
        return Err(CoreError::new(
            "Flatpak is not installed. Install Flatpak and configure the catalog remote for your user first.",
        ));
    }
    Ok(())
}

/// No Flatpak installation is an ordinary discovery result, not an error.
pub(crate) fn installed_apps() -> CoreResult<Vec<(String, String, String)>> {
    if !platform::command_exists("flatpak") {
        return Ok(Vec::new());
    }
    let output = platform::run_command(
        "flatpak",
        &[
            "list",
            "--user",
            "--app",
            "--columns=application,branch,origin",
        ],
    )?;
    if !output.status.success() {
        return Err(CoreError::new(format!(
            "Could not list Flatpak games: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            (fields.len() == 3).then(|| (fields[0].into(), fields[1].into(), fields[2].into()))
        })
        .collect())
}

pub(crate) fn is_installed(game: &GameDefinition, apps: &[(String, String, String)]) -> bool {
    apps.iter().any(|(app, branch, origin)| {
        app == &game.source.app_id
            && branch == game.flatpak_branch()
            && origin == &game.source.remote
    })
}

fn check_registration(game: &GameDefinition, entries: &[GameEntry]) -> CoreResult<()> {
    if registry::load_blacklisted_names()?.contains(&game.name.to_lowercase()) {
        return Err(CoreError::Blacklisted(game.name.clone()));
    }
    let launch_target = target(game);
    for entry in entries {
        if entry.name == game.name
            && (entry.runner_kind != RunnerKind::Flatpak || entry.launch_target != launch_target)
        {
            return Err(CoreError::GameNameExists(game.name.clone()));
        }
        if entry.name != game.name
            && entry.runner_kind == RunnerKind::Flatpak
            && entry.launch_target == launch_target
        {
            return Err(CoreError::GameTargetExists(game.source.app_id.clone()));
        }
    }
    Ok(())
}

pub(crate) fn install(game: &GameDefinition, progress: &Progress) -> CoreResult<GameInstallReport> {
    game.validate()?;
    require_flatpak()?;
    storage::with_data_lock(|| check_registration(game, &registry::load_entries()?))?;
    progress.report_step(format!("Installing {} with Flatpak", game.name));
    let reference = format!("{}//{}", game.source.app_id, game.flatpak_branch());
    platform::run_cancellable_command(
        "flatpak",
        &[
            "install",
            "--user",
            "--app",
            "--noninteractive",
            "--assumeyes",
            "--or-update",
            &game.source.remote,
            &reference,
        ],
        progress,
    )
    .map_err(|error| {
        if matches!(error, CoreError::Cancelled) {
            error
        } else {
            CoreError::new(format!(
                "{error}. Flatpak remote '{}' must be configured for your user. \
                 If installation completed, use Discover to register it.",
                game.source.remote,
            ))
        }
    })?;
    if !is_installed(game, &installed_apps()?) {
        return Err(CoreError::new(
            "Flatpak did not report the requested application as installed",
        ));
    }
    // A completed transaction is registered even if cancellation arrives just afterwards.
    storage::with_data_lock(|| {
        let mut entries = registry::load_entries()?;
        check_registration(game, &entries)?;
        entries.retain(|entry| entry.name != game.name);
        entries.push(GameEntry {
            name: game.name.clone(),
            runner_kind: RunnerKind::Flatpak,
            launch_target: target(game),
        });
        registry::save_entries(&entries)?;
        crate::playlist_service::sync_automatic_playlists()
    })?;
    Ok(GameInstallReport {
        game_name: game.name.clone(),
        release_tag: game.flatpak_branch().into(),
        install_dir: None,
    })
}

pub(crate) fn launch(value: &str) -> CoreResult<()> {
    let target = Target::parse(value)?;
    require_flatpak()?;
    platform::run_attached_command(
        "flatpak",
        &[
            "run",
            "--user",
            &format!("--branch={}", target.branch),
            &target.app_id,
        ],
    )
}

pub(crate) fn update(game: &GameDefinition, progress: &Progress) -> CoreResult<String> {
    require_flatpak()?;
    let entry = registry::load_entries()?
        .into_iter()
        .find(|entry| entry.name == game.name)
        .ok_or_else(|| CoreError::GameNotFound(game.name.clone()))?;
    if entry.runner_kind != RunnerKind::Flatpak || entry.launch_target != target(game) {
        return Err(CoreError::new(
            "Registered Flatpak differs from the catalog; remove its Basalt entry and install again",
        ));
    }
    if !is_installed(game, &installed_apps()?) {
        return Err(CoreError::new(
            "Flatpak game is no longer installed; install it again",
        ));
    }
    progress.report_step(format!("Updating {} with Flatpak", game.name));
    platform::run_cancellable_command(
        "flatpak",
        &[
            "update",
            "--user",
            "--app",
            "--noninteractive",
            "--assumeyes",
            &format!("{}//{}", game.source.app_id, game.flatpak_branch()),
        ],
        progress,
    )?;
    Ok(format!("Updated {} using Flatpak", game.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_catalog_and_registered_targets_before_execution() {
        let game = crate::find_game("SuperTuxKart").unwrap();
        assert!(!game.follows_standard());
        assert!(Target::parse(&target(&game)).is_ok());
        for value in [
            r#"{"app_id":"--command=sh","branch":"stable"}"#,
            r#"{"app_id":"net.example.Game","branch":"stable; touch /tmp/x"}"#,
            r#"{"app_id":"net.example.Game","branch":"--help"}"#,
            r#"{"app_id":"net.example.Game","branch":"stable","extra":"x"}"#,
        ] {
            assert!(Target::parse(value).is_err());
        }
        for (field, value) in [
            ("app_id", "--help"),
            ("remote", "https://evil"),
            ("branch", "../stable"),
        ] {
            let mut json = serde_json::to_value(&game).unwrap();
            json["source"][field] = value.into();
            let invalid: GameDefinition = serde_json::from_value(json).unwrap();
            assert!(invalid.validate().is_err());
        }
        let mut old = game.clone();
        old.schema_version = 2;
        assert!(old.validate().is_err());
        let mut mixed = game;
        mixed.launch.executable = "app/bin/game".into();
        assert!(mixed.validate().is_err());
    }
}
