use std::path::PathBuf;
use std::process::{Command, ExitStatus, Output};

use directories::{BaseDirs, ProjectDirs};

use super::platforms;
use crate::error::{CoreError, CoreResult};

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "hunglo2020";
const PROJECT_APPLICATION: &str = "Basalt";
const LEGACY_APP_DIR_NAME: &str = ".basalt";

pub fn home_dir() -> CoreResult<PathBuf> {
    BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .ok_or_else(|| CoreError::new("Could not determine the home directory".to_string()))
}

/// Persistent user data (game registry, playlists, blacklist).
pub fn data_dir() -> CoreResult<PathBuf> {
    project_dirs().map(|dirs| dirs.data_dir().to_path_buf())
}

/// User-editable configuration (settings.json).
pub fn config_dir() -> CoreResult<PathBuf> {
    project_dirs().map(|dirs| dirs.config_dir().to_path_buf())
}

/// Regenerable data (artwork caches).
pub fn cache_dir() -> CoreResult<PathBuf> {
    project_dirs().map(|dirs| dirs.cache_dir().to_path_buf())
}

/// Read-only, system-wide Basalt data installed by the package (e.g. `/usr/share/basalt`),
/// in priority order.
pub fn system_data_dirs() -> Vec<PathBuf> {
    let raw = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());

    std::env::split_paths(&raw)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("basalt"))
        .collect()
}

/// The pre-XDG `~/.basalt` directory that older builds stored everything in.
pub fn legacy_app_dir() -> CoreResult<PathBuf> {
    Ok(home_dir()?.join(LEGACY_APP_DIR_NAME))
}

fn project_dirs() -> CoreResult<ProjectDirs> {
    ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION).ok_or_else(
        || CoreError::new("Could not determine Basalt's application directories".to_string()),
    )
}

pub fn command_exists(command_name: &str) -> bool {
    platforms::command_exists(command_name)
}

/// Runs an executable that ships next to the current one (falling back to PATH),
/// waits for it to exit, and returns its exit status.
pub fn run_sibling_executable(name: &str) -> CoreResult<ExitStatus> {
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|candidate| candidate.is_file());

    let program = match sibling {
        Some(path) => path,
        None if command_exists(name) => PathBuf::from(name),
        None => {
            return Err(CoreError::new(format!(
                "Could not find '{}' next to this executable or on PATH",
                name
            )))
        }
    };

    Command::new(&program).status().map_err(|error| {
        CoreError::new(format!("Failed to start {}: {}", program.display(), error))
    })
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

pub fn normalize_script_path(raw_script_path: &str) -> CoreResult<String> {
    platforms::normalize_script_path(raw_script_path)
}

pub fn launch_script(script_path: &str) -> CoreResult<()> {
    platforms::launch_script(script_path)
}

pub fn launch_script_with_stdin(script_path: &str, stdin_content: &str) -> CoreResult<()> {
    platforms::launch_script_with_stdin(script_path, stdin_content)
}

pub fn run_command(command_name: &str, args: &[&str]) -> CoreResult<Output> {
    platforms::run_command(command_name, args)
}
