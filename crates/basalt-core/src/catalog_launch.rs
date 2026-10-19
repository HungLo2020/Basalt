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
    crate::platform::launch_catalog_game(
        &contained_path(root, &game.launch.executable)?,
        &contained_path(root, &game.launch.working_directory)?,
        &game.launch.arguments,
    )
}
