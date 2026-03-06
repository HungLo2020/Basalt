use std::env;
use std::path::PathBuf;

use super::{EMULATOR_ARTWORK_IMAGES_PATH, EMULATOR_ARTWORK_INDEX_PATH};

pub(super) fn clear_artwork_cache_files() {
    if let Some(steam_dir) = steam_artwork_cache_dir() {
        let _ = std::fs::remove_dir_all(&steam_dir);
    }

    if let Some(emulator_root_dir) = emulator_artwork_cache_root_dir() {
        let _ = std::fs::remove_dir_all(&emulator_root_dir);
    }

    if let Some(steam_dir) = steam_artwork_cache_dir() {
        let _ = std::fs::create_dir_all(steam_dir);
    }

    if let Some(emulator_images_dir) = emulator_artwork_images_cache_dir() {
        let _ = std::fs::create_dir_all(emulator_images_dir);
    }

    if let Some(emulator_index_dir) = emulator_artwork_index_cache_dir() {
        let _ = std::fs::create_dir_all(emulator_index_dir);
    }
}

pub(super) fn emulator_artwork_images_cache_dir() -> Option<PathBuf> {
    let cache_dir = emulator_artwork_cache_root_dir()?.join(EMULATOR_ARTWORK_IMAGES_PATH);
    if std::fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }
    Some(cache_dir)
}

pub(super) fn emulator_artwork_index_cache_dir() -> Option<PathBuf> {
    let cache_dir = emulator_artwork_cache_root_dir()?.join(EMULATOR_ARTWORK_INDEX_PATH);
    if std::fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }
    Some(cache_dir)
}

pub(super) fn steam_artwork_cache_dir() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let cache_dir = PathBuf::from(home)
        .join(".basalt")
        .join("cache")
        .join("steam_artwork");

    if std::fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }

    Some(cache_dir)
}

pub(super) fn current_unix_timestamp_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn emulator_artwork_cache_root_dir() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let cache_root = PathBuf::from(home)
        .join(".basalt")
        .join("cache")
        .join("emulator_artwork");

    if std::fs::create_dir_all(&cache_root).is_err() {
        return None;
    }

    Some(cache_root)
}
