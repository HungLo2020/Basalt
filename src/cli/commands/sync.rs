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
            SyncDirection::Up => usage::usage_sync_up(),
            SyncDirection::Down => usage::usage_sync_down(),
        });
    }

    let platform = args[1].trim();

    if platform.is_empty() {
        return Err(match direction {
            SyncDirection::Up => usage::usage_sync_up(),
            SyncDirection::Down => usage::usage_sync_down(),
        });
    }

    if platform.eq_ignore_ascii_case("mattmc") {
        match direction {
            SyncDirection::Up => {
                core::sync_mattmc_up()?;
                println!("Ran sync-up script for MattMC.");
            }
            SyncDirection::Down => {
                core::sync_mattmc_down()?;
                println!("Ran sync-down script for MattMC.");
            }
        }

        return Ok(());
    }

    match direction {
        SyncDirection::Up => {
            let report = core::sync_emulation_roms_up_for_system(platform)?;
            println!(
                "Sync Up ({}) complete: copied {}, unchanged {}, deleted {}.",
                platform.to_uppercase(),
                report.copied,
                report.unchanged,
                report.deleted
            );
        }
        SyncDirection::Down => {
            let (sync_report, emulator_report) =
                core::sync_emulation_roms_down_and_discover_for_system(platform)?;
            println!(
                "Sync Down ({}) complete: copied {}, unchanged {}, deleted {}.",
                platform.to_uppercase(),
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
