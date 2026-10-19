//! Discover installed catalog games and Steam apps.
mod steam;
use crate::{CoreResult, DiscoverReport, DiscoverResult, DiscoverRunner, SteamDiscoverReport, ALL_DISCOVER_RUNNERS};
use crate::{catalog, catalog_launch, libraries, registry, runners::RunnerKind, playlist_service, storage::with_data_lock};
pub fn discover_games() -> CoreResult<DiscoverReport> { discover_with_runners(&ALL_DISCOVER_RUNNERS) }
pub fn discover_with_runners(runners: &[DiscoverRunner]) -> CoreResult<DiscoverReport> {
    with_data_lock(|| {
        let mut report = DiscoverReport { catalog: None, steam: None };
        if runners.contains(&DiscoverRunner::Catalog) {
            let mut found = false;
            let mut entries = registry::load_entries()?;
            for game in catalog::bundled_games()? {
                if entries.iter().any(|e| e.name == game.name) { found = true; continue; }
                for library in libraries::list_libraries()? {
                    let root = library.path.join(&game.id);
                    let manifest = root.join(catalog_launch::MANIFEST);
                    if manifest.is_file() && catalog_launch::validate_launch(&root, &game).is_ok() {
                        entries.push(crate::GameEntry { name: game.name.clone(), runner_kind: RunnerKind::Catalog, launch_target: manifest.to_string_lossy().into_owned() });
                        found = true;
                        break;
                    }
                    // Existing MattMC installs predate the generic manifest.
                    if game.id == "MattMC" && root.join("run-mattmc.sh").is_file() {
                        entries.push(crate::GameEntry { name: game.name.clone(), runner_kind: RunnerKind::Bash, launch_target: root.join("run-mattmc.sh").to_string_lossy().into_owned() });
                        found = true;
                        break;
                    }
                }
            }
            registry::save_entries(&entries)?;
            report.catalog = Some(if found { DiscoverResult::Added } else { DiscoverResult::NotFound });
        }
        if runners.contains(&DiscoverRunner::Steam) {
            let (found, added, already_exists) = steam::discover_steam_entries()?;
            report.steam = Some(SteamDiscoverReport { found, added, already_exists });
        }
        playlist_service::sync_automatic_playlists()?;
        Ok(report)
    })
}
