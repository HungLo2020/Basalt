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
            SyncDirection::Up => usage::usage_sync_saves_up(),
            SyncDirection::Down => usage::usage_sync_saves_down(),
        });
    }

    let system = args[1].trim();

    if system.is_empty() {
        return Err(match direction {
            SyncDirection::Up => usage::usage_sync_saves_up(),
            SyncDirection::Down => usage::usage_sync_saves_down(),
        });
    }

    match direction {
        SyncDirection::Up => {
            let report = core::sync_emulation_saves_up_for_system(system)?;
            println!(
                "Sync Saves Up ({}) complete: copied {}, unchanged {}, deleted {}.",
                system.to_uppercase(),
                report.copied,
                report.unchanged,
                report.deleted
            );
        }
        SyncDirection::Down => {
            let report = core::sync_emulation_saves_down_for_system(system)?;
            println!(
                "Sync Saves Down ({}) complete: copied {}, unchanged {}, deleted {}.",
                system.to_uppercase(),
                report.copied,
                report.unchanged,
                report.deleted
            );
        }
    }

    Ok(())
}
