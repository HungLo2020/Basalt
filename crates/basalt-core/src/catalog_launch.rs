//! Installed catalog metadata anchors launch paths independently of library location.
use crate::{CoreError, CoreResult, catalog::GameDefinition};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) const MANIFEST: &str = ".basalt-game.json";

pub(crate) fn contained_path(root: &Path, relative: &str) -> CoreResult<PathBuf> {
    let root = root
        .canonicalize()
        .map_err(|e| CoreError::io("Resolve installation", root, e))?;
    let path = root.join(relative);
    let resolved = path
        .canonicalize()
        .map_err(|e| CoreError::io("Resolve game path", &path, e))?;
    if !resolved.starts_with(&root) {
        return Err(CoreError::InvalidInput(
            "Game path escapes installation".into(),
        ));
    }
    Ok(resolved)
}

pub(crate) fn validate_launch(root: &Path, game: &GameDefinition) -> CoreResult<()> {
    if !contained_path(root, &game.launch.executable)?.is_file()
        || !contained_path(root, &game.launch.working_directory)?.is_dir()
    {
        return Err(CoreError::new(
            "Release is missing its launch executable or working directory",
        ));
    }
    Ok(())
}

pub(crate) fn launch(target: &str) -> CoreResult<()> {
    let path = Path::new(target);
    let root = path
        .parent()
        .ok_or_else(|| CoreError::new("Invalid catalog launch target"))?;
    let bytes = fs::read(path).map_err(|e| CoreError::io("Read installed manifest", path, e))?;
    let game: GameDefinition =
        serde_json::from_slice(&bytes).map_err(|e| CoreError::new(e.to_string()))?;
    game.validate()?;
    validate_launch(root, &game)?;
    let root = root
        .canonicalize()
        .map_err(|e| CoreError::io("Resolve installation", root, e))?;
    let marker = if game.follows_standard() {
        if crate::game_data::is_running(&root) {
            return Err(CoreError::InvalidInput(format!("{} is already running", game.name)));
        }
        if crate::game_data::operation_in_progress(&root) {
            return Err(CoreError::InvalidInput(format!(
                "{} is busy (an update, sync, backup or restore is running)",
                game.name
            )));
        }
        Some(crate::game_data::running_marker(&root))
    } else {
        None
    };
    crate::platform::launch_catalog_game(
        &contained_path(&root, &game.launch.executable)?,
        &contained_path(&root, &game.launch.working_directory)?,
        &game.launch.arguments,
        &crate::game_data::environment(&game, &root, None)?,
        marker.as_deref(),
    )
}
