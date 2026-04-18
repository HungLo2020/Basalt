use crate::core::{
    add_game, is_already_exists_error, is_blacklisted_error, CoreResult, DiscoverResult,
};
use crate::core::registry;
use crate::core::runners::RunnerKind;
use crate::platform;

const MATTMC_ENTRY_NAME: &str = "MattMC";

pub fn discover_mattmc_entry() -> CoreResult<DiscoverResult> {
    let home = platform::home_dir()?;
    let mattmc_script = home
        .join("Games")
        .join("MattMC")
        .join(platform::mattmc_launch_script_name());

    if !mattmc_script.exists() || !mattmc_script.is_file() {
        return Ok(DiscoverResult::NotFound);
    }

    let mattmc_script_str = mattmc_script
        .to_str()
        .ok_or_else(|| "MattMC script path contains invalid UTF-8".to_string())?;

    let mut entries = registry::load_entries()?;
    if let Some(existing_entry) = entries
        .iter_mut()
        .find(|entry| entry.name == MATTMC_ENTRY_NAME)
    {
        if existing_entry.runner_kind == RunnerKind::Bash
            && existing_entry.launch_target == mattmc_script_str
        {
            return Ok(DiscoverResult::AlreadyExists);
        }

        existing_entry.runner_kind = RunnerKind::Bash;
        existing_entry.launch_target = mattmc_script_str.to_string();
        registry::save_entries(&entries)?;
        return Ok(DiscoverResult::Added);
    }

    match add_game(MATTMC_ENTRY_NAME, mattmc_script_str) {
        Ok(_) => Ok(DiscoverResult::Added),
        Err(err) if is_already_exists_error(&err) || is_blacklisted_error(&err) => {
            Ok(DiscoverResult::AlreadyExists)
        }
        Err(err) => Err(err),
    }
}