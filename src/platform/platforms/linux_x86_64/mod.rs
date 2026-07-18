use std::env;
use std::fs;
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
        home.join("Library")
            .join("Application Support")
            .join("Steam"),
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

pub fn mattmc_sync_script_name() -> &'static str {
    "SyncGameData.sh"
}

pub fn mattmc_update_script_name() -> &'static str {
    "update-mattmc.sh"
}

pub fn mattmc_release_zip_suffix() -> &'static str {
    "linux-x64"
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

pub fn basalt_update_asset_suffix() -> &'static str {
    ".deb"
}

pub fn basalt_update_asset_marker() -> &'static str {
    "amd64"
}

pub fn install_basalt_update_and_restart(installer_path: &Path) -> Result<(), String> {
    if !installer_path.exists() || !installer_path.is_file() {
        return Err(format!(
            "Basalt update package was not found: {}",
            installer_path.display()
        ));
    }

    let current_exe = env::current_exe()
        .map_err(|error| format!("Failed to resolve current Basalt executable: {}", error))?;
    let helper_path = env::temp_dir().join(format!(
        "basalt-update-helper-{}-{}.sh",
        std::process::id(),
        update_helper_timestamp()
    ));
    let install_command = if command_exists("pkexec") {
        format!(
            "pkexec dpkg -i {}",
            shell_single_quoted_path(installer_path)
        )
    } else if command_exists("sudo") {
        let sudo_check = Command::new("sudo")
            .arg("-n")
            .arg("true")
            .status()
            .map_err(|error| format!("Failed to check sudo availability: {}", error))?;
        if !sudo_check.success() {
            return Err(
                "Basalt updates require pkexec or an active non-interactive sudo session."
                    .to_string(),
            );
        }

        format!(
            "sudo -n dpkg -i {}",
            shell_single_quoted_path(installer_path)
        )
    } else {
        return Err(
            "Basalt updates require pkexec or sudo to install the .deb package.".to_string(),
        );
    };

    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
installer_path={installer_path}
basalt_exe={current_exe}
parent_pid={parent_pid}

while kill -0 "$parent_pid" >/dev/null 2>&1; do
  sleep 0.2
done

{install_command}
rm -f "$installer_path"
nohup "$basalt_exe" >/dev/null 2>&1 &
rm -f "$0"
"#,
        installer_path = shell_single_quoted_path(installer_path),
        current_exe = shell_single_quoted_path(&current_exe),
        parent_pid = std::process::id(),
        install_command = install_command,
    );

    fs::write(&helper_path, script).map_err(|error| {
        format!(
            "Failed to write Basalt update helper {}: {}",
            helper_path.display(),
            error
        )
    })?;

    Command::new("chmod")
        .arg("+x")
        .arg(&helper_path)
        .status()
        .map_err(|error| format!("Failed to mark update helper executable: {}", error))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Failed to mark update helper executable: chmod exited with {}",
                    status
                ))
            }
        })?;

    Command::new("bash")
        .arg(&helper_path)
        .spawn()
        .map_err(|error| {
            format!(
                "Failed to start Basalt update helper {}: {}",
                helper_path.display(),
                error
            )
        })?;

    std::process::exit(0);
}

pub fn can_install_basalt_updates() -> bool {
    true
}

fn shell_single_quoted_path(path: &Path) -> String {
    let escaped = path.to_string_lossy().replace('\'', "'\\''");
    format!("'{}'", escaped)
}

fn update_helper_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
