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

pub(crate) fn env_var(name: &str) -> Option<String> {
    env::var(name).ok()
}

pub(crate) fn env_var_os(name: &str) -> Option<std::ffi::OsString> {
    env::var_os(name)
}

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

/// Supplies private input to a command without exposing its output or input in errors.
pub(crate) fn run_command_with_private_input(
    command_name: &str,
    args: &[&str],
    input: &[u8],
) -> CoreResult<ExitStatus> {
    let mut child = Command::new(command_name)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| CoreError::new(format!("Cannot start {command_name}: {error}")))?;
    let written = child
        .stdin
        .take()
        .ok_or_else(|| CoreError::new("Missing command input"))
        .and_then(|mut stdin| {
            stdin
                .write_all(input)
                .map_err(|error| CoreError::new(format!("Cannot supply command input: {error}")))
        });
    let status = child
        .wait()
        .map_err(|error| CoreError::new(format!("Cannot wait for {command_name}: {error}")));
    written?;
    status
}

#[cfg(test)]
mod private_input_tests {
    use super::run_command_with_private_input;

    #[test]
    fn command_receives_input_and_returns_its_exit_status() {
        let args = ["-c", "test \"$(cat)\" = 'test credential'"];
        assert!(
            run_command_with_private_input("sh", &args, b"test credential")
                .unwrap()
                .success()
        );
        assert!(
            !run_command_with_private_input("sh", &args, b"different credential")
                .unwrap()
                .success()
        );
    }
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

/// Execute a catalog-specified maintenance script without a shell command string.
pub fn run_maintenance_script(
    path: &Path,
    cwd: &Path,
    arguments: &[String],
    input: Option<&str>,
) -> CoreResult<()> {
    if !path.is_file() || !cwd.is_dir() {
        return Err(CoreError::new(format!(
            "Maintenance script or directory missing: {}",
            path.display()
        )));
    }
    let mut command = Command::new("bash");
    command.arg(path).args(arguments).current_dir(cwd);
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command
        .spawn()
        .map_err(|err| CoreError::new(format!("Cannot launch maintenance script: {err}")))?;
    if let Some(input) = input
        && let Some(mut stdin) = child.stdin.take()
    {
        stdin
            .write_all(input.as_bytes())
            .map_err(|err| CoreError::new(format!("Cannot supply maintenance input: {err}")))?;
    }
    check_script_status(
        child
            .wait()
            .map_err(|err| CoreError::new(format!("Maintenance script failed: {err}")))?,
    )
}

/// Run a catalog executable with its declared arguments, working directory and environment,
/// waiting for it to exit. While it runs, `running_marker` (if any) holds its process ID so
/// other Basalt processes can tell the game is open.
pub(crate) fn launch_catalog_game(
    path: &Path,
    cwd: &Path,
    arguments: &[String],
    environment: &[(String, String)],
    running_marker: Option<&Path>,
) -> CoreResult<()> {
    let mut command = if path.extension().is_some_and(|ext| ext == "sh") {
        let mut command = Command::new("bash");
        command.arg(path);
        command
    } else {
        Command::new(path)
    };
    let mut child = command
        .args(arguments)
        .current_dir(cwd)
        .envs(environment.iter().map(|(key, value)| (key, value)))
        .spawn()
        .map_err(|e| CoreError::new(format!("Cannot launch game: {e}")))?;
    if let Some(marker) = running_marker {
        let written = marker
            .parent()
            .map(std::fs::create_dir_all)
            .unwrap_or(Ok(()))
            .and_then(|()| std::fs::write(marker, child.id().to_string()));
        if let Err(error) = written {
            crate::warnings::warn(format!(
                "Could not record that the game is running ({}): {error}",
                marker.display()
            ));
        }
    }
    let status = child
        .wait()
        .map_err(|e| CoreError::new(format!("Game process failed: {e}")));
    if let Some(marker) = running_marker {
        let _ = std::fs::remove_file(marker);
    }
    check_script_status(status?)
}

/// True if a process with this ID exists.
pub(crate) fn process_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

/// Standard input for a game script.
pub(crate) enum ScriptInput<'a> {
    None,
    Text(&'a str),
}

/// Runs a STANDARDS.md game script: `.sh` files with bash, anything else directly. Output lines
/// are echoed and reported as progress; on failure the last line written to standard error
/// becomes the error message. Cancellation kills the script.
pub(crate) fn run_game_script(
    path: &Path,
    cwd: &Path,
    arguments: &[String],
    input: ScriptInput<'_>,
    environment: &[(String, String)],
    progress: &crate::progress::Progress,
) -> CoreResult<()> {
    use std::io::{BufRead, BufReader};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    if !path.is_file() || !cwd.is_dir() {
        return Err(CoreError::new(format!(
            "Game script or directory missing: {}",
            path.display()
        )));
    }
    let mut command = if path.extension().is_some_and(|ext| ext == "sh") {
        let mut command = Command::new("bash");
        command.arg(path);
        command
    } else {
        Command::new(path)
    };
    command
        .args(arguments)
        .current_dir(cwd)
        .envs(environment.iter().map(|(key, value)| (key, value)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(match input {
            ScriptInput::None => Stdio::null(),
            ScriptInput::Text(_) => Stdio::piped(),
        });
    let mut child = command
        .spawn()
        .map_err(|err| CoreError::new(format!("Cannot start game script: {err}")))?;
    if let ScriptInput::Text(text) = input
        && let Some(mut stdin) = child.stdin.take()
    {
        // A script that exits without reading its input is not an error.
        let _ = stdin.write_all(text.as_bytes());
    }

    let (lines, received) = mpsc::channel::<String>();
    let stdout = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                println!("{line}");
                let _ = lines.send(line);
            }
        })
    });
    let last_error = Arc::new(Mutex::new(String::new()));
    let stderr = child.stderr.take().map(|stderr| {
        let last_error = Arc::clone(&last_error);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("{line}");
                if !line.trim().is_empty()
                    && let Ok(mut last) = last_error.lock()
                {
                    *last = line.trim().to_string();
                }
            }
        })
    });

    let mut cancelled = false;
    let status = loop {
        match received.recv_timeout(Duration::from_millis(100)) {
            Ok(line) if !line.trim().is_empty() => progress.report_step(line.trim()),
            _ => {}
        }
        if !cancelled && progress.check_cancelled().is_err() {
            cancelled = true;
            let _ = child.kill();
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|err| CoreError::new(format!("Game script failed: {err}")))?
        {
            break status;
        }
    };
    for handle in [stdout, stderr].into_iter().flatten() {
        let _ = handle.join();
    }
    while let Ok(line) = received.try_recv() {
        if !line.trim().is_empty() {
            progress.report_step(line.trim());
        }
    }
    if cancelled {
        return Err(CoreError::Cancelled);
    }
    if status.success() {
        return Ok(());
    }
    let reason = last_error.lock().map(|line| line.clone()).unwrap_or_default();
    match check_script_status(status) {
        Err(_) if !reason.is_empty() => Err(CoreError::new(reason)),
        other => other,
    }
}

/// Runs a game attached to the caller's terminal, preserving game output.
pub(crate) fn run_attached_command(program: &str, args: &[&str]) -> CoreResult<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| CoreError::new(format!("Failed to run {program}: {error}")))?;
    if !status.success() {
        return Err(CoreError::new(format!("{program} exited with {status}")));
    }
    Ok(())
}

/// Drains diagnostics while polling cancellation, so a verbose child cannot fill its pipe.
/// Flatpak owns transaction recovery; a cancelled transaction can be retried or discovered.
pub(crate) fn run_cancellable_command(
    program: &str,
    args: &[&str],
    progress: &crate::Progress,
) -> CoreResult<()> {
    use std::io::Read;
    use std::{thread, time::Duration};
    progress.check_cancelled()?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| CoreError::new(format!("Failed to run {program}: {error}")))?;
    let mut stderr = child.stderr.take().expect("piped stderr");
    let reader = thread::spawn(move || {
        let mut diagnostic = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(count) = stderr.read(&mut buffer) {
            if count == 0 {
                break;
            }
            let keep = count.min(16384_usize.saturating_sub(diagnostic.len()));
            diagnostic.extend_from_slice(&buffer[..keep]);
        }
        String::from_utf8_lossy(&diagnostic).into_owned()
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(CoreError::new(format!(
                    "Failed to wait for {program}: {error}"
                )));
            }
        }
        if let Err(error) = progress.check_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(error);
        }
        thread::sleep(Duration::from_millis(100));
    };
    let diagnostic = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(CoreError::new(format!(
            "{program} failed ({status}): {}",
            diagnostic.trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod cancellable_process_tests {
    use super::*;
    use crate::{CancelToken, Progress};
    use std::{
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn cancellation_stops_and_reaps_the_child() {
        let cancel = CancelToken::new();
        let progress = Progress::new(|_| {}, cancel.clone());
        let worker = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            cancel.cancel();
        });
        let start = Instant::now();
        let result = run_cancellable_command("/bin/sh", &["-c", "while :; do :; done"], &progress);
        worker.join().unwrap();
        assert!(matches!(result, Err(CoreError::Cancelled)));
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn drains_verbose_errors_and_checks_cancellation_before_spawn() {
        let result = run_cancellable_command("/bin/sh", &["-c",
            "i=0; while [ $i -lt 10000 ]; do printf 'verbose failure diagnostic\\n' >&2; i=$((i+1)); done; exit 1"
        ], &Progress::none()).unwrap_err().to_string();
        assert!(result.contains("verbose failure diagnostic"));
        assert!(result.len() < 17000);
        let cancel = CancelToken::new();
        cancel.cancel();
        let progress = Progress::new(|_| {}, cancel);
        assert!(matches!(
            run_cancellable_command("/must-not-be-spawned", &[], &progress),
            Err(CoreError::Cancelled)
        ));
    }
}
