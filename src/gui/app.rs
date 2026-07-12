use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::core::{self, GameEntry};

use super::app_state::{
    BackgroundJobState, ControllerState, InstallState, LibraryState, NavigationState,
    SettingsState, StartupLoadState, UpdateState,
};
use super::artwork::ArtworkStore;
use super::top_bar::TopBarTab;

pub(super) struct BasaltApp {
    pub(super) navigation: NavigationState,
    pub(super) library: LibraryState,
    pub(super) install: InstallState,
    pub(super) settings: SettingsState,
    pub(super) update: UpdateState,
    pub(super) startup_load: StartupLoadState,
    pub(super) background_job: BackgroundJobState,
    pub(super) controller: ControllerState,
    pub(super) artwork_store: ArtworkStore,
}

impl Default for BasaltApp {
    fn default() -> Self {
        let (startup_games_tx, startup_games_rx) =
            mpsc::channel::<core::CoreResult<Vec<GameEntry>>>();
        thread::spawn(move || {
            let _ = startup_games_tx.send(core::list_games());
        });

        let mut app = Self {
            navigation: NavigationState::default(),
            library: LibraryState::default(),
            install: InstallState::default(),
            settings: SettingsState::load(),
            update: UpdateState::default(),
            startup_load: StartupLoadState {
                games_rx: Some(startup_games_rx),
            },
            background_job: BackgroundJobState::default(),
            controller: ControllerState::default(),
            artwork_store: ArtworkStore::new(),
        };
        app.artwork_store.prepare_for_games(&app.library.games);
        app.start_update_check();
        app
    }
}

impl BasaltApp {
    fn apply_persisted_window_mode_if_needed(&mut self, ctx: &eframe::egui::Context) {
        if !self.settings.pending_initial_window_mode_apply {
            return;
        }

        if self.settings.launcher_fullscreen_enabled {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(false));
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(true));
        } else if self.settings.launcher_maximized_enabled {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(false));
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(true));
        } else {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Fullscreen(false));
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(false));
        }

        self.settings.pending_initial_window_mode_apply = false;
    }
}

pub fn run() -> Result<(), String> {
    let display_settings =
        core::load_launcher_display_settings().unwrap_or(core::LauncherDisplaySettings {
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
        self.poll_background_job();
        self.poll_update_tasks();
        self.artwork_store.poll_download_results(ctx);
        self.apply_persisted_window_mode_if_needed(ctx);
        if self.background_job.rx.is_some()
            || self.update.check_rx.is_some()
            || self.update.install_rx.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        let main_region_gray = eframe::egui::Color32::from_rgb(49, 56, 69);
        let top_region_gray = eframe::egui::Color32::from_rgb(44, 51, 64);
        let right_region_gray = eframe::egui::Color32::from_rgb(55, 62, 76);
        let right_panel_width = (ctx.screen_rect().width() / 4.0) * (2.0 / 3.0);

        let actions = self.render_top_bar(ctx, top_region_gray);
        self.apply_top_bar_actions(actions);

        match self.navigation.active_tab {
            TopBarTab::Library => {
                self.render_library_screen(
                    ctx,
                    main_region_gray,
                    right_region_gray,
                    right_panel_width,
                );
                if self.controller.gilrs.is_some() {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
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
