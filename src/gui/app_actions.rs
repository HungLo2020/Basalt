use std::collections::HashSet;

use crate::core::{self, DiscoverResult, GameEntry};

use super::app::BasaltApp;
use super::search;
use super::top_bar::{PlaylistSelection, TopBarActions, TopBarTab};

impl BasaltApp {
    pub(super) fn apply_top_bar_actions(&mut self, actions: TopBarActions) {
        if actions.open_settings {
            if self.active_tab == TopBarTab::Library || self.active_tab == TopBarTab::Install {
                self.settings_return_tab = self.active_tab;
            }
            self.active_tab = TopBarTab::Settings;
        }

        if actions.go_back_from_settings && self.active_tab == TopBarTab::Settings {
            self.active_tab = self.settings_return_tab;
        }

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
        match core::discover_with_runners(&core::ALL_DISCOVER_RUNNERS) {
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

                let emulator_message = match report.emulators {
                    Some(emulators) => format!(
                        "Emulators: found {}, added {}, updated {}, existing {}",
                        emulators.found,
                        emulators.added,
                        emulators.updated,
                        emulators.already_exists
                    ),
                    None => "Emulators skipped".to_string(),
                };

                self.status_message = format!(
                    "Discover complete | {} | {} | {}",
                    mattmc_message, steam_message, emulator_message
                );
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

    pub(super) fn remove_game_from_gui(&mut self, game_name: &str) {
        match core::remove_game(game_name) {
            Ok(_) => {
                self.selected_index = None;
                self.refresh_games();
                self.status_message = format!("Removed {}", game_name);
            }
            Err(err) => {
                self.status_message = format!("Remove failed: {}", err);
            }
        }
    }

    pub(super) fn install_mattmc_from_gui(&mut self) {
        match crate::cli::run_install_mattmc_command() {
            Ok(_) => {
                self.install_status_message = "MattMC install completed".to_string();
                self.refresh_games();
            }
            Err(err) => {
                self.install_status_message = format!("Install failed: {}", err);
            }
        }
    }

    pub(super) fn install_emulator_core_from_gui(&mut self, system: &str) {
        match core::install_emulation_core_for_system(system) {
            Ok(_) => {
                self.install_status_message =
                    format!("Installed {} emulator core", system.to_uppercase());
            }
            Err(err) => {
                self.install_status_message =
                    format!("{} core install failed: {}", system.to_uppercase(), err);
            }
        }
    }

    pub(super) fn sync_emulator_roms_up_from_gui(&mut self, system: &str) {
        match core::sync_emulation_roms_up_for_system(system) {
            Ok(report) => {
                self.install_status_message = format!(
                    "Sync Roms Up ({}) completed: copied {}, unchanged {}, deleted {}",
                    system.to_uppercase(),
                    report.copied,
                    report.unchanged,
                    report.deleted
                );
            }
            Err(err) => {
                self.install_status_message = format!(
                    "Sync Roms Up ({}) failed: {}",
                    system.to_uppercase(),
                    err
                );
            }
        }
    }

    pub(super) fn sync_emulator_roms_down_from_gui(&mut self, system: &str) {
        match core::sync_emulation_roms_down_and_discover_for_system(system) {
            Ok((sync_report, emulator_report)) => {
                self.refresh_games();
                self.install_status_message = format!(
                    "Sync Roms Down ({}) completed: copied {}, unchanged {}, deleted {} | Emulator discover: found {}, added {}, updated {}, existing {}",
                    system.to_uppercase(),
                    sync_report.copied,
                    sync_report.unchanged,
                    sync_report.deleted,
                    emulator_report.found,
                    emulator_report.added,
                    emulator_report.updated,
                    emulator_report.already_exists
                );
            }
            Err(err) => {
                self.install_status_message = format!(
                    "Sync Roms Down ({}) failed: {}",
                    system.to_uppercase(),
                    err
                );
            }
        }
    }

    pub(super) fn sync_emulator_saves_up_from_gui(&mut self, system: &str) {
        match core::sync_emulation_saves_up_for_system(system) {
            Ok(report) => {
                self.install_status_message = format!(
                    "Sync Saves Up ({}) completed: copied {}, unchanged {}, deleted {}",
                    system.to_uppercase(),
                    report.copied,
                    report.unchanged,
                    report.deleted
                );
            }
            Err(err) => {
                self.install_status_message = format!(
                    "Sync Saves Up ({}) failed: {}",
                    system.to_uppercase(),
                    err
                );
            }
        }
    }

    pub(super) fn sync_emulator_saves_down_from_gui(&mut self, system: &str) {
        match core::sync_emulation_saves_down_for_system(system) {
            Ok(report) => {
                self.install_status_message = format!(
                    "Sync Saves Down ({}) completed: copied {}, unchanged {}, deleted {}",
                    system.to_uppercase(),
                    report.copied,
                    report.unchanged,
                    report.deleted
                );
            }
            Err(err) => {
                self.install_status_message = format!(
                    "Sync Saves Down ({}) failed: {}",
                    system.to_uppercase(),
                    err
                );
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
