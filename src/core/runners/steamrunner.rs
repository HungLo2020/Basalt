use std::process::Command;

use crate::platform;

pub fn detect_appid(raw_input: &str) -> Option<String> {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().all(|value| value.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }

    if let Some(value) = trimmed.strip_prefix("steam://rungameid/") {
        if value.chars().all(|char_value| char_value.is_ascii_digit()) {
            return Some(value.to_string());
        }
    }

    if let Some(value) = trimmed.strip_prefix("steam://run/") {
        if value.chars().all(|char_value| char_value.is_ascii_digit()) {
            return Some(value.to_string());
        }
    }

    if let Some(value) = trimmed.strip_prefix("steam:appid:") {
        if value.chars().all(|char_value| char_value.is_ascii_digit()) {
            return Some(value.to_string());
        }
    }

    if let Some(value) = trimmed.strip_prefix("steam-appid:") {
        if value.chars().all(|char_value| char_value.is_ascii_digit()) {
            return Some(value.to_string());
        }
    }

    None
}

pub fn launch(appid: &str) -> Result<(), String> {
    if appid.is_empty() || !appid.chars().all(|value| value.is_ascii_digit()) {
        return Err(format!("Invalid Steam appid: {}", appid));
    }

    let status = if platform::command_exists("steam") {
        Command::new("steam")
            .arg("-applaunch")
            .arg(appid)
            .status()
            .map_err(|err| format!("Failed to launch Steam app via steam command: {}", err))?
    } else if platform::command_exists("flatpak") && flatpak_has_steam()? {
        Command::new("flatpak")
            .arg("run")
            .arg("com.valvesoftware.Steam")
            .arg("-applaunch")
            .arg(appid)
            .status()
            .map_err(|err| format!("Failed to launch Steam app via flatpak: {}", err))?
    } else if cfg!(target_os = "macos") && platform::command_exists("open") {
        Command::new("open")
            .arg(format!("steam://rungameid/{}", appid))
            .status()
            .map_err(|err| format!("Failed to launch Steam app via open command: {}", err))?
    } else {
        return Err("Steam is not installed or not on PATH.".to_string());
    };

    if !status.success() {
        return Err(format!(
            "Steam launch exited with non-zero status: {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string())
        ));
    }

    Ok(())
}
fn flatpak_has_steam() -> Result<bool, String> {
    let status = Command::new("flatpak")
        .arg("info")
        .arg("com.valvesoftware.Steam")
        .status()
        .map_err(|err| format!("Failed to check flatpak Steam installation: {}", err))?;

    Ok(status.success())
}