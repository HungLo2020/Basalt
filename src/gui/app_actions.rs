use std::collections::HashSet;

use crate::cli;
use crate::core::{self, DiscoverResult, GameEntry};

use super::app::BasaltApp;
use super::search;
use super::top_bar::{PlaylistSelection, TopBarActions};

impl BasaltApp {
    pub(super) fn apply_top_bar_actions(&mut self, actions: TopBarActions) {
        if let Some(tab) = actions.switch_to_tab {
            self.active_tab = tab;
        }

        if let Some(selection) = actions.select_playlist {
            match selection {
                PlaylistSelection::AllGames => {
                    self.selected_playlist = None;
                }
                PlaylistSelection::Named(name) => {
                    self.selected_playlist = Some(name);
                }
            }
        }

        if actions.trigger_discover {
            self.discover_games();
        }
        if actions.trigger_refresh {
            self.refresh_games();
            self.status_message = "Game list refreshed".to_string();
        }
    }

    pub(super) fn refresh_games(&mut self) {
        match core::list_games() {
            Ok(games) => {
                self.apply_loaded_games(games);
            }
            Err(err) => {
                self.games.clear();
                self.selected_index = None;
                self.status_message = format!("Failed to load games: {}", err);
            }
        }
    }

    pub(super) fn discover_games(&mut self) {
        match core::discover_games() {
            Ok(report) => {
                self.refresh_games();

                let mattmc_message = match report.mattmc {
                    Some(DiscoverResult::Added) => "MattMC added".to_string(),
                    Some(DiscoverResult::AlreadyExists) => "MattMC already exists".to_string(),
                    Some(DiscoverResult::NotFound) => "MattMC not found".to_string(),
                    None => "MattMC skipped".to_string(),
                };

                let steam_message = match report.steam {
                    Some(steam) => format!(
                        "Steam: found {}, added {}, existing {}",
                        steam.found, steam.added, steam.already_exists
                    ),
                    None => "Steam skipped".to_string(),
                };

                self.status_message =
                    format!("Discover complete | {} | {}", mattmc_message, steam_message);
            }
            Err(err) => {
                self.status_message = format!("Discover failed: {}", err);
            }
        }
    }

    pub(super) fn selected_game(&self) -> Option<&GameEntry> {
        self.selected_index.and_then(|index| self.games.get(index))
    }

    pub(super) fn filtered_library_indices(&self) -> Vec<usize> {
        let selected_playlist_games: Option<HashSet<&str>> = self
            .selected_playlist
            .as_ref()
            .and_then(|playlist_name| {
                self.playlists
                    .iter()
                    .find(|playlist| playlist.name == *playlist_name)
                    .map(|playlist| {
                        playlist
                            .game_names
                            .iter()
                            .map(String::as_str)
                            .collect::<HashSet<&str>>()
                    })
            });

        self.games
            .iter()
            .enumerate()
            .filter(|(_, game)| {
                let in_selected_playlist = selected_playlist_games
                    .as_ref()
                    .map(|game_names| game_names.contains(game.name.as_str()))
                    .unwrap_or(true);

                in_selected_playlist
                    && (search::matches_query(&game.name, &self.library_search_query)
                    || search::matches_query(game.runner_kind.as_str(), &self.library_search_query)
                    || search::matches_query(&game.launch_target, &self.library_search_query))
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(super) fn refresh_playlists(&mut self) {
        match core::list_playlists() {
            Ok(playlists) => {
                self.playlists = playlists;

                if let Some(selected_playlist) = self.selected_playlist.as_ref() {
                    let exists = self
                        .playlists
                        .iter()
                        .any(|playlist| playlist.name == *selected_playlist);
                    if !exists {
                        self.selected_playlist = None;
                    }
                }
            }
            Err(err) => {
                self.status_message = format!("Failed to load playlists: {}", err);
                self.playlists = vec![core::Playlist {
                    name: core::FAVORITES_PLAYLIST_NAME.to_string(),
                    game_names: Vec::new(),
                }];
                self.selected_playlist = None;
            }
        }
    }

    pub(super) fn is_game_favorited(&self, game_name: &str) -> bool {
        self.playlists
            .iter()
            .find(|playlist| playlist.name == core::FAVORITES_PLAYLIST_NAME)
            .map(|playlist| playlist.game_names.iter().any(|name| name == game_name))
            .unwrap_or(false)
    }

    pub(super) fn set_game_favorited_from_gui(&mut self, game_name: &str, favorited: bool) {
        let result = if favorited {
            core::add_game_to_playlist(core::FAVORITES_PLAYLIST_NAME, game_name)
        } else {
            core::remove_game_from_playlist(core::FAVORITES_PLAYLIST_NAME, game_name)
        };

        match result {
            Ok(_) => {
                self.refresh_playlists();
                self.status_message = if favorited {
                    format!("Added {} to Favorites", game_name)
                } else {
                    format!("Removed {} from Favorites", game_name)
                };
            }
            Err(err) => {
                self.status_message = if favorited {
                    format!("Favorite failed: {}", err)
                } else {
                    format!("Unfavorite failed: {}", err)
                };
            }
        }
    }

    pub(super) fn install_mattmc_from_gui(&mut self) {
        match cli::run_install_mattmc_command() {
            Ok(_) => {
                self.install_status_message = "MattMC install completed".to_string();
                self.refresh_games();
            }
            Err(err) => {
                self.install_status_message = format!("Install failed: {}", err);
            }
        }
    }

    pub(super) fn sync_mattmc_up_from_gui(&mut self) {
        self.run_mattmc_sync_with_input("up\n", "SyncUp");
    }

    pub(super) fn sync_mattmc_down_from_gui(&mut self) {
        self.run_mattmc_sync_with_input("down\n", "SyncDown");
    }

    fn run_mattmc_sync_with_input(&mut self, stdin_content: &str, action_label: &str) {
        match core::run_game_sibling_script_with_input("MattMC", "SyncGameData.sh", stdin_content) {
            Ok(_) => {
                self.status_message = format!("{} completed for MattMC", action_label);
            }
            Err(err) => {
                self.status_message = format!("{} failed: {}", action_label, err);
            }
        }
    }
}
