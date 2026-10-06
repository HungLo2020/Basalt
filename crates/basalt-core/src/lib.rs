//! Basalt's launcher core: the game library, discovery, launching, emulation, and sync.
//!
//! This crate has no UI dependencies. The `basalt` CLI and the `basalt-gui` front end are thin
//! layers over this API.

#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

mod artwork;
mod discovery;
mod download;
mod emulation;
mod error;
mod game_service;
mod mattmc;
mod mattmc_install;
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
pub use discovery::{discover_games, discover_with_runners};
pub use emulation::{
    EmulatorInstallReport, EmulatorLaunchTarget, EmulatorSystem, SyncReport, emulator_rom_dir,
    emulator_save_dir, emulator_supports_save_sync, emulator_systems, install_emulator_core,
    install_emulators, is_emulator_core_installed, sync_roms_down, sync_roms_up, sync_saves_down,
    sync_saves_up,
};
pub use error::{CoreError, CoreResult};
pub use game_service::{
    add_game, add_game_to_playlist, launch_game, list_games, list_playlists, remove_all_games,
    remove_game, remove_game_from_playlist,
};
pub use mattmc::{
    MATTMC_GAME_NAME, backup_mattmc, mattmc_install_dir, sync_mattmc_down, sync_mattmc_interactive,
    sync_mattmc_up, update_mattmc,
};
pub use mattmc_install::{MattmcInstallReport, install_mattmc};
pub use platform::run_sibling_executable;
pub use playlist_service::FAVORITES_PLAYLIST_NAME;
pub use progress::{CancelToken, Progress, ProgressUnit, ProgressUpdate};
pub use runners::RunnerKind;
pub use settings::{
    EmulationRemotePaths, LauncherDisplaySettings, default_emulation_remote_paths,
    load_emulation_remote_paths, load_launcher_display_settings, save_emulation_remote_paths,
    save_launcher_display_settings,
};
pub use types::{
    ALL_DISCOVER_RUNNERS, DiscoverReport, DiscoverResult, DiscoverRunner, EmulatorDiscoverReport,
    GameEntry, Playlist, SteamDiscoverReport,
};
pub use warnings::take_warnings;
