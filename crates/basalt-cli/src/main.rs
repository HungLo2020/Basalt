#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

mod commands;
mod progress_printer;

use std::process::ExitCode;

use basalt_core as core;
use clap::{Parser, Subcommand};

const GUI_EXECUTABLE_NAME: &str = "basalt-gui";

/// Basalt game launcher.
///
/// Run without a command to open the Basalt GUI.
#[derive(Parser)]
#[command(name = "basalt", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Add a game via script file, Steam appid/URL, or catalog launch target
    Add { name: String, target: String },

    /// Remove a saved game by name
    Remove { name: String },

    /// Add a game to an existing playlist (e.g. Favorites)
    AddToPlaylist { playlist: String, name: String },

    /// Remove a game from an existing playlist
    RemoveFromPlaylist { playlist: String, name: String },

    /// Remove all saved games
    RemoveAll,

    /// List all added games and playlists
    List,

    /// Discover catalog installations and Steam games.
    Discover {
        #[arg(long)]
        steam: bool,
        #[arg(long)]
        catalog: bool,
    },

    /// List games available from the bundled catalog
    Catalog,

    /// Install/reinstall a catalog game; prompts for a library unless --library is supplied
    Install {
        game: String,
        #[arg(long)]
        library: Option<String>,
    },

    /// List or add game installation libraries
    Libraries {
        #[command(subcommand)]
        action: LibraryCommand,
    },

    /// Launch a saved game by name
    Launch { name: String },

    /// Run an action for an installed catalog game (see `basalt actions <game>`)
    Maintain {
        game: String,
        action: String,
        /// Backup to restore, for the restore action (see `basalt backups <game>`)
        #[arg(long)]
        snapshot: Option<String>,
        /// For sync actions: re-read every file on both sides instead of trusting cached
        /// hashes and the sync folder's file list
        #[arg(long)]
        verify: bool,
    },

    /// List the actions a catalog game supports
    Actions { game: String },

    /// List a catalog game's backups, newest first
    Backups { game: String },

    /// Show or set the folder game data is synced to
    SyncRemote {
        /// Absolute folder path, e.g. a mounted network share
        path: Option<std::path::PathBuf>,
        /// Go back to the default sync folder (/mnt/storage/Storage/Sync)
        #[arg(long, conflicts_with = "path")]
        clear: bool,
    },

    /// GitHub authentication (native login does not require gh)
    Github { #[command(subcommand)] action: GithubCommand },

    /// Clear cached artwork metadata and images
    RefreshMetadata,
}

#[derive(Subcommand)]
enum GithubCommand {
    Login,
    Status,
    Logout,
}

#[derive(Subcommand)]
enum LibraryCommand {
    List,
    Add {
        id: String,
        path: std::path::PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        return launch_gui();
    };

    let result = commands::run(command);

    // Non-fatal problems core ran into along the way.
    for warning in core::take_warnings() {
        eprintln!("Warning: {}", warning);
    }

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {}", error);
            ExitCode::FAILURE
        }
    }
}

/// `basalt` with no arguments opens the GUI, as it did before the CLI and GUI were split into
/// separate executables.
fn launch_gui() -> ExitCode {
    match basalt_core::run_sibling_executable(GUI_EXECUTABLE_NAME) {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("Error: {}", error);
            eprintln!("Run `basalt --help` for command-line usage.");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn subcommands_keep_their_kebab_case_names() {
        let cli = Cli::try_parse_from(["basalt", "add-to-playlist", "Favorites", "Doom"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::AddToPlaylist { ref playlist, ref name })
                if playlist == "Favorites" && name == "Doom"
        ));

        let cli = Cli::try_parse_from(["basalt", "discover", "--steam", "--catalog"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Discover {
                steam: true,
                catalog: true
            })
        ));
    }

    #[test]
    fn maintenance_is_generic_and_old_commands_are_rejected() {
        let cli = Cli::try_parse_from(["basalt", "maintain", "MattMC", "sync_up"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Maintain { snapshot: None, verify: false, .. })
        ));
        let cli = Cli::try_parse_from(["basalt", "maintain", "MattMC", "sync", "--verify"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Maintain { verify: true, .. })));
        let cli = Cli::try_parse_from([
            "basalt", "maintain", "Jackal", "restore", "--snapshot", "20260101T000000.000Z-manual",
        ])
        .unwrap();
        assert!(matches!(cli.command, Some(Command::Maintain { snapshot: Some(_), .. })));
        assert!(Cli::try_parse_from(["basalt", "sync-remote", "/mnt/x", "--clear"]).is_err());
        for obsolete in [
            "sync-mattmc",
            "backup-mattmc",
            "update-mattmc",
            "install-mattmc",
        ] {
            assert!(Cli::try_parse_from(["basalt", obsolete]).is_err());
        }
    }

    #[test]
    fn no_arguments_selects_gui() {
        assert!(Cli::try_parse_from(["basalt"]).unwrap().command.is_none());
    }
}
