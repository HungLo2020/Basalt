use std::collections::BTreeMap;

use crate::{add_game, CoreResult};

/// Adds every installed Steam app to the registry. Returns (found, added, already_exists).
///
/// Library and manifest parsing is delegated to `steamlocate`, which finds native, Flatpak, and
/// Snap installs plus every library folder they reference.
pub fn discover_steam_entries() -> CoreResult<(usize, usize, usize)> {
    let installed_apps = collect_installed_apps();

    let mut steam_added = 0usize;
    let mut steam_already_exists = 0usize;

    for (appid, name) in &installed_apps {
        let steam_target = format!("steam://rungameid/{}", appid);

        match add_game(name, &steam_target) {
            Ok(_) => steam_added += 1,
            Err(err) if err.is_duplicate_game() || err.is_blacklisted() => {
                steam_already_exists += 1;
            }
            Err(err) => return Err(err),
        }
    }

    Ok((installed_apps.len(), steam_added, steam_already_exists))
}

/// App id -> name for every readable manifest. The same library can be reachable through
/// several Steam roots (e.g. `~/.steam/steam` symlinks), so apps are de-duplicated by id.
fn collect_installed_apps() -> BTreeMap<u32, String> {
    let mut apps = BTreeMap::new();

    // No Steam install is not an error: discovery just finds nothing.
    let steam_dirs = steamlocate::locate_all().unwrap_or_default();

    for steam_dir in steam_dirs {
        let Ok(libraries) = steam_dir.libraries() else {
            continue;
        };

        for library in libraries.flatten() {
            for app in library.apps().flatten() {
                let Some(name) = app.name.filter(|name| !name.trim().is_empty()) else {
                    continue;
                };
                apps.entry(app.app_id).or_insert(name);
            }
        }
    }

    apps
}
