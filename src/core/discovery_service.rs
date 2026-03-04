use super::discovery;
use super::error::{CoreError, CoreResult};
use super::{DiscoverReport, DiscoverRunner, SteamDiscoverReport, ALL_DISCOVER_RUNNERS};

pub fn discover_games() -> CoreResult<DiscoverReport> {
    discover_with_runners(&ALL_DISCOVER_RUNNERS)
}

pub fn discover_with_runners(runners: &[DiscoverRunner]) -> CoreResult<DiscoverReport> {
    let mut should_run_mattmc = false;
    let mut should_run_steam = false;

    for runner in runners {
        match runner {
            DiscoverRunner::Mattmc => should_run_mattmc = true,
            DiscoverRunner::Steam => should_run_steam = true,
        }
    }

    let mattmc = if should_run_mattmc {
        Some(discovery::mattmc::discover_mattmc_entry()?)
    } else {
        None
    };

    let steam = if should_run_steam {
        let (found, added, already_exists) = discovery::steam::discover_steam_entries()?;
        Some(SteamDiscoverReport {
            found,
            added,
            already_exists,
        })
    } else {
        None
    };

    Ok(DiscoverReport { mattmc, steam })
}

pub(crate) fn is_already_exists_error(error: &CoreError) -> bool {
    let error_message = error.message();

    (error_message.starts_with("A game with name '")
        && error_message.ends_with("' already exists"))
        || (error_message.starts_with("A game with script '")
            && error_message.ends_with("' already exists"))
        || (error_message.starts_with("A game with target '")
            && error_message.ends_with("' already exists"))
}

pub(crate) fn is_blacklisted_error(error: &CoreError) -> bool {
    let error_message = error.message();
    error_message.starts_with("Game name '") && error_message.ends_with("' is blacklisted")
}
