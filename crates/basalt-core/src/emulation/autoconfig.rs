use std::fs;

use serde_json::Value;

use super::cores;
use super::paths;

const JOYPAD_AUTOCONFIG_REPO_API_URL: &str =
    "https://api.github.com/repos/libretro/retroarch-joypad-autoconfig/contents";

pub(super) fn ensure_xbox_autoconfig_profiles() -> Result<(), String> {
    for backend in ["udev", "sdl2"] {
        sync_xbox_autoconfig_backend(backend)?;
    }

    Ok(())
}

fn sync_xbox_autoconfig_backend(backend: &str) -> Result<(), String> {
    let backend_url = format!("{}/{}", JOYPAD_AUTOCONFIG_REPO_API_URL, backend);
    let response = ureq::get(&backend_url)
        .set("User-Agent", "Basalt-Emulation-Installer")
        .call()
        .map_err(|error| format!("Failed to fetch joypad profile list: {}", error))?;

    let payload = response
        .into_string()
        .map_err(|error| format!("Failed to read joypad profile list payload: {}", error))?;
    let listing: Value = serde_json::from_str(&payload)
        .map_err(|error| format!("Failed to parse joypad profile list: {}", error))?;

    let entries = listing
        .as_array()
        .ok_or_else(|| "Joypad profile listing has unexpected format".to_string())?;

    let backend_dir = paths::retroarch_autoconfig_root_dir()?.join(backend);
    fs::create_dir_all(&backend_dir)
        .map_err(|error| format!("Failed to create autoconfig directory: {}", error))?;

    for entry in entries {
        let Some(name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !is_xbox_profile_name(name) {
            continue;
        }

        let Some(download_url) = entry.get("download_url").and_then(Value::as_str) else {
            continue;
        };

        let destination = backend_dir.join(name);
        if destination.exists() {
            continue;
        }

        if let Err(error) = cores::download_file(download_url, &destination) {
            eprintln!(
                "Warning: Failed to download controller profile {}: {}",
                name, error
            );
        }
    }

    Ok(())
}

fn is_xbox_profile_name(name: &str) -> bool {
    let normalized = name.to_lowercase();
    normalized.contains("xbox") || normalized.contains("x-box") || normalized.contains("microsoft")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xbox_profile_matching_accepts_common_xbox_names() {
        assert!(is_xbox_profile_name("Xbox Wireless Controller.cfg"));
        assert!(is_xbox_profile_name("Microsoft X-Box 360 pad.cfg"));
        assert!(!is_xbox_profile_name("DualShock 4.cfg"));
    }
}
