use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err(usage::usage_add_to_playlist());
    }

    let playlist_name = args[1].trim();
    let game_name = args[2].trim();

    core::add_game_to_playlist(playlist_name, game_name)?;
    println!(
        "Added '{}' to playlist '{}'.",
        game_name,
        playlist_name
    );
    Ok(())
}
