use crate::core;

pub fn run(_args: &[String]) -> Result<(), String> {
    let entries = core::list_games()?;
    let playlists = core::list_playlists()?;

    println!("Games:");
    if entries.is_empty() {
        println!("  (none)");
    } else {
        for entry in entries {
            println!(
                "  {}\t{}\t{}",
                entry.name,
                entry.runner_kind.as_str(),
                entry.launch_target
            );
        }
    }

    println!("\nPlaylists:");
    for playlist in playlists {
        println!("  {}:", playlist.name);
        if playlist.game_names.is_empty() {
            println!("    (empty)");
        } else {
            for game_name in playlist.game_names {
                println!("    - {}", game_name);
            }
        }
    }

    Ok(())
}