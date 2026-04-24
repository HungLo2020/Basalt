#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

pub mod api;
mod platforms;

pub use api::{
    app_dir,
    command_exists,
    home_dir,
    launch_script,
    launch_script_with_stdin,
    mattmc_launch_script_candidates,
    mattmc_sync_script_name,
    mattmc_release_zip_suffix,
    normalize_script_path,
    run_command,
    steam_candidate_roots,
};
