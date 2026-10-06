use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::paths;
use crate::error::{CoreError, CoreResult};

const JOYPAD_AUTOCONFIG_REPO_API_URL: &str =
    "https://api.github.com/repos/libretro/retroarch-joypad-autoconfig/contents";
const AUTOCONFIG_BACKENDS: [&str; 2] = ["udev", "sdl2"];
const USER_AGENT: &str = "Basalt-Emulation-Installer";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(15);

/// Makes sure Basalt's RetroArch autoconfig folder has Xbox controller profiles.
///
/// Only backends with no Xbox profiles on disk are fetched, so the network (and GitHub's
/// 60-requests-per-hour unauthenticated API limit) is touched once per machine, not on every
/// launch. Returns quickly and offline-safe when the profiles are already present.
pub(super) fn ensure_xbox_autoconfig_profiles() -> CoreResult<()> {
    let autoconfig_root = paths::retroarch_autoconfig_root_dir()?;

    for backend in AUTOCONFIG_BACKENDS {
        let backend_dir = autoconfig_root.join(backend);
        if has_xbox_profiles(&backend_dir) {
            continue;
        }
        download_xbox_profiles(backend, &backend_dir)?;
    }

    Ok(())
}

fn has_xbox_profiles(backend_dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(backend_dir) else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry.path().is_file()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".cfg") && is_xbox_profile_name(name))
    })
}

fn download_xbox_profiles(backend: &str, backend_dir: &Path) -> CoreResult<()> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .build();

    let backend_url = format!("{}/{}", JOYPAD_AUTOCONFIG_REPO_API_URL, backend);
    let payload = agent
        .get(&backend_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| CoreError::new(format!("Failed to fetch joypad profile list: {}", error)))?
        .into_string()
        .map_err(|error| {
            CoreError::new(format!(
                "Failed to read joypad profile list payload: {}",
                error
            ))
        })?;
    let listing: Value = serde_json::from_str(&payload).map_err(|error| {
        CoreError::new(format!("Failed to parse joypad profile list: {}", error))
    })?;

    let entries = listing.as_array().ok_or_else(|| {
        CoreError::new("Joypad profile listing has unexpected format".to_string())
    })?;

    fs::create_dir_all(backend_dir).map_err(|error| {
        CoreError::new(format!("Failed to create autoconfig directory: {}", error))
    })?;

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

        if let Err(error) = download_profile(&agent, download_url, &destination) {
            eprintln!(
                "Warning: Failed to download controller profile {}: {}",
                name, error
            );
        }
    }

    Ok(())
}

fn download_profile(agent: &ureq::Agent, url: &str, destination: &Path) -> CoreResult<()> {
    let contents = agent
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| CoreError::new(format!("Failed to download {}: {}", url, error)))?
        .into_string()
        .map_err(|error| CoreError::new(format!("Failed to read {}: {}", url, error)))?;

    fs::write(destination, contents).map_err(|error| {
        CoreError::new(format!(
            "Failed to save {}: {}",
            destination.display(),
            error
        ))
    })
}

fn is_xbox_profile_name(name: &str) -> bool {
    let normalized = name.to_lowercase();
    normalized.contains("xbox") || normalized.contains("x-box") || normalized.contains("microsoft")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_xbox_profiles_only_counts_xbox_cfg_files() {
        let dir =
            std::env::temp_dir().join(format!("basalt-autoconfig-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        assert!(!has_xbox_profiles(&dir), "missing folder has no profiles");

        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("DualShock 4.cfg"), "").unwrap();
        fs::write(dir.join("Xbox notes.txt"), "").unwrap();
        assert!(
            !has_xbox_profiles(&dir),
            "non-Xbox and non-.cfg files don't count"
        );

        fs::write(dir.join("Xbox Wireless Controller.cfg"), "").unwrap();
        assert!(has_xbox_profiles(&dir));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn xbox_profile_matching_accepts_common_xbox_names() {
        assert!(is_xbox_profile_name("Xbox Wireless Controller.cfg"));
        assert!(is_xbox_profile_name("Microsoft X-Box 360 pad.cfg"));
        assert!(!is_xbox_profile_name("DualShock 4.cfg"));
    }
}
