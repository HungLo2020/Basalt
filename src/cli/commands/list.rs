use crate::core;

pub fn run(_args: &[String]) -> Result<(), String> {
    let entries = core::list_games()?;

    if entries.is_empty() {
        println!("No games added yet.");
        return Ok(());
    }

    for entry in entries {
        println!("{}\t{}", entry.name, entry.script_path);
    }

    Ok(())
}