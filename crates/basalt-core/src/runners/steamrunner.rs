use crate::error::{CoreError, CoreResult};
use crate::platform;

pub fn detect_appid(raw_input: &str) -> Option<String> {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().all(|value| value.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }

    if let Some(value) = trimmed.strip_prefix("steam://rungameid/")
        && value.chars().all(|char_value| char_value.is_ascii_digit())
    {
        return Some(value.to_string());
    }

    if let Some(value) = trimmed.strip_prefix("steam://run/")
        && value.chars().all(|char_value| char_value.is_ascii_digit())
    {
        return Some(value.to_string());
    }

    if let Some(value) = trimmed.strip_prefix("steam:appid:")
        && value.chars().all(|char_value| char_value.is_ascii_digit())
    {
        return Some(value.to_string());
    }

    if let Some(value) = trimmed.strip_prefix("steam-appid:")
        && value.chars().all(|char_value| char_value.is_ascii_digit())
    {
        return Some(value.to_string());
    }

    None
}

pub fn launch(appid: &str) -> CoreResult<()> {
    if appid.is_empty() || !appid.chars().all(|value| value.is_ascii_digit()) {
        return Err(CoreError::new(format!("Invalid Steam appid: {}", appid)));
    }

    let output = if platform::command_exists("steam") {
        platform::run_command("steam", &["-applaunch", appid]).map_err(|err| {
            CoreError::new(format!(
                "Failed to launch Steam app via steam command: {}",
                err
            ))
        })?
    } else if platform::command_exists("flatpak") && flatpak_has_steam()? {
        platform::run_command(
            "flatpak",
            &["run", "com.valvesoftware.Steam", "-applaunch", appid],
        )
        .map_err(|err| CoreError::new(format!("Failed to launch Steam app via flatpak: {}", err)))?
    } else {
        return Err(CoreError::new(
            "Steam is not installed or not on PATH.".to_string(),
        ));
    };

    if !output.status.success() {
        return Err(CoreError::new(format!(
            "Steam launch exited with non-zero status: {}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string())
        )));
    }

    Ok(())
}
fn flatpak_has_steam() -> CoreResult<bool> {
    let output =
        platform::run_command("flatpak", &["info", "com.valvesoftware.Steam"]).map_err(|err| {
            CoreError::new(format!(
                "Failed to check flatpak Steam installation: {}",
                err
            ))
        })?;

    Ok(output.status.success())
}
