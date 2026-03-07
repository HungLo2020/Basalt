use std::fs;

use super::registry;

const CACHE_DIR_NAME: &str = "cache";
const STEAM_ARTWORK_DIR_NAME: &str = "steam_artwork";
const EMULATOR_ARTWORK_DIR_NAME: &str = "emulator_artwork";
const EMULATOR_ARTWORK_IMAGES_DIR_NAME: &str = "images";
const EMULATOR_ARTWORK_INDEX_DIR_NAME: &str = "index";

pub fn clear_artwork_cache() -> Result<(), String> {
    let app_dir = registry::get_app_dir()?;
    let cache_root = app_dir.join(CACHE_DIR_NAME);
    let steam_dir = cache_root.join(STEAM_ARTWORK_DIR_NAME);
    let emulator_root = cache_root.join(EMULATOR_ARTWORK_DIR_NAME);
    let emulator_images_dir = emulator_root.join(EMULATOR_ARTWORK_IMAGES_DIR_NAME);
    let emulator_index_dir = emulator_root.join(EMULATOR_ARTWORK_INDEX_DIR_NAME);

    let _ = fs::remove_dir_all(&steam_dir);
    let _ = fs::remove_dir_all(&emulator_root);

    fs::create_dir_all(&steam_dir)
        .map_err(|error| format!("Failed to create Steam artwork cache directory: {}", error))?;
    fs::create_dir_all(&emulator_images_dir).map_err(|error| {
        format!(
            "Failed to create emulator artwork images cache directory: {}",
            error
        )
    })?;
    fs::create_dir_all(&emulator_index_dir).map_err(|error| {
        format!(
            "Failed to create emulator artwork index cache directory: {}",
            error
        )
    })?;

    Ok(())
}
