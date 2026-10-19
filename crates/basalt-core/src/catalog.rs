//! Bundled, declarative game definitions. Catalog metadata is untrusted; it never runs shell code.
use serde::{Deserialize, Serialize};
use crate::error::{CoreError, CoreResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDefinition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub source: ReleaseSource,
    pub launch: LaunchDefinition,
    #[serde(default)]
    pub maintenance: std::collections::BTreeMap<String, MaintenanceCommand>,
    #[serde(default = "default_persistent")]
    pub persistent_directory: String,
}
fn default_persistent() -> String { "run".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSource {
    pub kind: String,
    pub repository: String,
    pub asset_prefix: String,
    pub asset_suffix: String,
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
fn safe_relative(value: &str) -> bool {
    use std::path::{Component, Path};
    !value.is_empty() && Path::new(value).components().all(|c| matches!(c, Component::Normal(_)))
}
impl GameDefinition {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != 1 || !safe_relative(&self.id)
            || self.id.contains('/') || self.id.contains('\\')
            || !safe_relative(&self.persistent_directory)
            || !safe_relative(&self.launch.executable)
            || (!self.launch.working_directory.is_empty() && self.launch.working_directory != "." && !safe_relative(&self.launch.working_directory))
            || self.source.kind != "github_release"
            || self.source.repository.split('/').count() != 2
            || !self.source.repository.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-./".contains(&b))
            || self.source.asset_prefix.is_empty() || self.source.asset_suffix != ".zip" {
            return Err(CoreError::InvalidInput(format!("Invalid game catalog entry: {}", self.id)));
        }
        for (action, command) in &self.maintenance {
            if action.is_empty() || !action.bytes().all(|c| c.is_ascii_lowercase() || c == b'_' || c == b'-')
                || !safe_relative(&command.path) || !command.path.ends_with(".sh") {
                return Err(CoreError::InvalidInput(format!("Invalid maintenance action {action} for {}", self.id)));
            }
        }
        Ok(())
    }
}
pub fn bundled_games() -> CoreResult<Vec<GameDefinition>> {
    let texts = [
        include_str!("../../../resources/games/mattmc.json"),
        include_str!("../../../resources/games/micromages.json"),
        include_str!("../../../resources/games/jackal.json"),
    ];
    texts.iter().map(|value| {
        let entry: GameDefinition = serde_json::from_str(value)
            .map_err(|err| CoreError::new(format!("Invalid bundled game manifest: {err}")))?;
        entry.validate()?;
        Ok(entry)
    }).collect()
}
pub fn find_game(id: &str) -> CoreResult<GameDefinition> {
    bundled_games()?.into_iter().find(|game| game.id == id)
        .ok_or_else(|| CoreError::GameNotFound(id.into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_catalog_is_valid_and_unique() {
        let list = bundled_games().unwrap();
        let mut ids = std::collections::HashSet::new();
        for item in list { assert!(ids.insert(item.id)); }
    }
    #[test]
    fn rejects_escaping_maintenance_path() {
        let mut game = find_game("MattMC").unwrap();
        game.maintenance.get_mut("backup").unwrap().path = "../evil.sh".into();
        assert!(game.validate().is_err());
    }
    #[test]
    fn rejects_escaping_executables() {
        let mut game = find_game("jackal").unwrap();
        game.launch.executable = "../evil".into();
        assert!(game.validate().is_err());
    }
}
