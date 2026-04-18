use std::env;
use std::path::{Path, PathBuf};

const APP_DIR_NAME: &str = ".basalt";

pub fn home_dir() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    Ok(PathBuf::from(home))
}

pub fn app_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(APP_DIR_NAME))
}

pub fn command_exists(command_name: &str) -> bool {
    let Some(path_value) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path_value).any(|directory| {
        let candidate = directory.join(command_name);
        candidate.exists() && candidate.is_file()
    })
}

pub fn steam_candidate_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local").join("share").join("Steam"),
        home.join(".steam").join("steam"),
        home.join("Library").join("Application Support").join("Steam"),
    ]
}

pub fn mattmc_launch_script_name() -> &'static str {
    "run-mattmc.sh"
}
