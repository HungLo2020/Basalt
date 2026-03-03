use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err(usage::usage_remove_all());
    }

    let removed_count = core::remove_all_games()?;
    println!("Removed {} game entries.", removed_count);
    Ok(())
}