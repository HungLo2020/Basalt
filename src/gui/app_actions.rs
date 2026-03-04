use crate::cli;
use crate::core::{self, DiscoverResult, GameEntry};

use super::app::BasaltApp;
use super::search;
use super::top_bar::TopBarActions;

impl BasaltApp {
    pub(super) fn apply_top_bar_actions(&mut self, actions: TopBarActions) {
        if let Some(tab) = actions.switch_to_tab {
            self.active_tab = tab;
        }

        if actions.trigger_add {
            self.add_from_inputs();
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

    pub(super) fn add_from_inputs(&mut self) {
        let name = self.add_name.trim().to_string();
        let script_path = self.add_script_path.trim().to_string();

        if name.is_empty() || script_path.is_empty() {
            self.status_message = "Add requires both Name and Script path".to_string();
            return;
        }

        match core::add_game(&name, &script_path) {
            Ok(_) => {
                self.refresh_games();
                self.status_message = format!("Added {}", name);
            }
            Err(err) => {
                self.status_message = format!("Add failed: {}", err);
            }
        }
    }

    pub(super) fn selected_game(&self) -> Option<&GameEntry> {
        self.selected_index.and_then(|index| self.games.get(index))
    }

    pub(super) fn filtered_library_indices(&self) -> Vec<usize> {
        self.games
            .iter()
            .enumerate()
            .filter(|(_, game)| {
                search::matches_query(&game.name, &self.library_search_query)
                    || search::matches_query(game.runner_kind.as_str(), &self.library_search_query)
                    || search::matches_query(&game.launch_target, &self.library_search_query)
            })
            .map(|(index, _)| index)
            .collect()
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
