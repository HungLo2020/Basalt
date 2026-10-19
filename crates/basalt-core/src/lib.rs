//! Basalt's launcher core: the game library, discovery, launching, and native game management.
//!
//! This crate has no UI dependencies. The `basalt` CLI and the `basalt-gui` front end are thin
//! layers over this API.

#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

mod artwork;
mod catalog;
mod catalog_launch;
mod discovery;
mod download;
mod error;
mod file_index;
mod game_data;
mod game_install;
mod game_layout;
mod game_service;
mod libraries;
mod maintenance;
mod platform;
mod playlist_service;
mod progress;
mod registry;
mod runners;
mod settings;
mod storage;
mod types;
mod warnings;

pub use artwork::{ArtworkKind, ArtworkRequest, clear_artwork_cache};
pub use catalog::{
    ActionConfig, ActionOptions, ActionSetting, GameDefinition, LaunchDefinition,
    LayoutDefinition, MaintenanceCommand, ReleaseSource, STANDARD_VERSION, catalog_games,
    find_game,
};
pub use discovery::{discover_games, discover_with_runners};
pub use error::{CoreError, CoreResult};
pub use game_install::{GameInstallReport, install_game};
pub use game_service::{
    add_game, add_game_to_playlist, launch_game, list_games, list_playlists, remove_all_games,
    remove_game, remove_game_from_playlist,
};
pub use libraries::{GameLibrary, add_library, default_library, install_directory, list_libraries};
pub use game_data::{BackupSnapshot, list_backups};
pub use game_layout::DataCategory;
pub use maintenance::{
    ActionRequest, BuiltinAction, GameAction, game_actions, list_game_actions, run_game_action,
};
pub use platform::run_sibling_executable;
pub use playlist_service::FAVORITES_PLAYLIST_NAME;
pub use progress::{CancelToken, Progress, ProgressUnit, ProgressUpdate};
pub use runners::RunnerKind;
pub use settings::{
    DEFAULT_SYNC_REMOTE, LauncherDisplaySettings, SyncRemote, load_launcher_display_settings,
    load_sync_remote, save_launcher_display_settings, save_sync_remote,
};
pub use types::{
    ALL_DISCOVER_RUNNERS, DiscoverReport, DiscoverResult, DiscoverRunner,
    GameEntry, Playlist, SteamDiscoverReport,
};
pub use warnings::take_warnings;
