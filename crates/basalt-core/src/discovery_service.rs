use super::discovery;
use super::error::CoreResult;
use super::playlist_service;
use super::storage::with_data_lock;
use super::{ALL_DISCOVER_RUNNERS, DiscoverReport, DiscoverRunner, SteamDiscoverReport};

pub fn discover_games() -> CoreResult<DiscoverReport> {
    discover_with_runners(&ALL_DISCOVER_RUNNERS)
}

pub fn discover_with_runners(runners: &[DiscoverRunner]) -> CoreResult<DiscoverReport> {
    with_data_lock(|| discover_with_runners_locked(runners))
}

fn discover_with_runners_locked(runners: &[DiscoverRunner]) -> CoreResult<DiscoverReport> {
    let mut should_run_mattmc = false;
    let mut should_run_steam = false;
    let mut should_run_emulators = false;

    for runner in runners {
        match runner {
            DiscoverRunner::Mattmc => should_run_mattmc = true,
            DiscoverRunner::Steam => should_run_steam = true,
            DiscoverRunner::Emulators => should_run_emulators = true,
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

    let emulators = if should_run_emulators {
        Some(discovery::emulators::discover_emulator_entries()?)
    } else {
        None
    };

    playlist_service::sync_automatic_playlists()?;

    Ok(DiscoverReport {
        mattmc,
        steam,
        emulators,
    })
}
