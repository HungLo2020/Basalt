use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err(usage::usage_sync_mattmc_down());
    }

    core::sync_mattmc_down()?;
    println!("Ran sync-down script for MattMC.");
    Ok(())
}
