use std::env;
use std::path::{Path, PathBuf};

const APP_DIR_NAME: &str = ".basalt";

pub fn home_dir() -> Result<PathBuf, String> {
    let user_profile = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map_err(|_| "Neither USERPROFILE nor HOME environment variables are set".to_string())?;
    Ok(PathBuf::from(user_profile))
}

pub fn app_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(APP_DIR_NAME))
}

pub fn command_exists(command_name: &str) -> bool {
    let Some(path_value) = env::var_os("PATH") else {
        return false;
    };

    let command_has_extension = Path::new(command_name)
        .extension()
        .and_then(|value| value.to_str())
        .is_some();

    let extensions = command_extensions();

    env::split_paths(&path_value).any(|directory| {
        let direct_candidate = directory.join(command_name);
        if direct_candidate.exists() && direct_candidate.is_file() {
            return true;
        }

        if command_has_extension {
            return false;
        }

        extensions.iter().any(|extension| {
            let candidate = directory.join(format!("{}.{}", command_name, extension));
            candidate.exists() && candidate.is_file()
        })
    })
}

pub fn steam_candidate_roots(home: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(program_files_x86) = env::var("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(program_files_x86).join("Steam"));
    }

    if let Ok(program_files) = env::var("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("Steam"));
    }

    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join("Steam"));
    }

    candidates.push(home.join("AppData").join("Local").join("Steam"));
    candidates
}

pub fn mattmc_launch_script_name() -> &'static str {
    "run-mattmc.sh"
}

fn command_extensions() -> Vec<String> {
    let raw = env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".to_string());
    raw.split(';')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
        .collect()
}
