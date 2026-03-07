mod artwork_cache;
mod discovery;
mod discovery_service;
mod emulation;
mod emulation_target;
mod emulator_systems;
mod error;
mod game_service;
mod playlist_service;
mod registry;
mod runners;
mod script_service;
mod settings;
mod types;

pub use discovery_service::{discover_games, discover_with_runners};
pub use artwork_cache::clear_artwork_cache;
pub use emulation::{
    install_core_for_system as install_emulation_core_for_system,
    install_runtime_and_cores as install_emulation_runtime,
    is_core_installed_for_system as is_emulation_core_installed_for_system,
    is_save_sync_supported_for_system as is_emulation_save_sync_supported_for_system,
    RomSyncReport as EmulationRomSyncReport,
    sync_saves_down_for_system as sync_emulation_saves_down_for_system,
    sync_saves_up_for_system as sync_emulation_saves_up_for_system,
    sync_roms_up_for_system as sync_emulation_roms_up_for_system,
};
pub use emulation_target::EmulationLaunchTarget;
pub use emulator_systems::{
    emulation_install_tiles,
    emulator_artwork_catalog_path,
};
pub use error::CoreResult;
pub use game_service::{
    add_game, add_game_to_playlist, launch_game, list_games, list_playlists, remove_all_games,
    remove_game, remove_game_from_playlist,
};
pub use playlist_service::FAVORITES_PLAYLIST_NAME;
pub use script_service::{
    run_game_sibling_script,
    sync_mattmc,
    sync_mattmc_down,
    sync_mattmc_up,
};
pub use settings::{
    default_emulation_remote_paths,
    LauncherDisplaySettings,
    load_launcher_display_settings,
    load_emulation_remote_paths,
    save_launcher_display_settings,
    save_emulation_remote_paths,
};
pub use types::{
    DiscoverReport, DiscoverResult, DiscoverRunner, EmulatorDiscoverReport, GameEntry, Playlist,
    SteamDiscoverReport, ALL_DISCOVER_RUNNERS,
};
#[allow(dead_code)]
pub type CoreError = error::CoreError;

pub(crate) use discovery_service::{is_already_exists_error, is_blacklisted_error};

pub fn sync_emulation_roms_down_and_discover_for_system(
    system: &str,
) -> CoreResult<(EmulationRomSyncReport, EmulatorDiscoverReport)> {
    let sync_report = emulation::sync_roms_down_for_system(system)?;
    let discover_report = discovery_service::discover_with_runners(&[DiscoverRunner::Emulators])?;
    let emulator_report = discover_report.emulators.unwrap_or(EmulatorDiscoverReport {
        found: 0,
        added: 0,
        updated: 0,
        already_exists: 0,
    });

    Ok((sync_report, emulator_report))
}