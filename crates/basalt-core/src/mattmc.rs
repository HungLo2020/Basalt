//! How Basalt recognises and locates MattMC. Everything MattMC-specific (discovery, install,
//! artwork, and the sync/update/backup scripts) goes through these definitions.

use std::path::PathBuf;

use crate::GameEntry;
use crate::error::CoreResult;
use crate::platform;

/// The library entry name MattMC is registered under by discovery and install. MattMC features
/// apply to the entry with exactly this name.
pub const MATTMC_GAME_NAME: &str = "MattMC";

const MATTMC_DIR_NAME: &str = "MattMC";

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
