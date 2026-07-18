use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err(usage::usage_update_mattmc());
    }

    core::update_mattmc()?;
    println!("Ran update script for MattMC.");
    Ok(())
}
