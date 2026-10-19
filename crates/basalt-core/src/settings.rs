use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::error::{CoreError, CoreResult};
use super::storage;

const SETTINGS_FILE_NAME: &str = "settings.json";

/// On-disk shape of settings.json. Every field is optional so partial or older files load, and
/// unknown keys are carried through `extra` so saving never drops data written by other versions.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SettingsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launcher: Option<LauncherSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sync: Option<SyncSection>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SyncSection {
    /// Folder that game data is synced to (a local disk or a mounted network/cloud folder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote: Option<PathBuf>,
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

#[derive(Clone, Copy)]
pub struct LauncherDisplaySettings {
    pub fullscreen_enabled: bool,
    pub maximized_enabled: bool,
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

pub fn save_launcher_display_settings(fullscreen_enabled: bool, maximized_enabled: bool) -> CoreResult<LauncherDisplaySettings> {
    let maximized_enabled = maximized_enabled && !fullscreen_enabled;
    update_settings_file(|settings| {
        let launcher = settings.launcher.get_or_insert_with(Default::default);
        launcher.fullscreen = Some(fullscreen_enabled);
        launcher.maximized = Some(maximized_enabled);
    })?;
    Ok(LauncherDisplaySettings { fullscreen_enabled, maximized_enabled })
}
/// Where games sync to when no folder is configured.
pub const DEFAULT_SYNC_REMOTE: &str = "/mnt/storage/Storage/Sync";

/// The folder games sync to, and whether it is the default (nothing configured).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncRemote {
    pub path: PathBuf,
    pub is_default: bool,
}

pub fn load_sync_remote() -> CoreResult<SyncRemote> {
    Ok(sync_remote_from(read_settings_file()?))
}

fn sync_remote_from(settings: SettingsFile) -> SyncRemote {
    match settings.sync.and_then(|sync| sync.remote) {
        Some(path) => SyncRemote {
            path,
            is_default: false,
        },
        None => SyncRemote {
            path: PathBuf::from(DEFAULT_SYNC_REMOTE),
            is_default: true,
        },
    }
}

/// Sets the sync folder, or with `None` returns to the default. It must be an absolute path; it does not have
/// to exist yet, since network folders may be unmounted while configuring.
pub fn save_sync_remote(remote: Option<&std::path::Path>) -> CoreResult<()> {
    if let Some(path) = remote
        && !path.is_absolute()
    {
        return Err(CoreError::InvalidInput(
            "The sync folder must be an absolute path".to_string(),
        ));
    }
    update_settings_file(|settings| {
        settings.sync.get_or_insert_with(Default::default).remote = remote.map(PathBuf::from);
    })
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

        assert_eq!(reparsed["launcher"]["fullscreen"], true);
        assert_eq!(reparsed["launcher"]["maximized"], false);
        assert_eq!(reparsed["some_new_section"]["enabled"], true);
    }

    #[test]
    fn sync_folder_defaults_until_configured() {
        let unset = sync_remote_from(parse_settings("{}").unwrap());
        assert_eq!(unset.path, PathBuf::from(DEFAULT_SYNC_REMOTE));
        assert!(unset.is_default);
        let set = sync_remote_from(
            parse_settings(r#"{"sync": {"remote": "/media/usb/Sync"}}"#).unwrap(),
        );
        assert_eq!(set.path, PathBuf::from("/media/usb/Sync"));
        assert!(!set.is_default);
    }

}
