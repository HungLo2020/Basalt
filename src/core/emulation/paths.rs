use std::fs;
use std::path::{Path, PathBuf};

use crate::platform;

use super::discoverable_systems;

pub(super) fn roms_root_dir() -> Result<PathBuf, String> {
    Ok(emulators_root_dir()?.join("roms"))
}

pub(super) fn saves_root_dir() -> Result<PathBuf, String> {
    Ok(emulators_root_dir()?.join("saves"))
}

pub(super) fn ensure_emulator_directories() -> Result<(), String> {
    let root = emulators_root_dir()?;
    let roms_root = root.join("roms");
    let saves_root = root.join("saves");
    let runtime_root = root.join("runtime").join("retroarch");
    let cores_root = runtime_root.join("cores");

    fs::create_dir_all(&roms_root)
        .map_err(|error| format!("Failed to create emulator ROM root: {}", error))?;
    fs::create_dir_all(&saves_root)
        .map_err(|error| format!("Failed to create emulator save root: {}", error))?;
    fs::create_dir_all(&cores_root)
        .map_err(|error| format!("Failed to create RetroArch core directory: {}", error))?;

    for system in discoverable_systems() {
        fs::create_dir_all(roms_root.join(system))
            .map_err(|error| format!("Failed to create ROM directory for {}: {}", system, error))?;
        fs::create_dir_all(saves_root.join(system)).map_err(|error| {
            format!("Failed to create save directory for {}: {}", system, error)
        })?;
    }

    Ok(())
}

pub(super) fn emulators_root_dir() -> Result<PathBuf, String> {
    Ok(platform::home_dir()?.join("Games").join("Emulators"))
}

pub(super) fn retroarch_runtime_dir() -> Result<PathBuf, String> {
    Ok(emulators_root_dir()?.join("runtime").join("retroarch"))
}

pub(super) fn retroarch_cores_dir() -> Result<PathBuf, String> {
    Ok(retroarch_runtime_dir()?.join("cores"))
}

pub(super) fn retroarch_autoconfig_root_dir() -> Result<PathBuf, String> {
    Ok(retroarch_runtime_dir()?.join("autoconfig"))
}

pub(super) fn canonicalize_or_keep(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn normalize_system_key(system: &str) -> Result<String, String> {
    let normalized = system.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("System key cannot be empty".to_string());
    }

    if normalized
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        Ok(normalized)
    } else {
        Err(format!(
            "System key '{}' contains invalid characters",
            system
        ))
    }
}
