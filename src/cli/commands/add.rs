use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err(usage::usage_add());
    }

    core::add_game(args[1].trim(), args[2].trim())?;
    println!("Game added successfully.");
    Ok(())
}