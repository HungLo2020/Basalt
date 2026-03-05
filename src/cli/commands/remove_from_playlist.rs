use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err(usage::usage_remove_from_playlist());
    }

    let playlist_name = args[1].trim();
    let game_name = args[2].trim();

    core::remove_game_from_playlist(playlist_name, game_name)?;
    println!(
        "Removed '{}' from playlist '{}'.",
        game_name,
        playlist_name
    );
    Ok(())
}
