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
    /// Sorts categories: Steam, My Games, then emulator systems.
    pub category_order: String,
    pub artwork_key: String,
    pub favorite: bool,
    pub is_mattmc: bool,
    pub catalog_id: String,
    pub maintenance_actions: Vec<String>,
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

    let catalog = core::bundled_games().unwrap_or_default();
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
                is_mattmc: game.is_mattmc(),
                catalog_id: catalog
                    .iter()
                    .find(|g| g.name == game.name)
                    .map(|g| g.id.clone())
                    .unwrap_or_default(),
                maintenance_actions: catalog
                    .iter()
                    .find(|g| g.name == game.name)
                    .map(|g| {
                        g.maintenance
                            .keys()
                            .filter(|a| a.as_str() != "sync")
                            .cloned()
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
    let tiles: Vec<InstallTileView> = core::bundled_games()
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
            artwork_key: if game.id == core::MATTMC_GAME_NAME {
                core::ArtworkRequest::mattmc().key
            } else {
                String::new()
            },
        })
        .collect();

    to_json(&tiles)
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
            game(core::MATTMC_GAME_NAME, RunnerKind::Bash, "/g/run-mattmc.sh"),
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
        assert_eq!(json[2]["category"], "GBA");
        assert_eq!(json[2]["categoryOrder"], "2-GBA");
        assert_eq!(json[2]["favorite"], false);
    }

    #[test]
    fn install_tiles_have_expected_labels() {
        let json: Value = serde_json::from_str(&install_tiles_json()).unwrap();
        let tiles = json.as_array().unwrap();

        assert_eq!(tiles[0]["key"], "MattMC");
        let atari = tiles
            .iter()
            .find(|tile| tile["system"] == "atari2600")
            .unwrap();
        assert_eq!(atari["title"], "Atari 2600 Core");
        assert_eq!(
            atari["description"],
            "RetroArch Stella core for Atari 2600 ROMs."
        );
        assert_eq!(atari["supportsSaveSync"], false);
    }
}
