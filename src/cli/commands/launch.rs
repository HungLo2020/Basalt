use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(usage::usage_launch());
    }

    core::launch_game(args[1].trim())?;
    Ok(())
}