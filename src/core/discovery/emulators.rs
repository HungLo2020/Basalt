use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::emulation;
use crate::core::playlist_service;
use crate::core::registry;
use crate::core::runners::RunnerKind;
use crate::core::{CoreResult, EmulatorDiscoverReport, GameEntry};

pub fn discover_emulator_entries() -> CoreResult<EmulatorDiscoverReport> {
    emulation::ensure_emulator_directories()?;

    let roms_root = emulation::roms_root_dir()?;
    let mut discovered = Vec::new();

    for system in emulation::discoverable_systems() {
        let system_dir = roms_root.join(system);
        if !system_dir.exists() || !system_dir.is_dir() {
            continue;
        }

        let mut rom_paths = Vec::new();
        collect_files_recursive(&system_dir, &mut rom_paths)?;

        for rom_path in rom_paths {
            if !emulation::is_supported_rom_for_system(system, &rom_path) {
                continue;
            }

            let launch_target = emulation::build_launch_target(system, &rom_path)?;
            let entry_name = build_entry_name(system, &rom_path);

            discovered.push(GameEntry {
                name: entry_name,
                runner_kind: RunnerKind::Emulator,
                launch_target,
            });
        }
    }

    let mut report = EmulatorDiscoverReport {
        found: discovered.len(),
        added: 0,
        updated: 0,
        already_exists: 0,
    };

    let mut entries = registry::load_entries()?;
    let mut changed = false;

    let discovered_targets: std::collections::BTreeSet<String> = discovered
        .iter()
        .map(|entry| entry.launch_target.clone())
        .collect();

    let mut removed_names = Vec::new();
    entries.retain(|entry| {
        let is_managed_emulator_entry = entry.runner_kind == RunnerKind::Emulator
            && entry.launch_target.starts_with("retroarch|");

        if is_managed_emulator_entry && !discovered_targets.contains(&entry.launch_target) {
            removed_names.push(entry.name.clone());
            changed = true;
            false
        } else {
            true
        }
    });

    for discovered_entry in discovered {
        if let Some(existing_entry) = entries
            .iter_mut()
            .find(|entry| entry.name == discovered_entry.name)
        {
            if existing_entry.runner_kind == discovered_entry.runner_kind
                && existing_entry.launch_target == discovered_entry.launch_target
            {
                report.already_exists += 1;
                continue;
            }

            existing_entry.runner_kind = discovered_entry.runner_kind;
            existing_entry.launch_target = discovered_entry.launch_target;
            report.updated += 1;
            changed = true;
            continue;
        }

        if entries.iter().any(|entry| {
            entry.runner_kind == discovered_entry.runner_kind
                && entry.launch_target == discovered_entry.launch_target
        }) {
            report.already_exists += 1;
            continue;
        }

        entries.push(discovered_entry);
        report.added += 1;
        changed = true;
    }

    if changed {
        registry::save_entries(&entries)?;
        if !removed_names.is_empty() {
            playlist_service::remove_games_from_all_playlists(&removed_names)?;
        }
        playlist_service::sync_automatic_playlists()?;
    }

    Ok(report)
}

fn collect_files_recursive(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Failed to read ROM directory {}: {}", root.display(), error))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Failed to read ROM directory entry: {}", error))?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }

    Ok(())
}

fn build_entry_name(system: &str, rom_path: &Path) -> String {
    let file_stem = rom_path
        .file_stem()
        .unwrap_or_else(|| OsStr::new("Unknown ROM"))
        .to_string_lossy();
    format!("[{}] {}", system.to_uppercase(), file_stem)
}
