use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err(usage::usage_sync_mattmc_up());
    }

    core::sync_mattmc_up()?;
    println!("Ran sync-up script for MattMC.");
    Ok(())
}
