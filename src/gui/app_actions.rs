use std::collections::HashSet;

use crate::core::{self, DiscoverResult, GameEntry};

use super::app::BasaltApp;
use super::background_jobs::{GuiBackgroundJobResult, GuiBackgroundStatusTarget};
use super::search;
use super::top_bar::{PlaylistSelection, TopBarActions, TopBarTab};

impl BasaltApp {
    pub(super) fn apply_top_bar_actions(&mut self, actions: TopBarActions) {
        if actions.open_settings {
            if self.navigation.active_tab == TopBarTab::Library
                || self.navigation.active_tab == TopBarTab::Install
            {
                self.navigation.settings_return_tab = self.navigation.active_tab;
            }
            self.navigation.active_tab = TopBarTab::Settings;
        }

        if actions.go_back_from_settings && self.navigation.active_tab == TopBarTab::Settings {
            self.navigation.active_tab = self.navigation.settings_return_tab;
        }

        if let Some(tab) = actions.switch_to_tab {
            self.navigation.active_tab = tab;
        }

        if let Some(selection) = actions.select_playlist {
            match selection {
                PlaylistSelection::AllGames => {
                    self.library.selected_playlist = None;
                }
                PlaylistSelection::Named(name) => {
                    self.library.selected_playlist = Some(name);
                }
            }
        }

        if actions.trigger_discover {
            self.discover_games();
        }
        if actions.trigger_refresh {
            self.refresh_games();
            self.library.status_message = "Game list refreshed".to_string();
        }
        if actions.trigger_refresh_metadata {
            self.refresh_metadata_from_gui();
        }
        if actions.trigger_update {
            self.handle_update_button_click();
        }
    }

    pub(super) fn refresh_metadata_from_gui(&mut self) {
        self.artwork_store
            .refresh_metadata_for_games(&self.library.games);
        self.library.status_message =
            "Metadata refresh started: caches cleared, artwork requeued".to_string();
    }

    pub(super) fn refresh_games(&mut self) {
        match core::list_games() {
            Ok(games) => {
                self.apply_loaded_games(games);
            }
            Err(err) => {
                self.library.games.clear();
                self.library.selected_index = None;
                self.library.status_message = format!("Failed to load games: {}", err);
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

                self.library.status_message = format!(
                    "Discover complete | {} | {} | {}",
                    mattmc_message, steam_message, emulator_message
                );
            }
            Err(err) => {
                self.library.status_message = format!("Discover failed: {}", err);
            }
        }
    }

    pub(super) fn selected_game(&self) -> Option<&GameEntry> {
        self.library
            .selected_index
            .and_then(|index| self.library.games.get(index))
    }

    pub(super) fn filtered_library_indices(&self) -> Vec<usize> {
        let selected_playlist_games: Option<HashSet<&str>> = self
            .library
            .selected_playlist
            .as_ref()
            .and_then(|playlist_name| {
                self.library
                    .playlists
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

        self.library
            .games
            .iter()
            .enumerate()
            .filter(|(_, game)| {
                let in_selected_playlist = selected_playlist_games
                    .as_ref()
                    .map(|game_names| game_names.contains(game.name.as_str()))
                    .unwrap_or(true);

                in_selected_playlist
                    && (search::matches_query(&game.name, &self.library.search_query)
                        || search::matches_query(
                            game.runner_kind.as_str(),
                            &self.library.search_query,
                        )
                        || search::matches_query(&game.launch_target, &self.library.search_query))
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(super) fn refresh_playlists(&mut self) {
        match core::list_playlists() {
            Ok(playlists) => {
                self.library.playlists = playlists;

                if let Some(selected_playlist) = self.library.selected_playlist.as_ref() {
                    let exists = self
                        .library
                        .playlists
                        .iter()
                        .any(|playlist| playlist.name == *selected_playlist);
                    if !exists {
                        self.library.selected_playlist = None;
                    }
                }
            }
            Err(err) => {
                self.library.status_message = format!("Failed to load playlists: {}", err);
                self.library.playlists = vec![core::Playlist {
                    name: core::FAVORITES_PLAYLIST_NAME.to_string(),
                    game_names: Vec::new(),
                }];
                self.library.selected_playlist = None;
            }
        }
    }

    pub(super) fn is_game_favorited(&self, game_name: &str) -> bool {
        self.library
            .playlists
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
                self.library.status_message = if favorited {
                    format!("Added {} to Favorites", game_name)
                } else {
                    format!("Removed {} from Favorites", game_name)
                };
            }
            Err(err) => {
                self.library.status_message = if favorited {
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
                self.library.selected_index = None;
                self.refresh_games();
                self.library.status_message = format!("Removed {}", game_name);
            }
            Err(err) => {
                self.library.status_message = format!("Remove failed: {}", err);
            }
        }
    }

    pub(super) fn install_mattmc_from_gui(&mut self) {
        self.start_background_job(
            GuiBackgroundStatusTarget::Install,
            "MattMC install started".to_string(),
            || GuiBackgroundJobResult::InstallMattmc(core::install_mattmc()),
        );
    }

    pub(super) fn install_emulator_core_from_gui(&mut self, system: &str) {
        let system = system.to_string();
        self.start_background_job(
            GuiBackgroundStatusTarget::Install,
            format!("Installing {} emulator core...", system.to_uppercase()),
            move || {
                let result = core::install_emulation_core_for_system(&system);
                GuiBackgroundJobResult::InstallEmulatorCore { system, result }
            },
        );
    }

    pub(super) fn sync_emulator_roms_up_from_gui(&mut self, system: &str) {
        let system = system.to_string();
        self.start_background_job(
            GuiBackgroundStatusTarget::Install,
            format!("Sync Roms Up ({}) started", system.to_uppercase()),
            move || {
                let result = core::sync_emulation_roms_up_for_system(&system);
                GuiBackgroundJobResult::SyncEmulatorRomsUp { system, result }
            },
        );
    }

    pub(super) fn sync_emulator_roms_down_from_gui(&mut self, system: &str) {
        let system = system.to_string();
        self.start_background_job(
            GuiBackgroundStatusTarget::Install,
            format!("Sync Roms Down ({}) started", system.to_uppercase()),
            move || {
                let result = core::sync_emulation_roms_down_and_discover_for_system(&system)
                    .map_err(String::from);
                GuiBackgroundJobResult::SyncEmulatorRomsDown { system, result }
            },
        );
    }

    pub(super) fn sync_emulator_saves_up_from_gui(&mut self, system: &str) {
        let system = system.to_string();
        self.start_background_job(
            GuiBackgroundStatusTarget::Install,
            format!("Sync Saves Up ({}) started", system.to_uppercase()),
            move || {
                let result = core::sync_emulation_saves_up_for_system(&system);
                GuiBackgroundJobResult::SyncEmulatorSavesUp { system, result }
            },
        );
    }

    pub(super) fn sync_emulator_saves_down_from_gui(&mut self, system: &str) {
        let system = system.to_string();
        self.start_background_job(
            GuiBackgroundStatusTarget::Install,
            format!("Sync Saves Down ({}) started", system.to_uppercase()),
            move || {
                let result = core::sync_emulation_saves_down_for_system(&system);
                GuiBackgroundJobResult::SyncEmulatorSavesDown { system, result }
            },
        );
    }

    pub(super) fn sync_mattmc_up_from_gui(&mut self) {
        self.start_background_job(
            GuiBackgroundStatusTarget::Library,
            "SyncUp started for MattMC".to_string(),
            || GuiBackgroundJobResult::SyncMattmcUp(core::sync_mattmc_up().map_err(String::from)),
        );
    }

    pub(super) fn sync_mattmc_down_from_gui(&mut self) {
        self.start_background_job(
            GuiBackgroundStatusTarget::Library,
            "SyncDown started for MattMC".to_string(),
            || {
                GuiBackgroundJobResult::SyncMattmcDown(
                    core::sync_mattmc_down().map_err(String::from),
                )
            },
        );
    }

    pub(super) fn save_emulation_remote_paths_from_gui(&mut self) {
        match core::save_emulation_remote_paths(
            &self.settings.remote_roms_root_input,
            &self.settings.remote_saves_root_input,
        ) {
            Ok(saved) => {
                self.settings.remote_roms_root_input = saved.roms_root_dir;
                self.settings.remote_saves_root_input = saved.saves_root_dir;
                self.settings.status_message = "Saved remote ROM/Saves default paths".to_string();
            }
            Err(err) => {
                self.settings.status_message = format!("Save failed: {}", err);
            }
        }
    }

    pub(super) fn save_launcher_display_settings_from_gui(
        &mut self,
        ctx: &eframe::egui::Context,
        previous_fullscreen_value: bool,
        previous_maximized_value: bool,
    ) {
        let mut desired_fullscreen = self.settings.launcher_fullscreen_enabled;
        let mut desired_maximized = self.settings.launcher_maximized_enabled;

        if desired_fullscreen {
            desired_maximized = false;
        }

        if desired_maximized {
            desired_fullscreen = false;
        }

        self.settings.launcher_fullscreen_enabled = desired_fullscreen;
        self.settings.launcher_maximized_enabled = desired_maximized;

        match core::save_launcher_display_settings(desired_fullscreen, desired_maximized) {
            Ok(saved) => {
                self.settings.launcher_fullscreen_enabled = saved.fullscreen_enabled;
                self.settings.launcher_maximized_enabled = saved.maximized_enabled;

                if saved.fullscreen_enabled {
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(false));
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(true));
                } else if saved.maximized_enabled {
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(false));
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(true));
                } else {
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(false));
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(false));
                }

                self.settings.status_message = if saved.fullscreen_enabled {
                    "Enabled launcher fullscreen".to_string()
                } else if saved.maximized_enabled {
                    "Enabled launcher maximized window mode".to_string()
                } else {
                    "Disabled launcher fullscreen/maximized window mode".to_string()
                };
            }
            Err(err) => {
                self.settings.launcher_fullscreen_enabled = previous_fullscreen_value;
                self.settings.launcher_maximized_enabled = previous_maximized_value;
                self.settings.status_message = format!("Save failed: {}", err);
            }
        }
    }
}
