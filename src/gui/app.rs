use crate::cli;
use crate::core::{self, DiscoverResult, GameEntry};

use super::top_bar::{TopBarActions, TopBarTab};

pub(super) struct BasaltApp {
    pub(super) active_tab: TopBarTab,
    pub(super) games: Vec<GameEntry>,
    pub(super) selected_index: Option<usize>,
    pub(super) add_name: String,
    pub(super) add_script_path: String,
    pub(super) status_message: String,
    pub(super) install_status_message: String,
}

impl Default for BasaltApp {
    fn default() -> Self {
        let mut app = Self {
            active_tab: TopBarTab::Library,
            games: Vec::new(),
            selected_index: None,
            add_name: String::new(),
            add_script_path: String::new(),
            status_message: String::new(),
            install_status_message: String::new(),
        };
        app.refresh_games();
        app
    }
}

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Basalt",
        options,
        Box::new(|_cc| Ok(Box::new(BasaltApp::default()))),
    )
    .map_err(|err| format!("Failed to launch GUI: {}", err))
}

impl eframe::App for BasaltApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        let region_gray = eframe::egui::Color32::from_gray(55);
        let white_line = eframe::egui::Stroke::new(1.0, eframe::egui::Color32::WHITE);
        let right_panel_width = ctx.screen_rect().width() / 4.0;

        let actions = self.render_top_bar(ctx, region_gray, white_line);
        self.apply_top_bar_actions(actions);

        match self.active_tab {
            TopBarTab::Library => {
                self.render_library_screen(ctx, region_gray, white_line, right_panel_width);
            }
            TopBarTab::Install => {
                self.render_install_screen(ctx, region_gray, white_line, right_panel_width);
            }
        }
    }
}

impl BasaltApp {
    fn apply_top_bar_actions(&mut self, actions: TopBarActions) {
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
                self.games = games;
                if let Some(index) = self.selected_index {
                    if index >= self.games.len() {
                        self.selected_index = None;
                    }
                }
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
}
