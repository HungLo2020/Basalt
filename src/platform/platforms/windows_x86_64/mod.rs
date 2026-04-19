use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

pub fn normalize_script_path(raw_script_path: &str) -> Result<String, String> {
    let script_path = Path::new(raw_script_path);
    if !script_path.exists() || !script_path.is_file() {
        return Err(format!(
            "Script does not exist or is not a file: {}",
            raw_script_path
        ));
    }

    let extension = script_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            "Only script files are supported right now (expected .ps1, .bat, .cmd, or .sh file)"
                .to_string()
        })?;

    if !matches!(extension.as_str(), "ps1" | "bat" | "cmd" | "sh") {
        return Err(
            "Only script files are supported right now (expected .ps1, .bat, .cmd, or .sh file)"
                .to_string(),
        );
    }

    let canonical_script_path = std::fs::canonicalize(script_path)
        .map_err(|err| format!("Failed to resolve script path: {}", err))?;

    canonical_script_path
        .to_str()
        .ok_or_else(|| "Script path contains invalid UTF-8".to_string())
        .map(|value| value.to_string())
}

pub fn launch_script(script_path: &str) -> Result<(), String> {
    let path = Path::new(script_path);
    if !path.exists() || !path.is_file() {
        return Err(format!(
            "Saved script path does not exist or is not a file: {}",
            script_path
        ));
    }

    let status = base_command_for_script(path)?
        .status()
        .map_err(|err| format!("Failed to launch script: {}", err))?;

    if !status.success() {
        return Err(format!(
            "Script exited with non-zero status: {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string())
        ));
    }

    Ok(())
}

pub fn launch_script_with_stdin(script_path: &str, stdin_content: &str) -> Result<(), String> {
    let path = Path::new(script_path);
    if !path.exists() || !path.is_file() {
        return Err(format!(
            "Saved script path does not exist or is not a file: {}",
            script_path
        ));
    }

    let mut child = base_command_for_script(path)?
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Failed to launch script: {}", err))?;

    if let Some(mut stdin_pipe) = child.stdin.take() {
        stdin_pipe
            .write_all(stdin_content.as_bytes())
            .map_err(|err| format!("Failed to write stdin to script: {}", err))?;
    }

    let status = child
        .wait()
        .map_err(|err| format!("Failed while waiting for script process: {}", err))?;

    if !status.success() {
        return Err(format!(
            "Script exited with non-zero status: {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string())
        ));
    }

    Ok(())
}

fn base_command_for_script(script_path: &Path) -> Result<Command, String> {
    let extension = script_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            "Only script files are supported right now (expected .ps1, .bat, .cmd, or .sh file)"
                .to_string()
        })?;

    match extension.as_str() {
        "ps1" => {
            let mut command = Command::new(resolve_powershell_command()?);
            command
                .arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(script_path);
            Ok(command)
        }
        "bat" | "cmd" => {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(script_path);
            Ok(command)
        }
        "sh" => {
            if !command_exists("bash") {
                return Err(
                    "Bash is required to run .sh scripts on Windows, but was not found on PATH."
                        .to_string(),
                );
            }

            let mut command = Command::new("bash");
            command.arg(script_path);
            Ok(command)
        }
        _ => Err(
            "Only script files are supported right now (expected .ps1, .bat, .cmd, or .sh file)"
                .to_string(),
        ),
    }
}

fn resolve_powershell_command() -> Result<&'static str, String> {
    if command_exists("pwsh") {
        return Ok("pwsh");
    }

    if command_exists("powershell") {
        return Ok("powershell");
    }

    Err("PowerShell was not found on PATH.".to_string())
}

fn command_extensions() -> Vec<String> {
    let raw = env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".to_string());
    raw.split(';')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
        .collect()
}
