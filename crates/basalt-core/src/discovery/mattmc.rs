use crate::platform;
use crate::registry;
use crate::runners::RunnerKind;
use crate::{add_game, CoreResult, DiscoverResult};

const MATTMC_ENTRY_NAME: &str = "MattMC";

pub fn discover_mattmc_entry() -> CoreResult<DiscoverResult> {
    let home = platform::home_dir()?;
    let mattmc_root = home.join("Games").join("MattMC");
    let mattmc_script = platform::mattmc_launch_script_candidates()
        .iter()
        .map(|candidate| mattmc_root.join(candidate))
        .find(|candidate| candidate.exists() && candidate.is_file());

    let Some(mattmc_script) = mattmc_script else {
        return Ok(DiscoverResult::NotFound);
    };

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
        Err(err) if err.is_duplicate_game() || err.is_blacklisted() => {
            Ok(DiscoverResult::AlreadyExists)
        }
        Err(err) => Err(err),
    }
}