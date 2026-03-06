mod discovery;
mod discovery_service;
mod emulation;
mod error;
mod game_service;
mod playlist_service;
mod registry;
mod runners;
mod script_service;
mod types;

pub use discovery_service::{discover_games, discover_with_runners};
pub use emulation::{
    install_core_for_system as install_emulation_core_for_system,
    install_runtime_and_cores as install_emulation_runtime,
    is_core_installed_for_system as is_emulation_core_installed_for_system,
};
pub use error::CoreResult;
pub use game_service::{
    add_game, add_game_to_playlist, launch_game, list_games, list_playlists, remove_all_games,
    remove_game, remove_game_from_playlist,
};
pub use playlist_service::FAVORITES_PLAYLIST_NAME;
pub use script_service::{run_game_sibling_script, run_game_sibling_script_with_input};
pub use types::{
    DiscoverReport, DiscoverResult, DiscoverRunner, EmulatorDiscoverReport, GameEntry, Playlist,
    SteamDiscoverReport, ALL_DISCOVER_RUNNERS,
};
#[allow(dead_code)]
pub type CoreError = error::CoreError;

pub(crate) use discovery_service::{is_already_exists_error, is_blacklisted_error};