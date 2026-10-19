//! Multiple configurable game library roots. Never erase existing installations.
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
use crate::{error::{CoreError, CoreResult}, platform, storage};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameLibrary {
    pub id: String,
    pub path: PathBuf,
}
fn file_path() -> CoreResult<PathBuf> {
    Ok(storage::config_dir()?.join("libraries.json"))
}
pub fn default_library() -> CoreResult<GameLibrary> {
    Ok(GameLibrary { id: "default".into(), path: platform::home_dir()?.join("Games") })
}
pub fn list_libraries() -> CoreResult<Vec<GameLibrary>> {
    let default = default_library()?;
    let file = file_path()?;
    if !file.exists() { return Ok(vec![default]); }
    let bytes = fs::read(&file).map_err(|err| CoreError::io("Read libraries", &file, err))?;
    let mut libraries: Vec<GameLibrary> = serde_json::from_slice(&bytes)
        .map_err(|err| CoreError::Json { path: file, source: err })?;
    if let Some(existing) = libraries.iter_mut().find(|v| v.id == "default") {
        existing.path = default.path;
    } else {
        libraries.insert(0, default);
    }
    Ok(libraries)
}
pub fn add_library(id: &str, path: &Path) -> CoreResult<GameLibrary> {
    if id.is_empty() || id == "default" || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err(CoreError::InvalidInput("Invalid library ID".into()));
    }
    if !path.is_absolute() { return Err(CoreError::InvalidInput("Library path must be absolute".into())); }
    fs::create_dir_all(path).map_err(|err| CoreError::io("Create library", path, err))?;
    let path = path.canonicalize().map_err(|err| CoreError::io("Resolve library", path, err))?;
    storage::with_data_lock(|| {
        let mut libraries = list_libraries()?;
        if libraries.iter().any(|v| v.id == id || v.path == path) {
            return Err(CoreError::InvalidInput("Library ID or directory already registered".into()));
        }
        let library = GameLibrary { id: id.into(), path };
        libraries.push(library.clone());
        let bytes = serde_json::to_vec_pretty(&libraries).map_err(|err| CoreError::new(err.to_string()))?;
        storage::write_atomic(&file_path()?, &bytes)?;
        Ok(library)
    })
}
pub fn install_directory(library_id: &str, game_id: &str) -> CoreResult<PathBuf> {
    let game = crate::catalog::find_game(game_id)?;
    let library = list_libraries()?.into_iter().find(|l| l.id == library_id)
        .ok_or_else(|| CoreError::InvalidInput(format!("Unknown game library: {library_id}")))?;
    Ok(library.path.join(game.id))
}
