use basalt_core::{self as core, CoreResult, DiscoverResult, DiscoverRunner};

use super::progress_printer::ProgressPrinter;
use super::{Command, LibraryCommand};

pub(super) fn run(command: Command) -> CoreResult<()> {
    match command {
        Command::Catalog => {
            for game in core::bundled_games()? {
                println!("{}\t{}\t{}", game.id, game.name, game.source.repository);
            }
        }
        Command::Libraries { action } => match action {
            LibraryCommand::List => {
                for library in core::list_libraries()? {
                    println!("{}\t{}", library.id, library.path.display());
                }
            }
            LibraryCommand::Add { id, path } => {
                let library = core::add_library(&id, &path)?;
                println!("Added {}: {}", library.id, library.path.display());
            }
        },
        Command::Install { game, library } => {
            core::find_game(game.trim())?;
            let library = match library {
                Some(id) => id,
                None => select_install_library()?,
            };
            let printer = ProgressPrinter::new();
            let report = core::install_game(game.trim(), library.trim(), &printer.progress())?;
            printer.finish();
            println!(
                "Installed {} release '{}' into {}",
                report.game_name,
                report.release_tag,
                report.install_dir.display()
            );
        }
        Command::Add { name, target } => {
            core::add_game(name.trim(), target.trim())?;
            println!("Game added successfully.");
        }
        Command::Remove { name } => {
            core::remove_game(name.trim())?;
            println!("Game removed successfully.");
        }
        Command::AddToPlaylist { playlist, name } => {
            core::add_game_to_playlist(playlist.trim(), name.trim())?;
            println!("Added '{}' to playlist '{}'.", name.trim(), playlist.trim());
        }
        Command::RemoveFromPlaylist { playlist, name } => {
            core::remove_game_from_playlist(playlist.trim(), name.trim())?;
            println!(
                "Removed '{}' from playlist '{}'.",
                name.trim(),
                playlist.trim()
            );
        }
        Command::RemoveAll => {
            let removed_count = core::remove_all_games()?;
            println!("Removed {} game entries.", removed_count);
        }
        Command::List => list()?,
        Command::Discover { steam, catalog } => discover(steam, catalog)?,
        Command::Maintain { game, action } => {
            core::run_game_maintenance(game.trim(), action.trim())?;
            println!("Completed '{}' for '{}'.", action, game);
        }
        Command::Launch { name } => core::launch_game(name.trim())?,
        Command::RefreshMetadata => {
            core::clear_artwork_cache()?;
            println!("Artwork metadata and image caches were cleared.");
        }
    }

    Ok(())
}

/// Positional arguments are trimmed; one that is only whitespace counts as missing.
fn required<'a>(value: &'a str, name: &str) -> CoreResult<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(core::CoreError::InvalidInput(format!(
            "<{}> cannot be empty",
            name
        )))
    } else {
        Ok(trimmed)
    }
}

fn list() -> CoreResult<()> {
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

fn discover(steam: bool, catalog: bool) -> CoreResult<()> {
    let runners: Vec<DiscoverRunner> = [
        (steam, DiscoverRunner::Steam),
        (catalog, DiscoverRunner::Catalog),
    ].into_iter().filter_map(|(yes, runner)| yes.then_some(runner)).collect();
    let report = if runners.is_empty() {
        core::discover_games()?
    } else {
        core::discover_with_runners(&runners)?
    };
    if let Some(result) = report.catalog {
        println!("Catalog discovery: {}", match result {
            DiscoverResult::Added => "games discovered",
            DiscoverResult::AlreadyExists => "already registered",
            DiscoverResult::NotFound => "no new installations",
        });
    }
    if let Some(steam) = report.steam {
        println!("Steam discovery: found {}, added {}, existing {}",
            steam.found, steam.added, steam.already_exists);
    }
    Ok(())
}

fn select_install_library() -> CoreResult<String> {
    use std::io::{self, IsTerminal, Write};
    if !io::stdin().is_terminal() {
        return Err(core::CoreError::new(
            "Specify --library <id> for a noninteractive install (use 'basalt libraries list')",
        ));
    }
    let libraries = core::list_libraries()?;
    for (index, library) in libraries.iter().enumerate() {
        println!("{}. {} — {}", index + 1, library.id, library.path.display());
    }
    print!("Install into library [1]: ");
    io::stdout()
        .flush()
        .map_err(|e| core::CoreError::new(e.to_string()))?;
    let mut input = String::new();
    if io::stdin()
        .read_line(&mut input)
        .map_err(|e| core::CoreError::new(e.to_string()))?
        == 0
    {
        return Err(core::CoreError::new("Library selection cancelled"));
    }
    let choice = if input.trim().is_empty() {
        1
    } else {
        input
            .trim()
            .parse::<usize>()
            .map_err(|_| core::CoreError::new("Invalid library number"))?
    };
    libraries
        .get(choice.wrapping_sub(1))
        .map(|l| l.id.clone())
        .ok_or_else(|| core::CoreError::new("Invalid library number"))
}
