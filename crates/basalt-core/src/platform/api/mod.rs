use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};

use directories::{BaseDirs, ProjectDirs};

use super::platforms;

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "hunglo2020";
const PROJECT_APPLICATION: &str = "Basalt";
const LEGACY_APP_DIR_NAME: &str = ".basalt";

pub fn home_dir() -> Result<PathBuf, String> {
    BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .ok_or_else(|| "Could not determine the home directory".to_string())
}

/// Persistent user data (game registry, playlists, blacklist).
pub fn data_dir() -> Result<PathBuf, String> {
    project_dirs().map(|dirs| dirs.data_dir().to_path_buf())
}

/// User-editable configuration (settings.json).
pub fn config_dir() -> Result<PathBuf, String> {
    project_dirs().map(|dirs| dirs.config_dir().to_path_buf())
}

/// Regenerable data (artwork caches).
pub fn cache_dir() -> Result<PathBuf, String> {
    project_dirs().map(|dirs| dirs.cache_dir().to_path_buf())
}

/// The pre-XDG `~/.basalt` directory that older builds stored everything in.
pub fn legacy_app_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(LEGACY_APP_DIR_NAME))
}

fn project_dirs() -> Result<ProjectDirs, String> {
    ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION)
        .ok_or_else(|| "Could not determine Basalt's application directories".to_string())
}

pub fn command_exists(command_name: &str) -> bool {
    platforms::command_exists(command_name)
}

/// Runs an executable that ships next to the current one (falling back to PATH),
/// waits for it to exit, and returns its exit status.
pub fn run_sibling_executable(name: &str) -> Result<ExitStatus, String> {
    let file_name = format!("{}{}", name, std::env::consts::EXE_SUFFIX);
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&file_name)))
        .filter(|candidate| candidate.is_file());

    let program = match sibling {
        Some(path) => path,
        None if command_exists(&file_name) || command_exists(name) => PathBuf::from(name),
        None => {
            return Err(format!(
                "Could not find '{}' next to this executable or on PATH",
                name
            ))
        }
    };

    Command::new(&program)
        .status()
        .map_err(|error| format!("Failed to start {}: {}", program.display(), error))
}

pub fn mattmc_launch_script_candidates() -> &'static [&'static str] {
    platforms::mattmc_launch_script_candidates()
}

pub fn mattmc_sync_script_name() -> &'static str {
    platforms::mattmc_sync_script_name()
}

pub fn mattmc_update_script_name() -> &'static str {
    platforms::mattmc_update_script_name()
}

pub fn mattmc_release_zip_suffix() -> &'static str {
    platforms::mattmc_release_zip_suffix()
}

pub fn normalize_script_path(raw_script_path: &str) -> Result<String, String> {
    platforms::normalize_script_path(raw_script_path)
}

pub fn launch_script(script_path: &str) -> Result<(), String> {
    platforms::launch_script(script_path)
}

pub fn launch_script_with_stdin(script_path: &str, stdin_content: &str) -> Result<(), String> {
    platforms::launch_script_with_stdin(script_path, stdin_content)
}

pub fn run_command(command_name: &str, args: &[&str]) -> Result<Output, String> {
    platforms::run_command(command_name, args)
}

pub fn basalt_update_asset_suffix() -> &'static str {
    platforms::basalt_update_asset_suffix()
}

pub fn basalt_update_asset_marker() -> &'static str {
    platforms::basalt_update_asset_marker()
}

pub fn install_basalt_update_and_restart(installer_path: &Path) -> Result<(), String> {
    platforms::install_basalt_update_and_restart(installer_path)
}

pub fn can_install_basalt_updates() -> bool {
    platforms::can_install_basalt_updates()
}
