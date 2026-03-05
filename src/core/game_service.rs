use super::error::{CoreError, CoreResult};
use super::playlist_service;
use super::registry;
use super::runners;
use super::GameEntry;

pub fn add_game(name: &str, raw_script_path: &str) -> CoreResult<()> {
    if name.is_empty() {
        return Err(CoreError::new("Game name cannot be empty"));
    }

    if name.contains('\t') || name.contains('\n') {
        return Err(CoreError::new("Game name cannot contain tabs or newlines"));
    }

    if is_name_blacklisted(name)? {
        return Err(CoreError::new(format!("Game name '{}' is blacklisted", name)));
    }

    let resolved_target = runners::resolve_add_target(raw_script_path)?;

    let mut entries = registry::load_entries()?;
    if entries.iter().any(|entry| entry.name == name) {
        return Err(CoreError::new(format!("A game with name '{}' already exists", name)));
    }

    if entries
        .iter()
        .any(|entry| {
            entry.runner_kind == resolved_target.runner_kind
                && entry.launch_target == resolved_target.launch_target
        })
    {
        return Err(CoreError::new(format!(
            "A game with target '{}' already exists",
            resolved_target.launch_target
        )));
    }

    entries.push(GameEntry {
        name: name.to_string(),
        runner_kind: resolved_target.runner_kind,
        launch_target: resolved_target.launch_target,
    });

    registry::save_entries(&entries)?;
    Ok(())
}

pub fn list_games() -> CoreResult<Vec<GameEntry>> {
    let blacklisted_names = registry::load_blacklisted_names()?;

    Ok(registry::load_entries()?
        .into_iter()
        .filter(|entry| !blacklisted_names.contains(&entry.name.to_lowercase()))
        .collect())
}

pub fn remove_game(name: &str) -> CoreResult<()> {
    if name.is_empty() {
        return Err(CoreError::new("Game name cannot be empty"));
    }

    let mut entries = registry::load_entries()?;
    let original_len = entries.len();

    entries.retain(|entry| entry.name != name);

    if entries.len() == original_len {
        return Err(CoreError::new(format!("No game found with name '{}'", name)));
    }

    registry::save_entries(&entries)?;
    playlist_service::remove_game_from_all_playlists(name)?;
    Ok(())
}

pub fn remove_all_games() -> CoreResult<usize> {
    let entries = registry::load_entries()?;
    let removed_count = entries.len();
    let removed_names: Vec<String> = entries.into_iter().map(|entry| entry.name).collect();

    registry::save_entries(&[])?;
    playlist_service::remove_games_from_all_playlists(&removed_names)?;

    Ok(removed_count)
}

pub fn add_game_to_playlist(playlist_name: &str, game_name: &str) -> CoreResult<()> {
    ensure_game_exists(game_name)?;
    playlist_service::add_game_to_playlist(playlist_name, game_name)
}

pub fn remove_game_from_playlist(playlist_name: &str, game_name: &str) -> CoreResult<()> {
    ensure_game_exists(game_name)?;
    playlist_service::remove_game_from_playlist(playlist_name, game_name)
}

pub fn list_playlists() -> CoreResult<Vec<super::types::Playlist>> {
    playlist_service::list_playlists()
}

pub fn launch_game(name: &str) -> CoreResult<()> {
    if name.is_empty() {
        return Err(CoreError::new("Game name cannot be empty"));
    }

    let entries = registry::load_entries()?;
    let entry = entries
        .into_iter()
        .find(|game| game.name == name)
        .ok_or_else(|| CoreError::new(format!("No game found with name '{}'", name)))?;

    runners::launch(entry.runner_kind, &entry.launch_target)?;
    Ok(())
}

fn is_name_blacklisted(name: &str) -> CoreResult<bool> {
    let blacklisted_names = registry::load_blacklisted_names()?;
    Ok(blacklisted_names.contains(&name.to_lowercase()))
}

fn ensure_game_exists(game_name: &str) -> CoreResult<()> {
    if game_name.is_empty() {
        return Err(CoreError::new("Game name cannot be empty"));
    }

    let entries = registry::load_entries()?;
    if entries.iter().any(|entry| entry.name == game_name) {
        Ok(())
    } else {
        Err(CoreError::new(format!(
            "No game found with name '{}'",
            game_name
        )))
    }
}
