use std::env;
use std::path::Path;

use crate::core::{add_game, is_already_exists_error, is_blacklisted_error, DiscoverResult};

const MATTMC_ENTRY_NAME: &str = "MattMC";

pub fn discover_mattmc_entry() -> Result<DiscoverResult, String> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    let mattmc_script = Path::new(&home)
        .join("Documents")
        .join("MattMC")
        .join("run-mattmc.sh");

    if !mattmc_script.exists() || !mattmc_script.is_file() {
        return Ok(DiscoverResult::NotFound);
    }

    let mattmc_script_str = mattmc_script
        .to_str()
        .ok_or_else(|| "MattMC script path contains invalid UTF-8".to_string())?;

    match add_game(MATTMC_ENTRY_NAME, mattmc_script_str) {
        Ok(_) => Ok(DiscoverResult::Added),
        Err(err) if is_already_exists_error(&err) || is_blacklisted_error(&err) => {
            Ok(DiscoverResult::AlreadyExists)
        }
        Err(err) => Err(err),
    }
}