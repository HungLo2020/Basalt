use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::registry;

const SETTINGS_FILE_NAME: &str = "settings.json";
const DEFAULT_REMOTE_ROMS_ROOT_DIR: &str = "/mnt/storage/OneDrive/Apps/Games/Emulators/ROMs";
const DEFAULT_REMOTE_SAVES_ROOT_DIR: &str = "/mnt/storage/OneDrive/Apps/Games/Emulators/Saves";

#[derive(Clone)]
pub struct EmulationRemotePaths {
    pub roms_root_dir: String,
    pub saves_root_dir: String,
}

pub fn default_emulation_remote_paths() -> EmulationRemotePaths {
    EmulationRemotePaths {
        roms_root_dir: DEFAULT_REMOTE_ROMS_ROOT_DIR.to_string(),
        saves_root_dir: DEFAULT_REMOTE_SAVES_ROOT_DIR.to_string(),
    }
}

pub fn load_emulation_remote_paths() -> Result<EmulationRemotePaths, String> {
    let defaults = default_emulation_remote_paths();
    let settings_path = settings_file_path()?;
    if !settings_path.exists() {
        return Ok(defaults);
    }

    let contents = fs::read_to_string(&settings_path).map_err(|error| {
        format!(
            "Failed to read settings file {}: {}",
            settings_path.display(),
            error
        )
    })?;

    if contents.trim().is_empty() {
        return Ok(defaults);
    }

    let root: Value = serde_json::from_str(&contents).map_err(|error| {
        format!(
            "Failed to parse settings file {}: {}",
            settings_path.display(),
            error
        )
    })?;

    let emulation_settings = root
        .get("emulation")
        .and_then(Value::as_object);

    let roms_root_dir = emulation_settings
        .and_then(|value| value.get("remote_roms_root_dir"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(defaults.roms_root_dir.as_str())
        .to_string();

    let saves_root_dir = emulation_settings
        .and_then(|value| value.get("remote_saves_root_dir"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(defaults.saves_root_dir.as_str())
        .to_string();

    Ok(EmulationRemotePaths {
        roms_root_dir,
        saves_root_dir,
    })
}

pub fn save_emulation_remote_paths(
    roms_root_dir: &str,
    saves_root_dir: &str,
) -> Result<EmulationRemotePaths, String> {
    let normalized_roms_root_dir = roms_root_dir.trim();
    if normalized_roms_root_dir.is_empty() {
        return Err("Remote ROM root path cannot be empty".to_string());
    }

    let normalized_saves_root_dir = saves_root_dir.trim();
    if normalized_saves_root_dir.is_empty() {
        return Err("Remote save root path cannot be empty".to_string());
    }

    let app_dir = registry::get_app_dir()?;
    fs::create_dir_all(&app_dir)
        .map_err(|error| format!("Failed to create settings directory: {}", error))?;

    let settings_path = app_dir.join(SETTINGS_FILE_NAME);
    let mut root = if settings_path.exists() {
        let existing_contents = fs::read_to_string(&settings_path).map_err(|error| {
            format!(
                "Failed to read settings file {}: {}",
                settings_path.display(),
                error
            )
        })?;

        if existing_contents.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str::<Value>(&existing_contents).unwrap_or_else(|_| json!({}))
        }
    } else {
        json!({})
    };

    if !root.is_object() {
        root = json!({});
    }

    if let Some(root_object) = root.as_object_mut() {
        let emulation_value = root_object
            .entry("emulation".to_string())
            .or_insert_with(|| json!({}));

        if !emulation_value.is_object() {
            *emulation_value = json!({});
        }

        if let Some(emulation_object) = emulation_value.as_object_mut() {
            emulation_object.insert(
                "remote_roms_root_dir".to_string(),
                Value::String(normalized_roms_root_dir.to_string()),
            );
            emulation_object.insert(
                "remote_saves_root_dir".to_string(),
                Value::String(normalized_saves_root_dir.to_string()),
            );
        }
    }

    let serialized = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("Failed to serialize settings: {}", error))?;
    fs::write(&settings_path, serialized).map_err(|error| {
        format!(
            "Failed to write settings file {}: {}",
            settings_path.display(),
            error
        )
    })?;

    Ok(EmulationRemotePaths {
        roms_root_dir: normalized_roms_root_dir.to_string(),
        saves_root_dir: normalized_saves_root_dir.to_string(),
    })
}

fn settings_file_path() -> Result<PathBuf, String> {
    Ok(registry::get_app_dir()?.join(SETTINGS_FILE_NAME))
}