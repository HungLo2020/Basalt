use crate::core;

use super::super::usage;

enum SyncDirection {
    Up,
    Down,
}

pub fn run_up(args: &[String]) -> Result<(), String> {
    run_with_direction(args, SyncDirection::Up)
}

pub fn run_down(args: &[String]) -> Result<(), String> {
    run_with_direction(args, SyncDirection::Down)
}

fn run_with_direction(args: &[String], direction: SyncDirection) -> Result<(), String> {
    if args.len() != 2 {
        return Err(match direction {
            SyncDirection::Up => usage::usage_sync_roms_up(),
            SyncDirection::Down => usage::usage_sync_roms_down(),
        });
    }

    let system = args[1].trim();

    if system.is_empty() {
        return Err(match direction {
            SyncDirection::Up => usage::usage_sync_roms_up(),
            SyncDirection::Down => usage::usage_sync_roms_down(),
        });
    }

    match direction {
        SyncDirection::Up => {
            let report = core::sync_emulation_roms_up_for_system(system)?;
            println!(
                "Sync Roms Up ({}) complete: copied {}, unchanged {}, deleted {}.",
                system.to_uppercase(),
                report.copied,
                report.unchanged,
                report.deleted
            );
        }
        SyncDirection::Down => {
            let (sync_report, emulator_report) =
                core::sync_emulation_roms_down_and_discover_for_system(system)?;
            println!(
                "Sync Roms Down ({}) complete: copied {}, unchanged {}, deleted {}.",
                system.to_uppercase(),
                sync_report.copied,
                sync_report.unchanged,
                sync_report.deleted
            );
            println!(
                "Emulator discover: found {}, added {}, updated {}, already existed {}.",
                emulator_report.found,
                emulator_report.added,
                emulator_report.updated,
                emulator_report.already_exists
            );
        }
    }

    Ok(())
}
