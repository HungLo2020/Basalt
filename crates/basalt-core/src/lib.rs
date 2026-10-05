//! Basalt's launcher core: the game library, discovery, launching, emulation, and sync.
//!
//! This crate has no UI dependencies. The `basalt` CLI and the `basalt-gui` front end are thin
//! layers over this API.

#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

pub mod artwork;
mod discovery;
mod discovery_service;
mod download;
mod emulation;
mod emulation_target;
mod emulator_systems;
mod error;
mod game_service;
mod mattmc_install;
pub mod platform;
mod playlist_service;
mod progress;
mod registry;
mod runners;
mod script_service;
mod settings;
mod storage;
mod types;
mod update_service;

pub use artwork::{clear_artwork_cache, ArtworkKind, ArtworkRequest};
pub use discovery_service::{discover_games, discover_with_runners};
pub use emulation::{
    install_core_for_system as install_emulation_core_for_system,
    install_runtime_and_cores as install_emulation_runtime,
    is_core_installed_for_system as is_emulation_core_installed_for_system,
    is_save_sync_supported_for_system as is_emulation_save_sync_supported_for_system,
    sync_roms_up_for_system as sync_emulation_roms_up_for_system,
    sync_saves_down_for_system as sync_emulation_saves_down_for_system,
    sync_saves_up_for_system as sync_emulation_saves_up_for_system, EmulationInstallReport,
    RomSyncReport as EmulationRomSyncReport,
};
pub use emulation_target::EmulationLaunchTarget;
pub use emulator_systems::{emulator_artwork_catalog_path, emulator_systems, EmulatorSystem};
pub use error::{CoreError, CoreResult};
pub use game_service::{
    add_game, add_game_to_playlist, launch_game, list_games, list_playlists, remove_all_games,
    remove_game, remove_game_from_playlist,
};
pub use mattmc_install::{install_mattmc, MattmcInstallReport};
pub use playlist_service::FAVORITES_PLAYLIST_NAME;
pub use progress::{CancelToken, Progress, ProgressUnit, ProgressUpdate};
pub use runners::RunnerKind;
pub use script_service::{
    run_game_sibling_script, sync_mattmc, sync_mattmc_down, sync_mattmc_up, update_mattmc,
};
pub use settings::{
    default_emulation_remote_paths, load_emulation_remote_paths, load_launcher_display_settings,
    save_emulation_remote_paths, save_launcher_display_settings, EmulationRemotePaths,
    LauncherDisplaySettings,
};
pub use types::{
    DiscoverReport, DiscoverResult, DiscoverRunner, EmulatorDiscoverReport, GameEntry, Playlist,
    SteamDiscoverReport, ALL_DISCOVER_RUNNERS,
};
pub use update_service::{
    can_install_updates as can_install_basalt_updates,
    check_for_updates as check_for_basalt_updates, download_update as download_basalt_update,
    install_update_and_restart as install_basalt_update_and_restart, BasaltBuildInfo,
    DownloadedUpdate, UpdateCheckResult,
};

/// Syncs a system's ROMs down from the remote root, then re-runs emulator discovery so the
/// library matches what is on disk.
pub fn sync_emulation_roms_down_and_discover_for_system(
    system: &str,
    progress: &Progress,
) -> CoreResult<(EmulationRomSyncReport, EmulatorDiscoverReport)> {
    let sync_report = emulation::sync_roms_down_for_system(system, progress)?;
    progress.report_step("Updating library");
    let discover_report = discovery_service::discover_with_runners(&[DiscoverRunner::Emulators])?;
    let emulator_report = discover_report.emulators.unwrap_or(EmulatorDiscoverReport {
        found: 0,
        added: 0,
        updated: 0,
        already_exists: 0,
    });

    Ok((sync_report, emulator_report))
}
