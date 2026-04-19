use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const APP_DIR_NAME: &str = ".basalt";
const MATTMC_LAUNCH_SCRIPT_CANDIDATES: &[&str] = &["run-mattmc.sh"];

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
        home.join(".var")
            .join("app")
            .join("com.valvesoftware.Steam")
            .join(".local")
            .join("share")
            .join("Steam"),
    ]
}

pub fn mattmc_launch_script_candidates() -> &'static [&'static str] {
    MATTMC_LAUNCH_SCRIPT_CANDIDATES
}

pub fn normalize_script_path(raw_script_path: &str) -> Result<String, String> {
    let script_path = Path::new(raw_script_path);
    if !script_path.exists() || !script_path.is_file() {
        return Err(format!(
            "Script does not exist or is not a file: {}",
            raw_script_path
        ));
    }

    let has_sh_extension = script_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("sh"))
        .unwrap_or(false);

    if !has_sh_extension {
        return Err("Only bash scripts are supported right now (expected .sh file)".to_string());
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

    let status = Command::new("bash")
        .arg(path)
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

    let mut child = Command::new("bash")
        .arg(path)
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

pub fn run_command(command_name: &str, args: &[&str]) -> Result<Output, String> {
    Command::new(command_name)
        .args(args)
        .output()
        .map_err(|error| format!("Failed to execute {}: {}", command_name, error))
}
