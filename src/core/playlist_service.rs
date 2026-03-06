use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use super::error::{CoreError, CoreResult};
use super::registry;
use super::runners::RunnerKind;
use super::types::Playlist;

const APP_DIR_NAME: &str = ".basalt";
const PLAYLISTS_FILE_NAME: &str = "playlists.tsv";
pub const FAVORITES_PLAYLIST_NAME: &str = "Favorites";
pub const STEAM_PLAYLIST_NAME: &str = "Steam";
pub const EMULATION_PLAYLIST_NAME: &str = "Emulation";

pub fn list_playlists() -> CoreResult<Vec<Playlist>> {
    sync_automatic_playlists()?;
    let mut memberships = load_playlist_memberships()?;

    let favorite_games = memberships
        .remove(FAVORITES_PLAYLIST_NAME)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let steam_games = memberships
        .remove(STEAM_PLAYLIST_NAME)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let emulation_games = memberships
        .remove(EMULATION_PLAYLIST_NAME)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut playlists = vec![Playlist {
        name: FAVORITES_PLAYLIST_NAME.to_string(),
        game_names: favorite_games,
    },
    Playlist {
        name: STEAM_PLAYLIST_NAME.to_string(),
        game_names: steam_games,
    },
    Playlist {
        name: EMULATION_PLAYLIST_NAME.to_string(),
        game_names: emulation_games,
    }];

    for (playlist_name, game_names) in memberships {
        playlists.push(Playlist {
            name: playlist_name,
            game_names: game_names.into_iter().collect(),
        });
    }

    Ok(playlists)
}

pub fn add_game_to_playlist(playlist_name: &str, game_name: &str) -> CoreResult<()> {
    if playlist_name.trim().is_empty() {
        return Err(CoreError::new("Playlist name cannot be empty"));
    }

    if game_name.trim().is_empty() {
        return Err(CoreError::new("Game name cannot be empty"));
    }

    let mut memberships = load_playlist_memberships()?;
    let canonical_playlist_name = resolve_playlist_name(playlist_name, &memberships)?;

    memberships
        .entry(canonical_playlist_name)
        .or_default()
        .insert(game_name.trim().to_string());

    save_playlist_memberships(&memberships)
}

pub fn remove_game_from_playlist(playlist_name: &str, game_name: &str) -> CoreResult<()> {
    if playlist_name.trim().is_empty() {
        return Err(CoreError::new("Playlist name cannot be empty"));
    }

    if game_name.trim().is_empty() {
        return Err(CoreError::new("Game name cannot be empty"));
    }

    let mut memberships = load_playlist_memberships()?;
    let canonical_playlist_name = resolve_playlist_name(playlist_name, &memberships)?;

    if let Some(playlist_games) = memberships.get_mut(&canonical_playlist_name) {
        playlist_games.remove(game_name.trim());
    }

    save_playlist_memberships(&memberships)
}

pub(super) fn remove_game_from_all_playlists(game_name: &str) -> CoreResult<()> {
    if game_name.is_empty() {
        return Ok(());
    }

    let mut memberships = load_playlist_memberships()?;
    let mut changed = false;

    for game_names in memberships.values_mut() {
        if game_names.remove(game_name) {
            changed = true;
        }
    }

    if changed {
        save_playlist_memberships(&memberships)?;
    }

    Ok(())
}

pub(super) fn remove_games_from_all_playlists(game_names: &[String]) -> CoreResult<()> {
    if game_names.is_empty() {
        return Ok(());
    }

    let names_to_remove: BTreeSet<&str> = game_names.iter().map(String::as_str).collect();
    let mut memberships = load_playlist_memberships()?;
    let mut changed = false;

    for playlist_games in memberships.values_mut() {
        let original_len = playlist_games.len();
        playlist_games.retain(|name| !names_to_remove.contains(name.as_str()));
        if playlist_games.len() != original_len {
            changed = true;
        }
    }

    if changed {
        save_playlist_memberships(&memberships)?;
    }

    Ok(())
}

pub(super) fn sync_automatic_playlists() -> CoreResult<()> {
    let mut memberships = load_playlist_memberships()?;
    let entries = registry::load_entries()?;

    let mut steam_games = BTreeSet::new();
    let mut emulation_games = BTreeSet::new();

    for entry in entries {
        match entry.runner_kind {
            RunnerKind::Steam => {
                steam_games.insert(entry.name);
            }
            RunnerKind::Emulator => {
                emulation_games.insert(entry.name);
            }
            RunnerKind::Bash => {}
        }
    }

    let steam_changed = memberships
        .get(STEAM_PLAYLIST_NAME)
        .map(|existing| existing != &steam_games)
        .unwrap_or(!steam_games.is_empty());
    let emulation_changed = memberships
        .get(EMULATION_PLAYLIST_NAME)
        .map(|existing| existing != &emulation_games)
        .unwrap_or(!emulation_games.is_empty());

    memberships.insert(STEAM_PLAYLIST_NAME.to_string(), steam_games);
    memberships.insert(EMULATION_PLAYLIST_NAME.to_string(), emulation_games);

    if steam_changed || emulation_changed {
        save_playlist_memberships(&memberships)?;
    }

    Ok(())
}

fn playlists_file_path() -> CoreResult<std::path::PathBuf> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    Ok(Path::new(&home)
        .join(APP_DIR_NAME)
        .join(PLAYLISTS_FILE_NAME))
}

fn ensure_app_dir() -> CoreResult<()> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    let app_dir = Path::new(&home).join(APP_DIR_NAME);
    fs::create_dir_all(&app_dir)
        .map_err(|err| format!("Failed to create app directory: {}", err))?;
    Ok(())
}

fn load_playlist_memberships() -> CoreResult<BTreeMap<String, BTreeSet<String>>> {
    let path = playlists_file_path()?;
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let file = fs::File::open(&path)
        .map_err(|err| format!("Failed to open playlists file: {}", err))?;
    let reader = BufReader::new(file);

    let mut memberships: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|err| format!("Failed to read playlists file: {}", err))?;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.splitn(2, '\t');
        let Some(playlist_name) = parts.next() else {
            continue;
        };
        let Some(game_name) = parts.next() else {
            continue;
        };

        let playlist_name = playlist_name.trim();
        let game_name = game_name.trim();

        if playlist_name.is_empty() || game_name.is_empty() {
            continue;
        }

        memberships
            .entry(playlist_name.to_string())
            .or_default()
            .insert(game_name.to_string());
    }

    Ok(memberships)
}

fn save_playlist_memberships(memberships: &BTreeMap<String, BTreeSet<String>>) -> CoreResult<()> {
    ensure_app_dir()?;
    let path = playlists_file_path()?;

    let mut file = fs::File::create(&path)
        .map_err(|err| format!("Failed to open playlists file for writing: {}", err))?;

    for (playlist_name, game_names) in memberships {
        for game_name in game_names {
            let line = format!("{}\t{}\n", playlist_name, game_name);
            file.write_all(line.as_bytes())
                .map_err(|err| format!("Failed to write playlists file: {}", err))?;
        }
    }

    Ok(())
}

fn resolve_playlist_name(
    playlist_name: &str,
    memberships: &BTreeMap<String, BTreeSet<String>>,
) -> CoreResult<String> {
    let trimmed = playlist_name.trim();

    if trimmed.eq_ignore_ascii_case(FAVORITES_PLAYLIST_NAME) {
        return Ok(FAVORITES_PLAYLIST_NAME.to_string());
    }
    if trimmed.eq_ignore_ascii_case(STEAM_PLAYLIST_NAME) {
        return Ok(STEAM_PLAYLIST_NAME.to_string());
    }
    if trimmed.eq_ignore_ascii_case(EMULATION_PLAYLIST_NAME) {
        return Ok(EMULATION_PLAYLIST_NAME.to_string());
    }

    for existing_name in memberships.keys() {
        if existing_name.eq_ignore_ascii_case(trimmed) {
            return Ok(existing_name.clone());
        }
    }

    Err(CoreError::new(format!(
        "Playlist '{}' does not exist",
        trimmed
    )))
}
