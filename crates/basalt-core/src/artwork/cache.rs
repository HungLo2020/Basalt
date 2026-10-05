use std::fs;
use std::path::PathBuf;

use super::{EMULATOR_ARTWORK_IMAGES_PATH, EMULATOR_ARTWORK_INDEX_PATH};
use crate::error::{CoreError, CoreResult};
use crate::storage;

const STEAM_ARTWORK_DIR_NAME: &str = "steam_artwork";
const EMULATOR_ARTWORK_DIR_NAME: &str = "emulator_artwork";

/// Deletes all downloaded artwork and artwork metadata so it is fetched again.
pub fn clear_artwork_cache() -> CoreResult<()> {
    let cache_root = storage::cache_dir()?;
    let steam_dir = cache_root.join(STEAM_ARTWORK_DIR_NAME);
    let emulator_root = cache_root.join(EMULATOR_ARTWORK_DIR_NAME);

    let _ = fs::remove_dir_all(&steam_dir);
    let _ = fs::remove_dir_all(&emulator_root);

    for dir in [
        steam_dir,
        emulator_root.join(EMULATOR_ARTWORK_IMAGES_PATH),
        emulator_root.join(EMULATOR_ARTWORK_INDEX_PATH),
    ] {
        fs::create_dir_all(&dir).map_err(|error| CoreError::io("Failed to create", &dir, error))?;
    }

    Ok(())
}

pub(super) fn emulator_artwork_images_cache_dir() -> Option<PathBuf> {
    ensure_cache_subdir(&[EMULATOR_ARTWORK_DIR_NAME, EMULATOR_ARTWORK_IMAGES_PATH])
}

pub(super) fn emulator_artwork_index_cache_dir() -> Option<PathBuf> {
    ensure_cache_subdir(&[EMULATOR_ARTWORK_DIR_NAME, EMULATOR_ARTWORK_INDEX_PATH])
}

pub(super) fn steam_artwork_cache_dir() -> Option<PathBuf> {
    ensure_cache_subdir(&[STEAM_ARTWORK_DIR_NAME])
}

pub(super) fn current_unix_timestamp_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn ensure_cache_subdir(segments: &[&str]) -> Option<PathBuf> {
    let mut dir = storage::cache_dir().ok()?;
    for segment in segments {
        dir.push(segment);
    }

    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}
