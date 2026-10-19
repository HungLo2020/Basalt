//! Plain-Rust conversion of core data into the JSON the QML pages bind to.
//!
//! Kept free of Qt types so it can be unit-tested without a GUI.

use std::collections::HashSet;

use basalt_core::{self as core, GameEntry, Playlist, RunnerKind};
use serde::Serialize;

const STEAM_CATEGORY: &str = "Steam";
const MY_GAMES_CATEGORY: &str = "My Games";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    pub name: String,
    pub runner: &'static str,
    pub target: String,
    pub category: String,
    /// Sorts categories: Steam, My Games, then catalog games.
    pub category_order: String,
    pub artwork_key: String,
    pub favorite: bool,
    pub is_mattmc: bool,
    pub catalog_id: String,
    pub actions: Vec<ActionView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionView {
    pub name: String,
    pub label: String,
    /// Restore: the page asks which backup first.
    pub needs_snapshot: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupView {
    pub id: String,
    pub reason: String,
    pub files: usize,
    pub created_unix: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistView {
    pub name: String,
    pub games: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallTileView {
    pub key: String,
    pub title: String,
    pub description: String,
    /// "game" or "core".
    pub kind: &'static str,
    pub system: String,
    pub supports_save_sync: bool,
    pub category: &'static str,
    pub artwork_key: String,
}

pub fn games_json(games: &[GameEntry], playlists: &[Playlist]) -> String {
    let favorites: HashSet<&str> = playlists
        .iter()
        .find(|playlist| playlist.name == core::FAVORITES_PLAYLIST_NAME)
        .map(|playlist| playlist.game_names.iter().map(String::as_str).collect())
        .unwrap_or_default();

    let catalog = core::catalog_games().unwrap_or_default();
    let views: Vec<GameView> = games
        .iter()
        .map(|game| {
            let (category, category_order) = game_category(game);
            GameView {
                name: game.name.clone(),
                runner: game.runner_kind.as_str(),
                target: game.launch_target.clone(),
                category_order,
                category,
                artwork_key: core::ArtworkRequest::for_game(game)
                    .map(|request| request.key)
                    .unwrap_or_default(),
                favorite: favorites.contains(game.name.as_str()),
                is_mattmc: game.name == "MattMC",
                catalog_id: catalog
                    .iter()
                    .find(|g| g.name == game.name)
                    .map(|g| g.id.clone())
                    .unwrap_or_default(),
                actions: catalog
                    .iter()
                    .find(|g| g.name == game.name)
                    // A standard game still registered from an older installation has no
                    // actions until it is reinstalled.
                    .filter(|g| !g.follows_standard() || game.runner_kind == RunnerKind::Catalog)
                    .map(|g| {
                        core::game_actions(g)
                            .into_iter()
                            // A schema 1 "sync" script prompts for its direction, which the GUI
                            // cannot answer; sync_up and sync_down cover it.
                            .filter(|a| !(a.legacy && a.name == "sync"))
                            .map(|a| ActionView {
                                needs_snapshot: a.needs_snapshot(),
                                name: a.name,
                                label: a.label,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect();

    to_json(&views)
}

pub fn playlists_json(playlists: &[Playlist]) -> String {
    let views: Vec<PlaylistView> = playlists
        .iter()
        .map(|playlist| PlaylistView {
            name: playlist.name.clone(),
            games: playlist.game_names.clone(),
        })
        .collect();

    to_json(&views)
}

pub fn install_tiles_json() -> String {
    let tiles: Vec<InstallTileView> = core::catalog_games()
        .unwrap_or_default()
        .into_iter()
        .map(|game| InstallTileView {
            key: game.id.clone(),
            title: game.name.clone(),
            description: format!(
                "Install the latest release of {} from {}. Existing game data is preserved.",
                game.name, game.source.repository
            ),
            kind: "game",
            system: String::new(),
            supports_save_sync: false,
            category: MY_GAMES_CATEGORY,
            artwork_key: if game.id == "MattMC" {
                core::ArtworkRequest::mattmc().key
            } else {
                String::new()
            },
        })
        .collect();

    to_json(&tiles)
}

pub fn backups_json(game_id: &str) -> String {
    let views: Vec<BackupView> = core::list_backups(game_id)
        .unwrap_or_default()
        .into_iter()
        .map(|backup| BackupView {
            id: backup.id,
            reason: backup.reason.replace('-', " "),
            files: backup.files,
            created_unix: backup.created_unix,
        })
        .collect();
    to_json(&views)
}

/// Category name and sort key.
fn game_category(game: &GameEntry) -> (String, String) {
    match game.runner_kind {
        RunnerKind::Steam => (STEAM_CATEGORY.to_string(), "0".to_string()),
        RunnerKind::Bash | RunnerKind::Catalog => (MY_GAMES_CATEGORY.to_string(), "1".to_string()),
    }
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {

    use serde_json::Value;

    use super::*;

    fn game(name: &str, runner_kind: RunnerKind, launch_target: &str) -> GameEntry {
        GameEntry {
            name: name.to_string(),
            runner_kind,
            launch_target: launch_target.to_string(),
        }
    }

    #[test]
    fn games_json_categorizes_and_marks_favorites() {
        let games = vec![
            game("Doom", RunnerKind::Steam, "2280"),
            game("MattMC", RunnerKind::Bash, "/g/run-mattmc.sh"),
        ];
        let playlists = vec![Playlist {
            name: core::FAVORITES_PLAYLIST_NAME.to_string(),
            game_names: vec!["Doom".to_string()],
        }];

        let json: Value = serde_json::from_str(&games_json(&games, &playlists)).unwrap();

        assert_eq!(json[0]["category"], "Steam");
        assert_eq!(json[0]["favorite"], true);
        assert_eq!(json[0]["artworkKey"], "steam:2280");
        assert_eq!(json[1]["category"], "My Games");
        assert_eq!(json[1]["isMattmc"], true);
        // MattMC follows the standard, but this old run-mattmc.sh registration offers no
        // actions until it is reinstalled.
        assert_eq!(json[1]["actions"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn standard_games_installed_by_basalt_list_their_actions() {
        let games = vec![game(
            "MattMC",
            RunnerKind::Catalog,
            "/g/MattMC/.basalt-game.json",
        )];
        let json: Value = serde_json::from_str(&games_json(&games, &[])).unwrap();
        let actions: Vec<&str> = json[0]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            actions,
            ["update", "backup", "restore", "sync", "sync_up", "sync_down", "clear_cache"]
        );
        assert_eq!(json[0]["actions"][2]["needsSnapshot"], true);
    }

    #[test]
    fn install_tiles_have_expected_labels() {
        let json: Value = serde_json::from_str(&install_tiles_json()).unwrap();
        let tiles = json.as_array().unwrap();

        assert_eq!(tiles[0]["key"], "MattMC");
        assert_eq!(tiles.len(), 3);
    }
}
