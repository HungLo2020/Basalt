//! Operating-system access: directories, environment, and processes.
//!
//! The only module allowed to touch `std::env` and `std::process::Command` (enforced by clippy,
//! see clippy.toml), so every OS assumption lives here.

#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

#[cfg(not(target_os = "linux"))]
compile_error!("Basalt only supports Linux.");

use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

use directories::{BaseDirs, ProjectDirs};

use crate::error::{CoreError, CoreResult};

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "hunglo2020";
const PROJECT_APPLICATION: &str = "Basalt";
const LEGACY_APP_DIR_NAME: &str = ".basalt";

// ----- directories -------------------------------------------------------------------------

pub fn home_dir() -> CoreResult<PathBuf> {
    BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .ok_or_else(|| CoreError::new("Could not determine the home directory"))
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
    let raw = env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());

    env::split_paths(&raw)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("basalt"))
        .collect()
}

/// The pre-XDG `~/.basalt` directory that older builds stored everything in.
pub fn legacy_app_dir() -> CoreResult<PathBuf> {
    Ok(home_dir()?.join(LEGACY_APP_DIR_NAME))
}

fn project_dirs() -> CoreResult<ProjectDirs> {
    ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION)
        .ok_or_else(|| CoreError::new("Could not determine Basalt's application directories"))
}

// ----- processes ---------------------------------------------------------------------------

pub fn command_exists(command_name: &str) -> bool {
    let Some(path_value) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path_value).any(|directory| directory.join(command_name).is_file())
}

/// Runs a command to completion and returns its output.
pub fn run_command(command_name: &str, args: &[&str]) -> CoreResult<Output> {
    Command::new(command_name)
        .args(args)
        .output()
        .map_err(|error| CoreError::new(format!("Failed to execute {}: {}", command_name, error)))
}

/// Runs an executable that ships next to the current one (falling back to PATH),
/// waits for it to exit, and returns its exit status.
pub fn run_sibling_executable(name: &str) -> CoreResult<ExitStatus> {
    let sibling = env::current_exe()
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
            )));
        }
    };

    Command::new(&program).status().map_err(|error| {
        CoreError::new(format!("Failed to start {}: {}", program.display(), error))
    })
}

// ----- game scripts ------------------------------------------------------------------------

/// Validates a script given as a game target and returns its canonical path.
pub fn normalize_script_path(raw_script_path: &str) -> CoreResult<String> {
    let script_path = Path::new(raw_script_path);
    if !script_path.is_file() {
        return Err(CoreError::new(format!(
            "Script does not exist or is not a file: {}",
            raw_script_path
        )));
    }

    let has_sh_extension = script_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("sh"));
    if !has_sh_extension {
        return Err(CoreError::new(
            "Only bash scripts are supported right now (expected .sh file)",
        ));
    }

    let canonical_script_path = std::fs::canonicalize(script_path)
        .map_err(|err| CoreError::new(format!("Failed to resolve script path: {}", err)))?;

    canonical_script_path
        .to_str()
        .ok_or_else(|| CoreError::new("Script path contains invalid UTF-8"))
        .map(|value| value.to_string())
}

/// Runs a script with bash, inheriting the terminal, and waits for it to finish.
pub fn launch_script(script_path: &str) -> CoreResult<()> {
    let path = existing_script(script_path)?;
    let status = Command::new("bash")
        .arg(path)
        .status()
        .map_err(|err| CoreError::new(format!("Failed to launch script: {}", err)))?;
    check_script_status(status)
}

/// Runs a script with bash, writing `stdin_content` to its standard input (for scripts that
/// prompt), and waits for it to finish.
pub fn launch_script_with_stdin(script_path: &str, stdin_content: &str) -> CoreResult<()> {
    let path = existing_script(script_path)?;
    let mut child = Command::new("bash")
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| CoreError::new(format!("Failed to launch script: {}", err)))?;

    if let Some(mut stdin_pipe) = child.stdin.take() {
        stdin_pipe
            .write_all(stdin_content.as_bytes())
            .map_err(|err| CoreError::new(format!("Failed to write stdin to script: {}", err)))?;
    }

    let status = child.wait().map_err(|err| {
        CoreError::new(format!("Failed while waiting for script process: {}", err))
    })?;
    check_script_status(status)
}

fn existing_script(script_path: &str) -> CoreResult<&Path> {
    let path = Path::new(script_path);
    if path.is_file() {
        Ok(path)
    } else {
        Err(CoreError::new(format!(
            "Saved script path does not exist or is not a file: {}",
            script_path
        )))
    }
}

fn check_script_status(status: ExitStatus) -> CoreResult<()> {
    if status.success() {
        return Ok(());
    }

    Err(CoreError::new(format!(
        "Script exited with non-zero status: {}",
        status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string())
    )))
}
