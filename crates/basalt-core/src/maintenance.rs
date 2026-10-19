//! Manifest-driven game actions; no game-specific dispatch.
//!
//! Schema 1 entries expose exactly their `maintenance` scripts, run as before. Schema 2 entries
//! get the STANDARDS.md built-in actions, adjusted by their `actions` overrides, plus any custom
//! script actions they declare.
use crate::{
    catalog::{self, ActionOptions, ActionSetting, GameDefinition, MaintenanceCommand},
    catalog_launch::contained_path,
    error::{CoreError, CoreResult},
    game_data::{self, Installation, OperationGuard, SyncMode, SyncSettings},
    game_layout::DataCategory,
    platform,
    progress::Progress,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinAction {
    Update,
    Backup,
    Restore,
    Sync,
    SyncUp,
    SyncDown,
    ClearCache,
}

impl BuiltinAction {
    pub const ALL: [Self; 7] = [
        Self::Update,
        Self::Backup,
        Self::Restore,
        Self::Sync,
        Self::SyncUp,
        Self::SyncDown,
        Self::ClearCache,
    ];

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Update => "update",
            Self::Backup => "backup",
            Self::Restore => "restore",
            Self::Sync => "sync",
            Self::SyncUp => "sync_up",
            Self::SyncDown => "sync_down",
            Self::ClearCache => "clear_cache",
        }
    }

    fn default_label(self) -> &'static str {
        match self {
            Self::Update => "Update",
            Self::Backup => "Back up",
            Self::Restore => "Restore backup",
            Self::Sync => "Sync",
            Self::SyncUp => "Sync up",
            Self::SyncDown => "Sync down",
            Self::ClearCache => "Clear cache",
        }
    }

    pub(crate) fn validate_options(self, options: &ActionOptions) -> Result<(), String> {
        let data_categories = |include: &Option<Vec<String>>| match include {
            None => Ok(()),
            Some(list) if list.is_empty() => Err("include cannot be empty".to_string()),
            Some(list) => list.iter().try_for_each(|name| match DataCategory::parse(name) {
                Some(DataCategory::Save | DataCategory::SharedConfig | DataCategory::Config) => {
                    Ok(())
                }
                _ => Err(format!(
                    "include may only list save, shared_config and config, not {name}"
                )),
            }),
        };
        let unsupported = |what: &str| Err(format!("{what} is not an option of {}", self.name()));
        match self {
            Self::Update => {
                if options.keep.is_some() {
                    return unsupported("keep");
                }
                if options.include.is_some() {
                    return unsupported("include");
                }
                Ok(())
            }
            Self::Backup => {
                if options.backup_first.is_some() {
                    return unsupported("backup_first");
                }
                if options.keep == Some(0) {
                    return Err("keep must be at least 1".into());
                }
                data_categories(&options.include)
            }
            Self::Sync | Self::SyncUp | Self::SyncDown => {
                if options.keep.is_some() {
                    return unsupported("keep");
                }
                if options.backup_first.is_some() {
                    return unsupported("backup_first");
                }
                data_categories(&options.include)
            }
            Self::Restore | Self::ClearCache => {
                if options.keep.is_some() || options.include.is_some() || options.backup_first.is_some()
                {
                    return Err(format!("{} has no options", self.name()));
                }
                Ok(())
            }
        }
    }
}

/// One action a game offers, as the CLI and GUI present it.
#[derive(Debug, Clone)]
pub struct GameAction {
    pub name: String,
    pub label: String,
    /// The built-in behavior, unless a script replaces it.
    pub builtin: Option<BuiltinAction>,
    pub script: Option<MaintenanceCommand>,
    pub options: ActionOptions,
    /// Schema 1 script (runs attached to the terminal, as before the standard).
    pub legacy: bool,
}

impl GameAction {
    /// Restore needs the user to pick a snapshot.
    pub fn needs_snapshot(&self) -> bool {
        self.script.is_none() && self.builtin == Some(BuiltinAction::Restore)
    }
}

fn label_for(name: &str) -> String {
    let words = name.replace(['_', '-'], " ");
    let mut chars = words.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

/// The actions a catalog entry offers, built-ins first.
pub fn game_actions(definition: &GameDefinition) -> Vec<GameAction> {
    if !definition.follows_standard() {
        return definition
            .maintenance
            .iter()
            .map(|(name, command)| GameAction {
                name: name.clone(),
                label: label_for(name),
                builtin: None,
                script: Some(command.clone()),
                options: ActionOptions::default(),
                legacy: true,
            })
            .collect();
    }
    let mut actions = Vec::new();
    for builtin in BuiltinAction::ALL {
        let config = match definition.actions.get(builtin.name()) {
            Some(ActionSetting::Enabled(false)) => continue,
            Some(ActionSetting::Configured(config)) => config.clone(),
            _ => Default::default(),
        };
        actions.push(GameAction {
            name: builtin.name().to_string(),
            label: config
                .label
                .unwrap_or_else(|| builtin.default_label().to_string()),
            builtin: Some(builtin),
            script: config.script,
            options: config.options.unwrap_or_default(),
            legacy: false,
        });
    }
    for (name, setting) in &definition.actions {
        if BuiltinAction::parse(name).is_some() {
            continue;
        }
        if let ActionSetting::Configured(config) = setting
            && let Some(script) = &config.script
        {
            actions.push(GameAction {
                name: name.clone(),
                label: config.label.clone().unwrap_or_else(|| label_for(name)),
                builtin: None,
                script: Some(script.clone()),
                options: ActionOptions::default(),
                legacy: false,
            });
        }
    }
    actions
}

pub fn list_game_actions(game_id: &str) -> CoreResult<Vec<GameAction>> {
    Ok(game_actions(&catalog::find_game(game_id)?))
}

fn categories(include: &Option<Vec<String>>, default: &[DataCategory]) -> Vec<DataCategory> {
    include
        .as_ref()
        .map(|names| names.iter().filter_map(|n| DataCategory::parse(n)).collect())
        .unwrap_or_else(|| default.to_vec())
}

/// Backup settings used by manual backups and the automatic ones taken before updates,
/// restores and sync downs; they come from the `backup` action's options even when that action
/// is disabled or scripted.
pub(crate) fn backup_settings(definition: &GameDefinition) -> (Vec<DataCategory>, u32) {
    let options = match definition.actions.get(BuiltinAction::Backup.name()) {
        Some(ActionSetting::Configured(config)) => config.options.clone().unwrap_or_default(),
        _ => ActionOptions::default(),
    };
    (
        categories(
            &options.include,
            &[DataCategory::Save, DataCategory::Config],
        ),
        options.keep.unwrap_or(game_data::DEFAULT_BACKUP_KEEP),
    )
}

/// Runs a game action and returns a message describing the outcome. `snapshot` selects the
/// backup for `restore`.
pub fn run_game_action(
    game_id: &str,
    action: &str,
    snapshot: Option<&str>,
    progress: &Progress,
) -> CoreResult<String> {
    let definition = catalog::find_game(game_id)?;
    let selected = game_actions(&definition)
        .into_iter()
        .find(|candidate| candidate.name == action)
        .ok_or_else(|| {
            CoreError::InvalidInput(format!(
                "{} does not support the action {action}",
                definition.name
            ))
        })?;
    let installation = Installation::locate(game_id)?;
    let name = definition.name.clone();

    if selected.legacy {
        let command = selected
            .script
            .as_ref()
            .ok_or_else(|| CoreError::new("Legacy action has no script"))?;
        let script = contained_path(&installation.root, &command.path)
            .map_err(|e| CoreError::new(format!("Missing maintenance script: {e}")))?;
        platform::run_maintenance_script(
            &script,
            &installation.root,
            &command.arguments,
            command.stdin.as_deref(),
        )?;
        return Ok(format!("Completed '{action}' for '{name}'"));
    }

    let _guard = OperationGuard::acquire(
        &installation,
        selected.builtin != Some(BuiltinAction::ClearCache),
    )?;

    if let Some(command) = &selected.script {
        let script = contained_path(&installation.root, &command.path)
            .map_err(|e| CoreError::new(format!("Missing script for {action}: {e}")))?;
        let environment = game_data::environment(&definition, &installation.root, Some(action))?;
        let input = match command.stdin.as_deref() {
            Some(text) => platform::ScriptInput::Text(text),
            None => platform::ScriptInput::None,
        };
        platform::run_game_script(
            &script,
            &installation.root,
            &command.arguments,
            input,
            &environment,
            progress,
        )?;
        return Ok(format!("Completed '{}' for {name}", selected.label));
    }

    let builtin = selected
        .builtin
        .ok_or_else(|| CoreError::new("Action has neither a script nor a built-in"))?;
    let (backup_categories, keep) = backup_settings(&definition);
    let sync_settings = SyncSettings {
        categories: &categories(
            &selected.options.include,
            &[DataCategory::Save, DataCategory::SharedConfig],
        ),
        backup_categories: &backup_categories,
        keep,
    };
    match builtin {
        BuiltinAction::Update => {
            let backup_first = selected.options.backup_first.unwrap_or(true);
            let report =
                crate::game_install::update_installation(&installation, backup_first, progress)?;
            Ok(format!(
                "Updated {name} to release '{}'",
                report.release_tag
            ))
        }
        BuiltinAction::Backup => {
            let snapshot = game_data::backup_installation(
                &installation,
                &backup_categories,
                keep,
                "manual",
                progress,
            )?;
            Ok(format!(
                "Backed up {name} ({} files) as {}",
                snapshot.files, snapshot.id
            ))
        }
        BuiltinAction::Restore => {
            let snapshot = snapshot.ok_or_else(|| {
                CoreError::InvalidInput(format!(
                    "Choose a backup to restore (see `basalt backups {game_id}`, then pass --snapshot <id>)"
                ))
            })?;
            game_data::restore_backup(&installation, snapshot, &backup_categories, keep, progress)
        }
        BuiltinAction::Sync => game_data::sync(&installation, SyncMode::Auto, &sync_settings, progress),
        BuiltinAction::SyncUp => game_data::sync(&installation, SyncMode::Up, &sync_settings, progress),
        BuiltinAction::SyncDown => {
            game_data::sync(&installation, SyncMode::Down, &sync_settings, progress)
        }
        BuiltinAction::ClearCache => game_data::clear_cache(&installation),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::standard_game;
    use serde_json::json;

    fn names(actions: &[GameAction]) -> Vec<&str> {
        actions.iter().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn legacy_entries_expose_only_their_scripts() {
        let actions = game_actions(&catalog::legacy_mattmc());
        assert_eq!(
            names(&actions),
            ["backup", "sync", "sync_down", "sync_up", "update"]
        );
        assert!(actions.iter().all(|a| a.legacy && a.builtin.is_none()));
    }

    #[test]
    fn standard_entries_get_every_builtin_by_default() {
        let actions = game_actions(&standard_game(json!({})));
        assert_eq!(
            names(&actions),
            ["update", "backup", "restore", "sync", "sync_up", "sync_down", "clear_cache"]
        );
        assert!(actions.iter().all(|a| a.script.is_none() && !a.legacy));
        assert!(actions[2].needs_snapshot());
    }

    #[test]
    fn overrides_disable_relabel_replace_and_add_actions() {
        let game = standard_game(json!({"actions": {
            "sync": false,
            "clear_cache": {"label": "Clear shader cache"},
            "update": {"script": {"path": "app/update.sh"}},
            "backup": {"options": {"keep": 3, "include": ["save"]}},
            "repair": {"label": "Repair install", "script": {"path": "app/repair.sh"}}
        }}));
        game.validate().unwrap();
        let actions = game_actions(&game);
        assert!(!names(&actions).contains(&"sync"));
        let find = |name: &str| actions.iter().find(|a| a.name == name).unwrap();
        assert_eq!(find("clear_cache").label, "Clear shader cache");
        assert_eq!(find("update").script.as_ref().unwrap().path, "app/update.sh");
        assert_eq!(find("repair").label, "Repair install");
        assert_eq!(backup_settings(&game), (vec![DataCategory::Save], 3));
        assert_eq!(names(&actions).last(), Some(&"repair"));
    }

    #[test]
    fn backup_defaults_cover_save_and_config() {
        assert_eq!(
            backup_settings(&standard_game(json!({}))),
            (vec![DataCategory::Save, DataCategory::Config], 10)
        );
    }
}
