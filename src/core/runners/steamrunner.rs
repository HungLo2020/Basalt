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

    let output = if platform::command_exists("steam") {
        platform::run_command("steam", &["-applaunch", appid])
            .map_err(|err| format!("Failed to launch Steam app via steam command: {}", err))?
    } else if platform::command_exists("flatpak") && flatpak_has_steam()? {
        platform::run_command(
            "flatpak",
            &["run", "com.valvesoftware.Steam", "-applaunch", appid],
        )
        .map_err(|err| format!("Failed to launch Steam app via flatpak: {}", err))?
    } else if cfg!(target_os = "macos") && platform::command_exists("open") {
        let steam_url = format!("steam://rungameid/{}", appid);
        platform::run_command("open", &[&steam_url])
            .map_err(|err| format!("Failed to launch Steam app via open command: {}", err))?
    } else {
        return Err("Steam is not installed or not on PATH.".to_string());
    };

    if !output.status.success() {
        return Err(format!(
            "Steam launch exited with non-zero status: {}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string())
        ));
    }

    Ok(())
}
fn flatpak_has_steam() -> Result<bool, String> {
    let output = platform::run_command("flatpak", &["info", "com.valvesoftware.Steam"])
        .map_err(|err| format!("Failed to check flatpak Steam installation: {}", err))?;

    Ok(output.status.success())
}