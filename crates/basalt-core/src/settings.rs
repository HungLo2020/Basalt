use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::error::{CoreError, CoreResult};
use super::storage;

const SETTINGS_FILE_NAME: &str = "settings.json";
const DEFAULT_REMOTE_ROMS_ROOT_DIR: &str = "/mnt/storage/OneDrive/Apps/Games/Emulators/ROMs";
const DEFAULT_REMOTE_SAVES_ROOT_DIR: &str = "/mnt/storage/OneDrive/Apps/Games/Emulators/Saves";

/// On-disk shape of settings.json. Every field is optional so partial or older files load, and
/// unknown keys are carried through `extra` so saving never drops data written by other versions.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SettingsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    emulation: Option<EmulationSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launcher: Option<LauncherSection>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct EmulationSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote_roms_root_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote_saves_root_dir: Option<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LauncherSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fullscreen: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maximized: Option<bool>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Clone)]
pub struct EmulationRemotePaths {
    pub roms_root_dir: String,
    pub saves_root_dir: String,
}

#[derive(Clone, Copy)]
pub struct LauncherDisplaySettings {
    pub fullscreen_enabled: bool,
    pub maximized_enabled: bool,
}

pub fn default_emulation_remote_paths() -> EmulationRemotePaths {
    EmulationRemotePaths {
        roms_root_dir: DEFAULT_REMOTE_ROMS_ROOT_DIR.to_string(),
        saves_root_dir: DEFAULT_REMOTE_SAVES_ROOT_DIR.to_string(),
    }
}

pub fn load_emulation_remote_paths() -> CoreResult<EmulationRemotePaths> {
    Ok(emulation_remote_paths_from(&read_settings_file()?))
}

pub fn load_launcher_display_settings() -> CoreResult<LauncherDisplaySettings> {
    let settings = read_settings_file()?;
    let launcher = settings.launcher.unwrap_or_default();
    let fullscreen_enabled = launcher.fullscreen.unwrap_or(false);
    let maximized_enabled = launcher.maximized.unwrap_or(false) && !fullscreen_enabled;

    Ok(LauncherDisplaySettings {
        fullscreen_enabled,
        maximized_enabled,
    })
}

pub fn save_emulation_remote_paths(
    roms_root_dir: &str,
    saves_root_dir: &str,
) -> CoreResult<EmulationRemotePaths> {
    let roms_root_dir = roms_root_dir.trim();
    if roms_root_dir.is_empty() {
        return Err(CoreError::InvalidInput(
            "Remote ROM root path cannot be empty".to_string(),
        ));
    }

    let saves_root_dir = saves_root_dir.trim();
    if saves_root_dir.is_empty() {
        return Err(CoreError::InvalidInput(
            "Remote save root path cannot be empty".to_string(),
        ));
    }

    update_settings_file(|settings| {
        let emulation = settings.emulation.get_or_insert_with(Default::default);
        emulation.remote_roms_root_dir = Some(roms_root_dir.to_string());
        emulation.remote_saves_root_dir = Some(saves_root_dir.to_string());
    })?;

    Ok(EmulationRemotePaths {
        roms_root_dir: roms_root_dir.to_string(),
        saves_root_dir: saves_root_dir.to_string(),
    })
}

pub fn save_launcher_display_settings(
    fullscreen_enabled: bool,
    maximized_enabled: bool,
) -> CoreResult<LauncherDisplaySettings> {
    let maximized_enabled = maximized_enabled && !fullscreen_enabled;

    update_settings_file(|settings| {
        let launcher = settings.launcher.get_or_insert_with(Default::default);
        launcher.fullscreen = Some(fullscreen_enabled);
        launcher.maximized = Some(maximized_enabled);
    })?;

    Ok(LauncherDisplaySettings {
        fullscreen_enabled,
        maximized_enabled,
    })
}

fn emulation_remote_paths_from(settings: &SettingsFile) -> EmulationRemotePaths {
    let defaults = default_emulation_remote_paths();
    let emulation = settings.emulation.as_ref();
    let non_empty = |value: Option<&String>, fallback: String| {
        value
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or(fallback)
    };

    EmulationRemotePaths {
        roms_root_dir: non_empty(
            emulation.and_then(|section| section.remote_roms_root_dir.as_ref()),
            defaults.roms_root_dir,
        ),
        saves_root_dir: non_empty(
            emulation.and_then(|section| section.remote_saves_root_dir.as_ref()),
            defaults.saves_root_dir,
        ),
    }
}

fn settings_file_path() -> CoreResult<PathBuf> {
    Ok(storage::config_dir()?.join(SETTINGS_FILE_NAME))
}

fn read_settings_file() -> CoreResult<SettingsFile> {
    let path = settings_file_path()?;
    if !path.exists() {
        return Ok(SettingsFile::default());
    }

    let contents =
        fs::read_to_string(&path).map_err(|error| CoreError::io("Failed to read", &path, error))?;
    parse_settings(&contents).map_err(|source| CoreError::Json { path, source })
}

fn parse_settings(contents: &str) -> Result<SettingsFile, serde_json::Error> {
    if contents.trim().is_empty() {
        Ok(SettingsFile::default())
    } else {
        serde_json::from_str(contents)
    }
}

/// Read-modify-write of settings.json under the data lock. A file that no longer parses is
/// replaced rather than blocking every future save.
fn update_settings_file(apply: impl FnOnce(&mut SettingsFile)) -> CoreResult<()> {
    storage::with_data_lock(|| {
        let mut settings = read_settings_file().unwrap_or_default();
        apply(&mut settings);

        let serialized = serde_json::to_string_pretty(&settings)
            .map_err(|error| CoreError::new(format!("Failed to serialize settings: {}", error)))?;
        storage::write_atomic(&settings_file_path()?, serialized.as_bytes())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_preserves_unknown_keys() {
        let original = r#"{
            "emulation": { "remote_roms_root_dir": "/roms", "future_option": 3 },
            "launcher": { "fullscreen": true },
            "some_new_section": { "enabled": true }
        }"#;

        let mut settings = parse_settings(original).unwrap();
        settings
            .launcher
            .get_or_insert_with(Default::default)
            .maximized = Some(false);
        let reparsed: Value =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();

        assert_eq!(reparsed["emulation"]["remote_roms_root_dir"], "/roms");
        assert_eq!(reparsed["emulation"]["future_option"], 3);
        assert_eq!(reparsed["launcher"]["fullscreen"], true);
        assert_eq!(reparsed["launcher"]["maximized"], false);
        assert_eq!(reparsed["some_new_section"]["enabled"], true);
    }

    #[test]
    fn remote_paths_fall_back_to_defaults_for_missing_or_blank_values() {
        let settings =
            parse_settings(r#"{ "emulation": { "remote_saves_root_dir": "   " } }"#).unwrap();
        let paths = emulation_remote_paths_from(&settings);
        let defaults = default_emulation_remote_paths();

        assert_eq!(paths.roms_root_dir, defaults.roms_root_dir);
        assert_eq!(paths.saves_root_dir, defaults.saves_root_dir);
        assert!(parse_settings("").is_ok());
    }
}
