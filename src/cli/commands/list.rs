use crate::core;

pub fn run(_args: &[String]) -> Result<(), String> {
    let entries = core::list_games()?;

    if entries.is_empty() {
        println!("No games added yet.");
        return Ok(());
    }

    for entry in entries {
        println!(
            "{}\t{}\t{}",
            entry.name,
            entry.runner_kind.as_str(),
            entry.launch_target
        );
    }

    Ok(())
}