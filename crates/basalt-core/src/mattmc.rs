//! Everything MattMC-specific: how Basalt recognises and locates MattMC, and the scripts that
//! ship with it (sync, update, backup). Discovery, install, artwork, and both front ends go
//! through these definitions.

use std::path::{Path, PathBuf};

use crate::GameEntry;
use crate::error::{CoreError, CoreResult};
use crate::platform;
use crate::registry;
use crate::runners::RunnerKind;

/// The library entry name MattMC is registered under by discovery and install. MattMC features
/// apply to the entry with exactly this name.
pub const MATTMC_GAME_NAME: &str = "MattMC";

const MATTMC_DIR_NAME: &str = "MattMC";
/// Launch scripts discovery looks for in the install directory.
pub(crate) const MATTMC_LAUNCH_SCRIPT_CANDIDATES: &[&str] = &["run-mattmc.sh"];
/// Release ZIPs are named `MattMC-Client-<version>-<suffix>...zip`.
pub(crate) const MATTMC_RELEASE_ZIP_SUFFIX: &str = "linux-x64";

// Scripts shipped next to MattMC's launch script.
const SYNC_SCRIPT_NAME: &str = "SyncGameData.sh";
const UPDATE_SCRIPT_NAME: &str = "update-mattmc.sh";
const BACKUP_SCRIPT_NAME: &str = "backup.sh";

/// Where MattMC is installed and discovered: `~/Games/MattMC`.
pub fn mattmc_install_dir() -> CoreResult<PathBuf> {
    Ok(platform::home_dir()?.join("Games").join(MATTMC_DIR_NAME))
}

impl GameEntry {
    /// Whether this library entry is MattMC.
    pub fn is_mattmc(&self) -> bool {
        self.name == MATTMC_GAME_NAME
    }
}

/// Syncs MattMC's game data up to the remote copy.
pub fn sync_mattmc_up() -> CoreResult<()> {
    // The sync script asks for a direction on standard input.
    run_script_with_input(SYNC_SCRIPT_NAME, "up\n")
}

/// Syncs MattMC's game data down from the remote copy.
pub fn sync_mattmc_down() -> CoreResult<()> {
    run_script_with_input(SYNC_SCRIPT_NAME, "down\n")
}

/// Runs the sync script attached to the terminal, so it asks the user which direction to
/// sync. Only for terminal use; front ends without a terminal use [`sync_mattmc_up`] or
/// [`sync_mattmc_down`].
pub fn sync_mattmc_interactive() -> CoreResult<()> {
    platform::launch_script(&script_path(SYNC_SCRIPT_NAME)?)
}

pub fn update_mattmc() -> CoreResult<()> {
    platform::launch_script(&script_path(UPDATE_SCRIPT_NAME)?)
}

pub fn backup_mattmc() -> CoreResult<()> {
    platform::launch_script(&script_path(BACKUP_SCRIPT_NAME)?)
}

fn run_script_with_input(script_name: &str, stdin_content: &str) -> CoreResult<()> {
    platform::launch_script_with_stdin(&script_path(script_name)?, stdin_content)
}

/// Path of a script that ships next to the MattMC library entry's launch script.
fn script_path(script_name: &str) -> CoreResult<String> {
    let entry = registry::load_entries()?
        .into_iter()
        .find(GameEntry::is_mattmc)
        .ok_or_else(|| CoreError::GameNotFound(MATTMC_GAME_NAME.to_string()))?;

    if entry.runner_kind != RunnerKind::Bash {
        return Err(CoreError::new(format!(
            "Game '{}' does not use the script runner, so its scripts can't be found",
            MATTMC_GAME_NAME
        )));
    }

    let launch_script_path = Path::new(&entry.launch_target);
    if !launch_script_path.is_file() {
        return Err(CoreError::new(format!(
            "Saved script path does not exist or is not a file: {}",
            entry.launch_target
        )));
    }

    let script_dir = launch_script_path.parent().ok_or_else(|| {
        CoreError::new(format!(
            "Could not determine parent directory for script: {}",
            entry.launch_target
        ))
    })?;

    let script = script_dir.join(script_name);
    if !script.is_file() {
        return Err(CoreError::new(format!(
            "No script found for '{}' at {}",
            MATTMC_GAME_NAME,
            script.display()
        )));
    }

    script
        .to_str()
        .ok_or_else(|| CoreError::new("Script path contains invalid UTF-8"))
        .map(|value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RunnerKind;

    #[test]
    fn only_the_exact_entry_name_is_mattmc() {
        let entry = |name: &str| GameEntry {
            name: name.to_string(),
            runner_kind: RunnerKind::Bash,
            launch_target: "/g/run-mattmc.sh".to_string(),
        };

        assert!(entry("MattMC").is_mattmc());
        assert!(!entry("mattmc").is_mattmc());
        assert!(!entry("MattMC (2)").is_mattmc());
    }
}
