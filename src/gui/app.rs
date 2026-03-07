use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use crate::core::{self, GameEntry, Playlist};
use gilrs::Gilrs;

use super::artwork::ArtworkStore;
use super::top_bar::TopBarTab;

pub(super) struct BasaltApp {
    pub(super) active_tab: TopBarTab,
    pub(super) settings_return_tab: TopBarTab,
    pub(super) games: Vec<GameEntry>,
    pub(super) playlists: Vec<Playlist>,
    pub(super) selected_playlist: Option<String>,
    pub(super) selected_index: Option<usize>,
    pub(super) selected_install_tile_key: Option<String>,
    pub(super) library_search_query: String,
    pub(super) install_search_query: String,
    pub(super) settings_remote_roms_root_input: String,
    pub(super) settings_remote_saves_root_input: String,
    pub(super) settings_launcher_fullscreen_enabled: bool,
    pub(super) settings_launcher_maximized_enabled: bool,
    pub(super) status_message: String,
    pub(super) install_status_message: String,
    pub(super) settings_status_message: String,
    pub(super) artwork_store: ArtworkStore,
    pub(super) startup_games_rx: Option<Receiver<core::CoreResult<Vec<GameEntry>>>>,
    pub(super) controller: Option<Gilrs>,
    pub(super) controller_stick_x_held: bool,
    pub(super) controller_stick_y_held: bool,
    pub(super) pending_scroll_to_selected: bool,
    pub(super) pending_initial_window_mode_apply: bool,
}

impl Default for BasaltApp {
    fn default() -> Self {
        let (startup_games_tx, startup_games_rx) = mpsc::channel::<core::CoreResult<Vec<GameEntry>>>();
        thread::spawn(move || {
            let _ = startup_games_tx.send(core::list_games());
        });

        let (settings_remote_roms_root_input, settings_remote_saves_root_input, settings_status_message) =
            match core::load_emulation_remote_paths() {
                Ok(paths) => (
                    paths.roms_root_dir,
                    paths.saves_root_dir,
                    String::new(),
                ),
                Err(error) => {
                    let defaults = core::default_emulation_remote_paths();
                    (
                        defaults.roms_root_dir,
                        defaults.saves_root_dir,
                        format!("Settings load warning: {}", error),
                    )
                }
            };

        let (
            settings_launcher_fullscreen_enabled,
            settings_launcher_maximized_enabled,
            settings_status_message,
        ) = match core::load_launcher_display_settings() {
                Ok(display_settings) => (
                    display_settings.fullscreen_enabled,
                    display_settings.maximized_enabled,
                    settings_status_message,
                ),
                Err(error) => {
                    let warning = format!("Display setting load warning: {}", error);
                    let merged_message = if settings_status_message.trim().is_empty() {
                        warning
                    } else {
                        format!("{} | {}", settings_status_message, warning)
                    };
                    (false, false, merged_message)
                }
            };

        let mut app = Self {
            active_tab: TopBarTab::Library,
            settings_return_tab: TopBarTab::Library,
            games: Vec::new(),
            playlists: vec![Playlist {
                name: "Favorites".to_string(),
                game_names: Vec::new(),
            }],
            selected_playlist: None,
            selected_index: None,
            selected_install_tile_key: None,
            library_search_query: String::new(),
            install_search_query: String::new(),
            settings_remote_roms_root_input,
            settings_remote_saves_root_input,
            settings_launcher_fullscreen_enabled,
            settings_launcher_maximized_enabled,
            status_message: "Loading games...".to_string(),
            install_status_message: String::new(),
            settings_status_message,
            artwork_store: ArtworkStore::new(),
            startup_games_rx: Some(startup_games_rx),
            controller: Gilrs::new().ok(),
            controller_stick_x_held: false,
            controller_stick_y_held: false,
            pending_scroll_to_selected: false,
            pending_initial_window_mode_apply: true,
        };
        app.artwork_store.prepare_for_games(&app.games);
        app
    }
}

impl BasaltApp {
    fn apply_persisted_window_mode_if_needed(&mut self, ctx: &eframe::egui::Context) {
        if !self.pending_initial_window_mode_apply {
            return;
        }

        if self.settings_launcher_fullscreen_enabled {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(false));
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(true));
        } else if self.settings_launcher_maximized_enabled {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(false));
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(true));
        } else {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(false));
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(false));
        }

        self.pending_initial_window_mode_apply = false;
    }
}

pub fn run() -> Result<(), String> {
    let display_settings = core::load_launcher_display_settings().unwrap_or(core::LauncherDisplaySettings {
        fullscreen_enabled: false,
        maximized_enabled: false,
    });
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_fullscreen(display_settings.fullscreen_enabled)
            .with_maximized(display_settings.maximized_enabled),
        ..Default::default()
    };

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
        self.apply_persisted_window_mode_if_needed(ctx);

        let main_region_gray = eframe::egui::Color32::from_rgb(49, 56, 69);
        let top_region_gray = eframe::egui::Color32::from_rgb(44, 51, 64);
        let right_region_gray = eframe::egui::Color32::from_rgb(55, 62, 76);
        let right_panel_width = (ctx.screen_rect().width() / 4.0) * (2.0 / 3.0);

        let actions = self.render_top_bar(ctx, top_region_gray);
        self.apply_top_bar_actions(actions);

        match self.active_tab {
            TopBarTab::Library => {
                self.render_library_screen(
                    ctx,
                    main_region_gray,
                    right_region_gray,
                    right_panel_width,
                );
                ctx.request_repaint_after(Duration::from_millis(16));
            }
            TopBarTab::Install => {
                self.render_install_screen(
                    ctx,
                    main_region_gray,
                    right_region_gray,
                    right_panel_width,
                );
            }
            TopBarTab::Settings => {
                self.render_settings_screen(
                    ctx,
                    main_region_gray,
                    right_region_gray,
                    right_panel_width,
                );
            }
        }
    }
}
