use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::core::{self, GameEntry};

use super::artwork::ArtworkStore;
use super::top_bar::TopBarTab;

pub(super) struct BasaltApp {
    pub(super) active_tab: TopBarTab,
    pub(super) games: Vec<GameEntry>,
    pub(super) selected_index: Option<usize>,
    pub(super) add_name: String,
    pub(super) add_script_path: String,
    pub(super) library_search_query: String,
    pub(super) install_search_query: String,
    pub(super) status_message: String,
    pub(super) install_status_message: String,
    pub(super) artwork_store: ArtworkStore,
    pub(super) startup_games_rx: Option<Receiver<core::CoreResult<Vec<GameEntry>>>>,
}

impl Default for BasaltApp {
    fn default() -> Self {
        let (startup_games_tx, startup_games_rx) = mpsc::channel::<core::CoreResult<Vec<GameEntry>>>();
        thread::spawn(move || {
            let _ = startup_games_tx.send(core::list_games());
        });

        let mut app = Self {
            active_tab: TopBarTab::Library,
            games: Vec::new(),
            selected_index: None,
            add_name: String::new(),
            add_script_path: String::new(),
            library_search_query: String::new(),
            install_search_query: String::new(),
            status_message: "Loading games...".to_string(),
            install_status_message: String::new(),
            artwork_store: ArtworkStore::new(),
            startup_games_rx: Some(startup_games_rx),
        };
        app.artwork_store.prepare_for_games(&app.games);
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
        self.poll_startup_games_load();
        self.artwork_store.poll_download_results(ctx);

        let region_gray = eframe::egui::Color32::from_rgb(49, 56, 69);
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
