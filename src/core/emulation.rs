use std::path::{Path, PathBuf};

use super::emulation_target::EmulationLaunchTarget;
use super::emulator_systems;

mod autoconfig;
mod cores;
mod paths;
mod runtime;
mod sync;

pub use sync::RomSyncReport;

pub struct EmulationInstallReport {
    pub runtime_ready: bool,
    pub cores_ready: usize,
}

pub fn install_runtime_and_cores() -> Result<EmulationInstallReport, String> {
    paths::ensure_emulator_directories()?;
    let runtime_command = runtime::ensure_runtime_command()?;

    let _ = autoconfig::ensure_xbox_autoconfig_profiles();

    let mut cores_ready = 0usize;
    for core_spec in emulator_systems::emulator_system_specs() {
        if cores::ensure_core_installed(core_spec, &runtime_command)?.exists() {
            cores_ready += 1;
        }
    }

    Ok(EmulationInstallReport {
        runtime_ready: true,
        cores_ready,
    })
}

pub fn install_core_for_system(system: &str) -> Result<(), String> {
    paths::ensure_emulator_directories()?;
    let runtime_command = runtime::ensure_runtime_command()?;
    let core_spec = emulator_systems::emulator_system(system)
        .ok_or_else(|| format!("Unsupported emulator system: {}", system))?;
    cores::ensure_core_installed(core_spec, &runtime_command)?;
    Ok(())
}

pub fn is_core_installed_for_system(system: &str) -> Result<bool, String> {
    let core_spec = emulator_systems::emulator_system(system)
        .ok_or_else(|| format!("Unsupported emulator system: {}", system))?;
    let core_path = paths::retroarch_cores_dir()?.join(core_spec.core_file);
    Ok(core_path.exists() && core_path.is_file())
}

pub fn sync_roms_up_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync::sync_roms_up_for_system(system)
}

pub fn sync_roms_down_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync::sync_roms_down_for_system(system)
}

pub fn sync_saves_up_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync::sync_saves_up_for_system(system)
}

pub fn sync_saves_down_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync::sync_saves_down_for_system(system)
}

pub fn discoverable_systems() -> Vec<&'static str> {
    emulator_systems::discoverable_system_keys()
}

pub fn is_save_sync_supported_for_system(system: &str) -> bool {
    emulator_systems::emulator_system(system)
        .map(|core_spec| core_spec.supports_save_sync)
        .unwrap_or(false)
}

pub fn is_supported_rom_for_system(system: &str, file_path: &Path) -> bool {
    let Some(extension) = file_path.extension().and_then(|value| value.to_str()) else {
        return false;
    };

    let normalized_extension = extension.to_lowercase();
    emulator_systems::emulator_system(system)
        .map(|core_spec| {
            core_spec
                .rom_extensions
                .iter()
                .any(|expected| *expected == normalized_extension)
        })
        .unwrap_or(false)
}

pub fn roms_root_dir() -> Result<PathBuf, String> {
    paths::roms_root_dir()
}

pub fn ensure_emulator_directories() -> Result<(), String> {
    paths::ensure_emulator_directories()
}

pub fn build_launch_target(system: &str, rom_path: &Path) -> Result<String, String> {
    let system_key = paths::normalize_system_key(system)?;
    if emulator_systems::emulator_system(&system_key).is_none() {
        return Err(format!("Unsupported emulator system: {}", system));
    }

    let canonical_rom_path = paths::canonicalize_or_keep(rom_path);
    EmulationLaunchTarget::new_retroarch(system_key, canonical_rom_path)?.encode()
}

pub fn launch_target(launch_target: &str) -> Result<(), String> {
    paths::ensure_emulator_directories()?;
    let runtime_command = runtime::ensure_runtime_command()?;
    let _ = autoconfig::ensure_xbox_autoconfig_profiles();
    let parsed_launch_target = EmulationLaunchTarget::decode(launch_target)?;
    let system = paths::normalize_system_key(parsed_launch_target.system_key())?;
    let rom_path = parsed_launch_target.rom_path().to_path_buf();

    if !rom_path.exists() || !rom_path.is_file() {
        return Err(format!("ROM file does not exist: {}", rom_path.display()));
    }

    if !is_supported_rom_for_system(&system, &rom_path) {
        return Err(format!(
            "ROM extension is not supported for system '{}': {}",
            system,
            rom_path.display()
        ));
    }

    let core_spec = emulator_systems::emulator_system(&system)
        .ok_or_else(|| format!("Unsupported emulator system: {}", system))?;
    let core_path = cores::ensure_core_installed(core_spec, &runtime_command)?;

    runtime::launch_retroarch(&runtime_command, &system, &rom_path, &core_path)
}
