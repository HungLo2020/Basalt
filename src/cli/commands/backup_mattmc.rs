use crate::core;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt backup-mattmc".to_string());
    }

    core::run_game_sibling_script("MattMC", "backup.sh")?;
    println!("Ran backup script for MattMC.");
    Ok(())
}