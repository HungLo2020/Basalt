use std::process::Command;

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

    let status = if command_available("steam") {
        Command::new("steam")
            .arg("-applaunch")
            .arg(appid)
            .status()
            .map_err(|err| format!("Failed to launch Steam app via steam command: {}", err))?
    } else if command_available("flatpak") && flatpak_has_steam()? {
        Command::new("flatpak")
            .arg("run")
            .arg("com.valvesoftware.Steam")
            .arg("-applaunch")
            .arg(appid)
            .status()
            .map_err(|err| format!("Failed to launch Steam app via flatpak: {}", err))?
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

fn command_available(command: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", command))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn flatpak_has_steam() -> Result<bool, String> {
    let status = Command::new("flatpak")
        .arg("info")
        .arg("com.valvesoftware.Steam")
        .status()
        .map_err(|err| format!("Failed to check flatpak Steam installation: {}", err))?;

    Ok(status.success())
}