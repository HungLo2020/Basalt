use crate::core;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt sync-mattmc".to_string());
    }

    core::run_game_sibling_script("MattMC", "SyncGameData.sh")?;
    println!("Ran sync script for MattMC.");
    Ok(())
}