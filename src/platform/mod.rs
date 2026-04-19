pub mod api;
mod platforms;

pub use api::{
    app_dir,
    command_exists,
    home_dir,
    launch_script,
    launch_script_with_stdin,
    mattmc_launch_script_name,
    normalize_script_path,
    steam_candidate_roots,
};
