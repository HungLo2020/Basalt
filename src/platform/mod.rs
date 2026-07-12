#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

pub mod api;
mod platforms;

pub use api::{
    app_dir, basalt_update_asset_marker, basalt_update_asset_suffix, can_install_basalt_updates,
    command_exists, home_dir, install_basalt_update_and_restart, launch_script, launch_script_with_stdin,
    mattmc_launch_script_candidates, mattmc_release_zip_suffix, mattmc_sync_script_name,
    normalize_script_path, run_command, steam_candidate_roots,
};
