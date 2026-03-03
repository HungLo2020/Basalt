use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(usage::usage_remove());
    }

    core::remove_game(args[1].trim())?;
    println!("Game removed successfully.");
    Ok(())
}