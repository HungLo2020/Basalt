use std::fs;
use std::io::copy;
use std::path::{Path, PathBuf};

use super::paths;
use super::runtime::{self, RuntimeCommand};
use crate::core::emulator_systems::EmulatorSystemSpec;

pub(super) fn ensure_core_installed(
    core_spec: &EmulatorSystemSpec,
    _runtime_command: &RuntimeCommand,
) -> Result<PathBuf, String> {
    let core_path = paths::retroarch_cores_dir()?.join(core_spec.core_file);
    if core_path.exists() {
        return Ok(core_path);
    }

    if !runtime::command_exists("unzip") {
        return Err("unzip is required to install RetroArch cores automatically.".to_string());
    }

    let archive_name = format!("{}.zip", core_spec.core_file);
    let archive_path = paths::retroarch_runtime_dir()?.join(archive_name);
    download_file(core_spec.archive_url, &archive_path)?;

    let archive_string = archive_path
        .to_str()
        .ok_or_else(|| "Core archive path contains invalid UTF-8".to_string())?;
    let cores_dir_string = paths::retroarch_cores_dir()?
        .to_str()
        .ok_or_else(|| "Core directory path contains invalid UTF-8".to_string())?
        .to_string();

    runtime::run_command("unzip", &["-o", archive_string, "-d", &cores_dir_string])?;

    if let Err(error) = fs::remove_file(&archive_path) {
        eprintln!(
            "Warning: Failed to remove temporary core archive at {}: {}",
            archive_path.display(),
            error
        );
    }

    if core_path.exists() {
        Ok(core_path)
    } else {
        Err(format!(
            "Core installation did not produce expected file: {}",
            core_path.display()
        ))
    }
}

pub(super) fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    let response = ureq::get(url)
        .set("User-Agent", "Basalt-Emulation-Installer")
        .call()
        .map_err(|error| format!("Failed to download {}: {}", url, error))?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create download directory: {}", error))?;
    }

    let mut reader = response.into_reader();
    let mut file = fs::File::create(destination)
        .map_err(|error| format!("Failed to create file {}: {}", destination.display(), error))?;
    copy(&mut reader, &mut file)
        .map_err(|error| format!("Failed to save {}: {}", destination.display(), error))?;

    Ok(())
}
