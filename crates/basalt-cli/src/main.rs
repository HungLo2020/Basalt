#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

mod commands;
mod progress_printer;

use std::process::ExitCode;

use basalt_core as core;
use clap::{ArgGroup, Parser, Subcommand, ValueEnum};

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
    /// Add a game via script file, Steam appid/URL, or emulator launch target
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

    /// Discover games and add new entries (no flags = all discover runners)
    Discover {
        /// Discover Steam games
        #[arg(long)]
        steam: bool,
        /// Discover MattMC
        #[arg(long)]
        mattmc: bool,
        /// Discover emulator ROM entries
        #[arg(long)]
        emulators: bool,
    },

    /// Install a RetroArch core for one system key (e.g. nes, gba, snes, atari2600, nds, 3ds)
    InstallCore { system: String },

    /// Show whether a system core is installed and if save sync is supported
    CoreStatus { system: String },

    /// Install RetroArch runtime plus built-in emulator cores
    InstallEmulators,

    /// Download latest MattMC release into ~/Games/MattMC
    InstallMattmc,

    /// Launch a saved game by name
    Launch { name: String },

    /// Run backup.sh from the MattMC launch script directory
    BackupMattmc,

    /// Sync MattMC's game data with its remote copy (without a direction, the script asks)
    SyncMattmc {
        /// Which way to sync: `up` (local to remote) or `down` (remote to local)
        #[arg(value_enum)]
        direction: Option<SyncDirection>,
    },

    /// Run update-mattmc from the MattMC launch script directory
    UpdateMattmc,

    /// Sync up for a platform: mattmc or an emulator system key (e.g. nes, gba)
    SyncUp { platform: String },

    /// Sync down for a platform: mattmc or an emulator system key (emulators also run discover)
    SyncDown { platform: String },

    /// Sync local save files to remote for a system key
    SyncSavesUp { system: String },

    /// Sync remote save files to local for a system key
    SyncSavesDown { system: String },

    /// Show or change the remote ROM/Saves root directories used by sync commands
    Settings {
        #[command(subcommand)]
        action: SettingsCommand,
    },

    /// Clear cached artwork metadata and images
    RefreshMetadata,
}

#[derive(Clone, Copy, ValueEnum)]
enum SyncDirection {
    Up,
    Down,
}

#[derive(Subcommand)]
enum SettingsCommand {
    /// Show remote ROM/Saves root directories
    Get,

    /// Update remote ROM/Saves root directories
    #[command(group(ArgGroup::new("paths").required(true).multiple(true)))]
    Set {
        /// Remote ROMs root directory
        #[arg(long, group = "paths", value_name = "PATH")]
        roms_root: Option<String>,
        /// Remote saves root directory
        #[arg(long, group = "paths", value_name = "PATH")]
        saves_root: Option<String>,
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

        let cli = Cli::try_parse_from(["basalt", "discover", "--steam", "--emulators"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Discover {
                steam: true,
                mattmc: false,
                emulators: true
            })
        ));
    }

    #[test]
    fn sync_mattmc_takes_an_optional_direction() {
        let parse = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
            Some(Command::SyncMattmc { direction }) => {
                direction.map(|d| matches!(d, SyncDirection::Up))
            }
            _ => panic!("expected sync-mattmc"),
        };

        assert_eq!(parse(&["basalt", "sync-mattmc"]), None);
        assert_eq!(parse(&["basalt", "sync-mattmc", "up"]), Some(true));
        assert_eq!(parse(&["basalt", "sync-mattmc", "down"]), Some(false));
        assert!(Cli::try_parse_from(["basalt", "sync-mattmc", "sideways"]).is_err());
    }

    #[test]
    fn settings_set_requires_at_least_one_path() {
        assert!(Cli::try_parse_from(["basalt", "settings", "set"]).is_err());
        assert!(Cli::try_parse_from(["basalt", "settings", "set", "--roms-root", "/r"]).is_ok());
    }

    #[test]
    fn no_arguments_selects_gui() {
        assert!(Cli::try_parse_from(["basalt"]).unwrap().command.is_none());
    }
}
