//! Bundled, declarative game definitions. Catalog metadata is untrusted; it never runs shell code.
//!
//! `schema_version` 1 is the original format (one preserved `persistent_directory` plus
//! game-owned `maintenance` scripts). `schema_version` 2 entries follow STANDARDS.md: Basalt's
//! default layout and built-in actions, overridable through `layout` and `actions`.
use crate::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The version of STANDARDS.md this build implements.
pub const STANDARD_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDefinition {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard_version: Option<u32>,
    pub id: String,
    pub name: String,
    pub source: ReleaseSource,
    pub launch: LaunchDefinition,
    /// Schema 1 only: game-owned scripts keyed by action name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub maintenance: BTreeMap<String, MaintenanceCommand>,
    /// Schema 1 only: the directory preserved on reinstall (default `run`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistent_directory: Option<String>,
    /// Schema 2 only: per-category overrides of the standard layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutDefinition>,
    /// Schema 2 only: per-action overrides and additional script actions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<String, ActionSetting>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSource {
    pub kind: String,
    pub repository: String,
    pub asset_prefix: String,
    pub asset_suffix: String,
    /// Required substrings, for example the release platform triplet.
    #[serde(default)]
    pub asset_contains: Vec<String>,
    /// Schema 2 only: the single folder the release archive wraps its files in. Its contents
    /// become the install root. Unset means the files are at the archive root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_root: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchDefinition {
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub working_directory: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceCommand {
    /// Script path relative to installation root.
    pub path: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    /// Optional input for scripts that prompt for a direction.
    #[serde(default)]
    pub stdin: Option<String>,
}

/// Each field replaces that category's default paths entirely.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_config: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<Vec<String>>,
}

/// `false` disables an action; an object configures, relabels or replaces it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionSetting {
    Enabled(bool),
    Configured(ActionConfig),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<ActionOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<MaintenanceCommand>,
}

/// Options for built-in actions; which ones apply depends on the action.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionOptions {
    /// backup: snapshots kept per game.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<u32>,
    /// backup, sync, sync_up, sync_down: data categories included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    /// update: snapshot user data before replacing the application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_first: Option<bool>,
}

fn safe_relative(value: &str) -> bool {
    use std::path::{Component, Path};
    !value.is_empty()
        && Path::new(value)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

/// Layout paths must stay inside the installation and away from Basalt's own files.
fn valid_layout_path(value: &str) -> bool {
    safe_relative(value)
        && std::path::Path::new(value)
            .components()
            .next()
            .is_some_and(|first| {
                first.as_os_str() != crate::game_layout::BASALT_DIR
                    && first.as_os_str() != crate::catalog_launch::MANIFEST
            })
}

fn valid_action_name(action: &str) -> bool {
    !action.is_empty()
        && action
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

/// A single folder name: letters, digits, '-', '_' and '.', and not '.', '..' or containing '..'.
fn valid_archive_root(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

fn valid_label(label: &str) -> bool {
    !label.trim().is_empty() && !label.contains(['\t', '\n', '\r'])
}

impl GameDefinition {
    /// True for entries that follow STANDARDS.md (schema 2).
    pub fn follows_standard(&self) -> bool {
        self.schema_version >= 2
    }

    /// Schema 1 preserved directory.
    pub fn legacy_persistent_directory(&self) -> &str {
        self.persistent_directory.as_deref().unwrap_or("run")
    }

    pub fn validate(&self) -> CoreResult<()> {
        let invalid = || {
            CoreError::InvalidInput(format!("Invalid game catalog entry: {}", self.id))
        };
        if !(1..=2).contains(&self.schema_version)
            || self.id.is_empty()
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !safe_relative(&self.launch.executable)
            || (!self.launch.working_directory.is_empty()
                && self.launch.working_directory != "."
                && !safe_relative(&self.launch.working_directory))
            || self.source.kind != "github_release"
            || self.source.repository.split('/').count() != 2
            || self
                .source
                .repository
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
            || self.name.is_empty()
            || self.name.contains(['\t', '\n', '\r'])
            || !self
                .source
                .repository
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-./".contains(&b))
            || self.source.asset_prefix.is_empty()
            || !self.source.asset_suffix.ends_with(".zip")
        {
            return Err(invalid());
        }
        if let Some(archive_root) = &self.source.archive_root {
            if !self.follows_standard() {
                return Err(CoreError::InvalidInput(format!(
                    "Invalid catalog entry {}: source.archive_root requires schema_version 2",
                    self.id
                )));
            }
            if !valid_archive_root(archive_root) {
                return Err(CoreError::InvalidInput(format!(
                    "Invalid catalog entry {}: source.archive_root {archive_root:?} must be a single \
                     folder name (letters, digits, '-', '_' and '.'; no '/' or '..')",
                    self.id
                )));
            }
        }
        if self.follows_standard() {
            self.validate_standard()
        } else {
            self.validate_legacy()
        }
    }

    fn validate_legacy(&self) -> CoreResult<()> {
        if self.standard_version.is_some()
            || self.layout.is_some()
            || !self.actions.is_empty()
            || !safe_relative(self.legacy_persistent_directory())
        {
            return Err(CoreError::InvalidInput(format!(
                "Invalid schema 1 catalog entry: {}",
                self.id
            )));
        }
        for (action, command) in &self.maintenance {
            if !valid_action_name(action)
                || !safe_relative(&command.path)
                || !command.path.ends_with(".sh")
            {
                return Err(CoreError::InvalidInput(format!(
                    "Invalid maintenance action {action} for {}",
                    self.id
                )));
            }
        }
        Ok(())
    }

    fn validate_standard(&self) -> CoreResult<()> {
        let invalid = |what: String| {
            CoreError::InvalidInput(format!("Invalid catalog entry {}: {what}", self.id))
        };
        match self.standard_version {
            Some(STANDARD_VERSION) => {}
            Some(other) => {
                return Err(invalid(format!(
                    "standard_version {other} is not supported (this Basalt implements {STANDARD_VERSION})"
                )));
            }
            None => return Err(invalid("standard_version is required".into())),
        }
        if !self.maintenance.is_empty() || self.persistent_directory.is_some() {
            return Err(invalid(
                "maintenance and persistent_directory are schema 1 fields; use actions and layout"
                    .into(),
            ));
        }
        if let Some(layout) = &self.layout {
            for (category, paths) in [
                ("app", &layout.app),
                ("save", &layout.save),
                ("shared_config", &layout.shared_config),
                ("config", &layout.config),
                ("cache", &layout.cache),
            ] {
                if let Some(paths) = paths
                    && (paths.is_empty() && category == "app"
                        || paths.iter().any(|path| !valid_layout_path(path)))
                {
                    return Err(invalid(format!("invalid layout.{category}")));
                }
            }
        }
        for (name, setting) in &self.actions {
            if !valid_action_name(name) {
                return Err(invalid(format!("invalid action name {name}")));
            }
            let builtin = crate::maintenance::BuiltinAction::parse(name);
            let config = match setting {
                ActionSetting::Enabled(true) if builtin.is_some() => continue,
                ActionSetting::Enabled(false) if builtin.is_some() => continue,
                ActionSetting::Enabled(_) => {
                    return Err(invalid(format!("custom action {name} needs a script")));
                }
                ActionSetting::Configured(config) => config,
            };
            if config.label.as_deref().is_some_and(|label| !valid_label(label)) {
                return Err(invalid(format!("invalid label for {name}")));
            }
            if let Some(script) = &config.script
                && !safe_relative(&script.path)
            {
                return Err(invalid(format!("invalid script path for {name}")));
            }
            match (builtin, &config.script, &config.options) {
                (_, Some(_), Some(_)) => {
                    return Err(invalid(format!("{name} cannot have both script and options")));
                }
                (None, None, _) => {
                    return Err(invalid(format!("custom action {name} needs a script")));
                }
                (Some(builtin), None, Some(options)) => builtin
                    .validate_options(options)
                    .map_err(|error| invalid(format!("{name}: {error}")))?,
                _ => {}
            }
        }
        Ok(())
    }
}
/// Every catalog entry: the bundled ones, overridden or extended by local entries in
/// `~/.config/basalt/games/*.json` (for trying a game's entry before it ships with Basalt).
/// Invalid local files are skipped with a warning.
pub fn catalog_games() -> CoreResult<Vec<GameDefinition>> {
    let mut games = bundled_games()?;
    for local in local_games() {
        match games.iter_mut().find(|game| game.id == local.id) {
            Some(existing) => *existing = local,
            None => games.push(local),
        }
    }
    Ok(games)
}

fn bundled_games() -> CoreResult<Vec<GameDefinition>> {
    let texts = [
        include_str!("../../../resources/games/mattmc.json"),
        include_str!("../../../resources/games/micromages.json"),
        include_str!("../../../resources/games/jackal.json"),
    ];
    texts
        .iter()
        .map(|value| {
            let entry: GameDefinition = serde_json::from_str(value)
                .map_err(|err| CoreError::new(format!("Invalid bundled game manifest: {err}")))?;
            entry.validate()?;
            Ok(entry)
        })
        .collect()
}

fn local_games() -> Vec<GameDefinition> {
    let Ok(directory) = crate::storage::config_dir().map(|dir| dir.join("games")) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let parsed = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<GameDefinition>(&bytes).map_err(|e| e.to_string())
                })
                .and_then(|game| game.validate().map(|()| game).map_err(|e| e.to_string()));
            match parsed {
                Ok(game) => Some(game),
                Err(error) => {
                    crate::warnings::warn(format!(
                        "Ignoring local catalog entry {}: {error}",
                        path.display()
                    ));
                    None
                }
            }
        })
        .collect()
}
pub fn find_game(id: &str) -> CoreResult<GameDefinition> {
    catalog_games()?
        .into_iter()
        .find(|game| game.id == id)
        .ok_or_else(|| CoreError::GameNotFound(id.into()))
}
/// MattMC's schema 1 entry from before it adopted the standard: the reference legacy game, with
/// a preserved `run/` and its own maintenance scripts.
#[cfg(test)]
pub(crate) fn legacy_mattmc() -> GameDefinition {
    serde_json::from_str(include_str!("testdata/legacy_mattmc.json")).unwrap()
}

/// A minimal schema 2 entry with `extra` fields merged over it.
#[cfg(test)]
pub(crate) fn standard_game(extra: serde_json::Value) -> GameDefinition {
    try_standard_game(extra).unwrap()
}

#[cfg(test)]
pub(crate) fn try_standard_game(
    extra: serde_json::Value,
) -> Result<GameDefinition, serde_json::Error> {
    let mut value = serde_json::json!({
        "schema_version": 2,
        "standard_version": 1,
        "id": "Sample",
        "name": "Sample",
        "source": {
            "kind": "github_release",
            "repository": "example/Sample",
            "asset_prefix": "Sample-",
            "asset_suffix": ".zip"
        },
        "launch": { "executable": "app/bin/sample" }
    });
    if let (Some(base), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
        for (key, item) in extra {
            base.insert(key.clone(), item.clone());
        }
    }
    serde_json::from_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn bundled_catalog_is_valid_and_unique() {
        let list = bundled_games().unwrap();
        let mut ids = std::collections::HashSet::new();
        for item in list {
            assert!(ids.insert(item.id));
        }
    }
    #[test]
    fn rejects_escaping_maintenance_path() {
        let mut game = legacy_mattmc();
        game.maintenance.get_mut("backup").unwrap().path = "../evil.sh".into();
        assert!(game.validate().is_err());
    }
    #[test]
    fn rejects_escaping_executables() {
        let mut game = find_game("Jackal").unwrap();
        game.launch.executable = "../evil".into();
        assert!(game.validate().is_err());
    }
    #[test]
    fn legacy_entries_keep_their_original_meaning() {
        let game = legacy_mattmc();
        game.validate().unwrap();
        assert!(!game.follows_standard());
        assert_eq!(game.legacy_persistent_directory(), "run");
        assert!(game.maintenance.contains_key("sync_up"));
        // Round-tripping an installed manifest must not invent schema 2 fields.
        let json = serde_json::to_value(&game).unwrap();
        assert!(json.get("layout").is_none() && json.get("actions").is_none());
    }
    #[test]
    fn minimal_standard_entry_is_valid() {
        standard_game(json!({})).validate().unwrap();
    }
    #[test]
    fn standard_entry_validation_rejects_mistakes() {
        for extra in [
            json!({"standard_version": null}),
            json!({"standard_version": 99}),
            json!({"persistent_directory": "run"}),
            json!({"maintenance": {"backup": {"path": "backup.sh"}}}),
            json!({"layout": {"save": ["../outside"]}}),
            json!({"layout": {"save": [".basalt/state"]}}),
            json!({"layout": {"app": []}}),
            json!({"actions": {"repair": false}}),
            json!({"actions": {"repair": {"label": "Repair"}}}),
            json!({"actions": {"backup": {"options": {"keep": 0}}}}),
            json!({"actions": {"backup": {"options": {"backup_first": true}}}}),
            json!({"actions": {"sync": {"options": {"include": ["app"]}}}}),
            json!({"actions": {"update": {"script": {"path": "u.sh"}, "options": {}}}}),
            json!({"actions": {"Bad Name": {"script": {"path": "x.sh"}}}}),
        ] {
            let accepted = try_standard_game(extra.clone())
                .is_ok_and(|game| game.validate().is_ok());
            assert!(!accepted, "accepted {extra}");
        }
    }
    #[test]
    fn archive_root_must_be_a_single_folder_name() {
        let with_root = |root: &str| {
            standard_game(json!({"source": {
                "kind": "github_release",
                "repository": "example/Sample",
                "asset_prefix": "Sample-",
                "asset_suffix": ".zip",
                "archive_root": root
            }}))
        };
        for valid in ["MattMC", "Sample-1.0_x64", "v.2"] {
            with_root(valid).validate().unwrap();
        }
        for invalid in ["", ".", "..", "a..b", "a/b", "/abs", "Matt MC", "a\\b", "../x"] {
            let error = with_root(invalid).validate().unwrap_err().to_string();
            assert!(error.contains("archive_root"), "{invalid:?}: {error}");
        }
        // Schema 1 keeps its automatic wrapper detection and has no archive_root.
        let mut legacy = find_game("Jackal").unwrap();
        legacy.source.archive_root = Some("Jackal".into());
        assert!(legacy.validate().unwrap_err().to_string().contains("schema_version 2"));
    }
    #[test]
    fn bundled_mattmc_entry_follows_the_standard() {
        let game: GameDefinition = serde_json::from_str(include_str!(
            "../../../resources/games/mattmc.json"
        ))
        .unwrap();
        game.validate().unwrap();
        assert!(game.follows_standard());
        assert_eq!(game.source.archive_root.as_deref(), Some("MattMC"));
        assert!(game.actions.is_empty() && game.layout.is_none());
    }
    #[test]
    fn unknown_option_fields_are_rejected_at_parse_time() {
        let value = json!({"label": "x", "colour": "red"});
        assert!(serde_json::from_value::<ActionConfig>(value).is_err());
    }
}
