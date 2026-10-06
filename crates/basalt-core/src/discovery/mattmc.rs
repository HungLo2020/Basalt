use crate::error::CoreError;
use crate::mattmc::{MATTMC_GAME_NAME, MATTMC_LAUNCH_SCRIPT_CANDIDATES, mattmc_install_dir};
use crate::registry;
use crate::runners::RunnerKind;
use crate::{CoreResult, DiscoverResult, add_game};

pub fn discover_mattmc_entry() -> CoreResult<DiscoverResult> {
    let mattmc_root = mattmc_install_dir()?;
    let mattmc_script = MATTMC_LAUNCH_SCRIPT_CANDIDATES
        .iter()
        .map(|candidate| mattmc_root.join(candidate))
        .find(|candidate| candidate.exists() && candidate.is_file());

    let Some(mattmc_script) = mattmc_script else {
        return Ok(DiscoverResult::NotFound);
    };

    let mattmc_script_str = mattmc_script
        .to_str()
        .ok_or_else(|| CoreError::new("MattMC script path contains invalid UTF-8".to_string()))?;

    let mut entries = registry::load_entries()?;
    if let Some(existing_entry) = entries
        .iter_mut()
        .find(|entry| entry.name == MATTMC_GAME_NAME)
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

    match add_game(MATTMC_GAME_NAME, mattmc_script_str) {
        Ok(_) => Ok(DiscoverResult::Added),
        Err(err) if err.is_duplicate_game() || err.is_blacklisted() => {
            Ok(DiscoverResult::AlreadyExists)
        }
        Err(err) => Err(err),
    }
}
