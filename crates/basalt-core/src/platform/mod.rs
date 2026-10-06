#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

pub mod api;
mod platforms;

pub use api::{
    cache_dir, command_exists, config_dir, data_dir, home_dir, launch_script,
    launch_script_with_stdin, legacy_app_dir, mattmc_launch_script_candidates,
    mattmc_release_zip_suffix, mattmc_sync_script_name, mattmc_update_script_name,
    normalize_script_path, run_command, run_sibling_executable, system_data_dirs,
};
