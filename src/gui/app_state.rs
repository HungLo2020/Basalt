use std::sync::mpsc::Receiver;

use crate::core::{self, GameEntry, Playlist};
use gilrs::Gilrs;

use super::background_jobs::GuiBackgroundJobResult;
use super::top_bar::TopBarTab;

pub(super) struct NavigationState {
    pub(super) active_tab: TopBarTab,
    pub(super) settings_return_tab: TopBarTab,
}

pub(super) struct LibraryState {
    pub(super) games: Vec<GameEntry>,
    pub(super) playlists: Vec<Playlist>,
    pub(super) selected_playlist: Option<String>,
    pub(super) selected_index: Option<usize>,
    pub(super) search_query: String,
    pub(super) status_message: String,
    pub(super) pending_scroll_to_selected: bool,
}

#[derive(Default)]
pub(super) struct InstallState {
    pub(super) selected_tile_key: Option<String>,
    pub(super) search_query: String,
    pub(super) status_message: String,
}

pub(super) struct SettingsState {
    pub(super) remote_roms_root_input: String,
    pub(super) remote_saves_root_input: String,
    pub(super) launcher_fullscreen_enabled: bool,
    pub(super) launcher_maximized_enabled: bool,
    pub(super) status_message: String,
    pub(super) pending_initial_window_mode_apply: bool,
}

#[derive(Default)]
pub(super) struct UpdateState {
    pub(super) status_message: String,
    pub(super) latest_update: Option<core::UpdateCheckResult>,
    pub(super) check_rx: Option<Receiver<Result<core::UpdateCheckResult, String>>>,
    pub(super) install_rx: Option<Receiver<Result<(), String>>>,
}

#[derive(Default)]
pub(super) struct StartupLoadState {
    pub(super) games_rx: Option<Receiver<core::CoreResult<Vec<GameEntry>>>>,
}

pub(super) struct ControllerState {
    pub(super) gilrs: Option<Gilrs>,
    pub(super) stick_x_held: bool,
    pub(super) stick_y_held: bool,
}

#[derive(Default)]
pub(super) struct BackgroundJobState {
    pub(super) rx: Option<Receiver<GuiBackgroundJobResult>>,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            active_tab: TopBarTab::Library,
            settings_return_tab: TopBarTab::Library,
        }
    }
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            games: Vec::new(),
            playlists: vec![Playlist {
                name: "Favorites".to_string(),
                game_names: Vec::new(),
            }],
            selected_playlist: None,
            selected_index: None,
            search_query: String::new(),
            status_message: "Loading games...".to_string(),
            pending_scroll_to_selected: false,
        }
    }
}

impl SettingsState {
    pub(super) fn load() -> Self {
        let (remote_roms_root_input, remote_saves_root_input, status_message) =
            match core::load_emulation_remote_paths() {
                Ok(paths) => (paths.roms_root_dir, paths.saves_root_dir, String::new()),
                Err(error) => {
                    let defaults = core::default_emulation_remote_paths();
                    (
                        defaults.roms_root_dir,
                        defaults.saves_root_dir,
                        format!("Settings load warning: {}", error),
                    )
                }
            };

        let (launcher_fullscreen_enabled, launcher_maximized_enabled, status_message) =
            match core::load_launcher_display_settings() {
                Ok(display_settings) => (
                    display_settings.fullscreen_enabled,
                    display_settings.maximized_enabled,
                    status_message,
                ),
                Err(error) => {
                    let warning = format!("Display setting load warning: {}", error);
                    let merged_message = if status_message.trim().is_empty() {
                        warning
                    } else {
                        format!("{} | {}", status_message, warning)
                    };
                    (false, false, merged_message)
                }
            };

        Self {
            remote_roms_root_input,
            remote_saves_root_input,
            launcher_fullscreen_enabled,
            launcher_maximized_enabled,
            status_message,
            pending_initial_window_mode_apply: true,
        }
    }
}

impl Default for ControllerState {
    fn default() -> Self {
        Self {
            gilrs: Gilrs::new().ok(),
            stick_x_held: false,
            stick_y_held: false,
        }
    }
}
