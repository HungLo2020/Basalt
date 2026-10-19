//! Manifest-driven maintenance script execution; no game-specific dispatch.
use crate::{
    catalog,
    error::{CoreError, CoreResult},
    platform, registry,
};
use std::path::{Path, PathBuf};

/// Run an action listed in the game's catalog manifest for an existing installation.
pub fn run_game_maintenance(game_id: &str, action: &str) -> CoreResult<()> {
    let definition = catalog::find_game(game_id)?;
    let command = definition.maintenance.get(action).ok_or_else(|| {
        CoreError::InvalidInput(format!(
            "Game {} does not support maintenance action {action}",
            definition.name
        ))
    })?;
    // A saved launch target anchors the installation, including legacy MattMC.
    let entry = registry::load_entries()?
        .into_iter()
        .find(|e| e.name == definition.name)
        .ok_or_else(|| CoreError::GameNotFound(definition.name.clone()))?;
    let launch = PathBuf::from(&entry.launch_target);
    let mut root = launch
        .parent()
        .ok_or_else(|| CoreError::new("Launch target has no parent"))?
        .to_path_buf();
    // Catalog launch paths may contain subdirectories within the installation.
    let depth = if entry.runner_kind == crate::RunnerKind::Catalog {
        0
    } else if entry.runner_kind == crate::RunnerKind::Bash {
        Path::new(&definition.launch.executable)
            .components()
            .count()
            .saturating_sub(1)
    } else {
        return Err(CoreError::InvalidInput(
            "Game does not have a local catalog installation".into(),
        ));
    };
    for _ in 0..depth {
        root = root
            .parent()
            .ok_or_else(|| CoreError::new("Invalid installation root"))?
            .to_path_buf();
    }
    let root = root
        .canonicalize()
        .map_err(|e| CoreError::new(format!("Cannot resolve game root: {e}")))?;
    let script = root.join(&command.path);
    let resolved = script
        .canonicalize()
        .map_err(|e| CoreError::new(format!("Missing maintenance script: {e}")))?;
    if !resolved.starts_with(&root) {
        return Err(CoreError::InvalidInput(
            "Maintenance script escapes game installation".into(),
        ));
    }
    platform::run_maintenance_script(
        &resolved,
        &root,
        &command.arguments,
        command.stdin.as_deref(),
    )
}
