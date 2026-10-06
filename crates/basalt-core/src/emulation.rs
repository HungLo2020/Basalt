//! Emulation: RetroArch and its cores, ROM and save folders, syncing them with a remote root,
//! and launching ROMs.

use std::path::{Path, PathBuf};

use crate::discovery;
use crate::error::{CoreError, CoreResult};
use crate::progress::Progress;
use crate::types::{DiscoverRunner, EmulatorDiscoverReport};
use crate::warnings;

mod autoconfig;
mod cores;
mod launch_target;
mod paths;
mod runtime;
mod sync;
mod systems;

pub use launch_target::EmulatorLaunchTarget;
pub use sync::SyncReport;
use systems::EmulatorSystemSpec;
pub(crate) use systems::emulator_artwork_catalog_path;
pub use systems::{EmulatorSystem, emulator_systems};

pub struct EmulatorInstallReport {
    pub cores_installed: usize,
}

/// Installs RetroArch (if missing) and every supported core. An explicit install action: it may
/// install RetroArch system-wide.
pub fn install_emulators(progress: &Progress) -> CoreResult<EmulatorInstallReport> {
    paths::ensure_emulator_directories()?;
    progress.report_step("Checking RetroArch runtime");
    let runtime_command = runtime::ensure_runtime_command()?;
    progress.check_cancelled()?;

    progress.report_step("Checking controller profiles");
    install_controller_profiles();

    let specs = systems::emulator_system_specs();
    let mut cores_installed = 0usize;
    for (index, core_spec) in specs.iter().enumerate() {
        progress.check_cancelled()?;
        progress.report_items(
            format!("Installing {} core", core_spec.short_name),
            index as u64,
            specs.len() as u64,
        );
        // Per-core byte progress would fight the overall count, so cores report nothing here.
        let core_progress = Progress::cancel_only(progress);
        if cores::ensure_core_installed(core_spec, &runtime_command, &core_progress)?.exists() {
            cores_installed += 1;
        }
    }
    progress.report_items(
        "Emulator cores installed",
        specs.len() as u64,
        specs.len() as u64,
    );

    Ok(EmulatorInstallReport { cores_installed })
}

/// Installs one system's core (and RetroArch, if missing).
pub fn install_emulator_core(system: &str, progress: &Progress) -> CoreResult<()> {
    let core_spec = supported_system(system)?;
    paths::ensure_emulator_directories()?;
    progress.report_step("Checking RetroArch runtime");
    let runtime_command = runtime::ensure_runtime_command()?;
    cores::ensure_core_installed(core_spec, &runtime_command, progress)?;

    // Fetch controller profiles now, while the user expects network activity, so launches
    // don't have to. A no-op when they are already present.
    progress.report_step("Checking controller profiles");
    install_controller_profiles();
    Ok(())
}

/// Controller profiles are a convenience: failing to fetch them is a warning, not an error.
fn install_controller_profiles() {
    if let Err(error) = autoconfig::ensure_xbox_autoconfig_profiles() {
        warnings::warn(format!("Could not download controller profiles: {}", error));
    }
}

pub fn is_emulator_core_installed(system: &str) -> CoreResult<bool> {
    let core_spec = supported_system(system)?;
    let core_path = paths::retroarch_cores_dir()?.join(core_spec.core_file);
    Ok(core_path.is_file())
}

pub fn emulator_supports_save_sync(system: &str) -> bool {
    systems::emulator_system(system).is_some_and(|core_spec| core_spec.supports_save_sync)
}

/// Where a system's ROMs go, e.g. `~/Games/Emulators/roms/gba`.
pub fn emulator_rom_dir(system: &str) -> CoreResult<PathBuf> {
    let core_spec = supported_system(system)?;
    Ok(paths::roms_root_dir()?.join(core_spec.system_key))
}

/// Where a system's save files go, e.g. `~/Games/Emulators/saves/gba`.
pub fn emulator_save_dir(system: &str) -> CoreResult<PathBuf> {
    let core_spec = supported_system(system)?;
    Ok(paths::saves_root_dir()?.join(core_spec.system_key))
}

pub fn sync_roms_up(system: &str, progress: &Progress) -> CoreResult<SyncReport> {
    sync::sync_roms_up_for_system(system, progress)
}

/// Syncs a system's ROMs down from the remote root, then re-runs emulator discovery so the
/// library matches what is on disk.
pub fn sync_roms_down(
    system: &str,
    progress: &Progress,
) -> CoreResult<(SyncReport, EmulatorDiscoverReport)> {
    let sync_report = sync::sync_roms_down_for_system(system, progress)?;
    progress.report_step("Updating library");
    let discover_report = discovery::discover_with_runners(&[DiscoverRunner::Emulators])?;
    let emulator_report = discover_report.emulators.unwrap_or(EmulatorDiscoverReport {
        found: 0,
        added: 0,
        updated: 0,
        already_exists: 0,
    });

    Ok((sync_report, emulator_report))
}

pub fn sync_saves_up(system: &str, progress: &Progress) -> CoreResult<SyncReport> {
    sync::sync_saves_up_for_system(system, progress)
}

pub fn sync_saves_down(system: &str, progress: &Progress) -> CoreResult<SyncReport> {
    sync::sync_saves_down_for_system(system, progress)
}

fn supported_system(system: &str) -> CoreResult<&'static EmulatorSystemSpec> {
    systems::emulator_system(system).ok_or_else(|| CoreError::UnsupportedSystem(system.to_string()))
}

pub(crate) fn discoverable_systems() -> Vec<&'static str> {
    systems::discoverable_system_keys()
}

pub(crate) fn is_supported_rom_for_system(system: &str, file_path: &Path) -> bool {
    let Some(extension) = file_path.extension().and_then(|value| value.to_str()) else {
        return false;
    };

    let normalized_extension = extension.to_lowercase();
    systems::emulator_system(system)
        .map(|core_spec| {
            core_spec
                .rom_extensions
                .iter()
                .any(|expected| *expected == normalized_extension)
        })
        .unwrap_or(false)
}

pub(crate) fn roms_root_dir() -> CoreResult<PathBuf> {
    paths::roms_root_dir()
}

pub(crate) fn ensure_emulator_directories() -> CoreResult<()> {
    paths::ensure_emulator_directories()
}

pub(crate) fn build_launch_target(system: &str, rom_path: &Path) -> CoreResult<String> {
    let system_key = paths::normalize_system_key(system)?;
    if systems::emulator_system(&system_key).is_none() {
        return Err(CoreError::new(format!(
            "Unsupported emulator system: {}",
            system
        )));
    }

    let canonical_rom_path = paths::canonicalize_or_keep(rom_path);
    EmulatorLaunchTarget::new_retroarch(system_key, canonical_rom_path)?.encode()
}

/// Launches an emulator game. Never installs or downloads anything: a missing RetroArch or core
/// is reported with how to install it (from the Install page or the CLI).
pub(crate) fn launch_target(launch_target: &str) -> CoreResult<()> {
    paths::ensure_emulator_directories()?;
    let runtime_command = runtime::installed_runtime_command()?;
    let parsed_launch_target = EmulatorLaunchTarget::decode(launch_target)?;
    let system = paths::normalize_system_key(parsed_launch_target.system_key())?;
    let rom_path = parsed_launch_target.rom_path().to_path_buf();

    if !rom_path.exists() || !rom_path.is_file() {
        return Err(CoreError::new(format!(
            "ROM file does not exist: {}",
            rom_path.display()
        )));
    }

    if !is_supported_rom_for_system(&system, &rom_path) {
        return Err(CoreError::new(format!(
            "ROM extension is not supported for system '{}': {}",
            system,
            rom_path.display()
        )));
    }

    let core_spec = supported_system(&system)?;
    let core_path = cores::installed_core_path(core_spec)?;

    runtime::launch_retroarch(&runtime_command, &system, &rom_path, &core_path)
}
