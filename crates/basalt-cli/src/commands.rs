use basalt_core::{self as core, CoreResult, DiscoverResult, DiscoverRunner};

use super::progress_printer::ProgressPrinter;
use super::{Command, SettingsCommand, SyncDirection};

pub(super) fn run(command: Command) -> CoreResult<()> {
    match command {
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
        Command::Discover {
            steam,
            mattmc,
            emulators,
        } => discover(steam, mattmc, emulators)?,
        Command::InstallCore { system } => {
            let system = required(&system, "system")?;
            let printer = ProgressPrinter::new();
            core::install_emulator_core(system, &printer.progress())?;
            printer.finish();
            println!("Installed emulator core for system '{}'.", system);
        }
        Command::CoreStatus { system } => core_status(required(&system, "system")?)?,
        Command::InstallEmulators => {
            let printer = ProgressPrinter::new();
            let report = core::install_emulators(&printer.progress())?;
            printer.finish();
            println!(
                "RetroArch is ready; {} emulator cores installed.",
                report.cores_installed
            );
            print_emulator_folders()?;
        }
        Command::InstallMattmc => {
            let printer = ProgressPrinter::new();
            let report = core::install_mattmc(&printer.progress())?;
            printer.finish();
            println!(
                "Installed MattMC release '{}' into {}",
                report.release_tag,
                report.install_dir.display()
            );
            println!("{}", report.discovery_message());
        }
        Command::Launch { name } => core::launch_game(name.trim())?,
        Command::BackupMattmc => {
            core::backup_mattmc()?;
            println!("Ran backup script for MattMC.");
        }
        Command::SyncMattmc { direction } => match direction {
            Some(SyncDirection::Up) => {
                core::sync_mattmc_up()?;
                println!("Ran sync-up script for MattMC.");
            }
            Some(SyncDirection::Down) => {
                core::sync_mattmc_down()?;
                println!("Ran sync-down script for MattMC.");
            }
            None => {
                core::sync_mattmc_interactive()?;
                println!("Ran sync script for MattMC.");
            }
        },
        Command::UpdateMattmc => {
            core::update_mattmc()?;
            println!("Ran update script for MattMC.");
        }
        Command::SyncUp { platform } => sync_roms(required(&platform, "platform")?, true)?,
        Command::SyncDown { platform } => sync_roms(required(&platform, "platform")?, false)?,
        Command::SyncSavesUp { system } => sync_saves(required(&system, "system")?, true)?,
        Command::SyncSavesDown { system } => sync_saves(required(&system, "system")?, false)?,
        Command::Settings { action } => settings(action)?,
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

fn discover(steam: bool, mattmc: bool, emulators: bool) -> CoreResult<()> {
    let runners: Vec<DiscoverRunner> = [
        (mattmc, DiscoverRunner::Mattmc),
        (steam, DiscoverRunner::Steam),
        (emulators, DiscoverRunner::Emulators),
    ]
    .into_iter()
    .filter_map(|(selected, runner)| selected.then_some(runner))
    .collect();

    let report = if runners.is_empty() {
        core::discover_games()?
    } else {
        core::discover_with_runners(&runners)?
    };

    if let Some(mattmc_result) = report.mattmc {
        match mattmc_result {
            DiscoverResult::Added => println!("Discovered MattMC and added it."),
            DiscoverResult::AlreadyExists => println!("MattMC entry already exists."),
            DiscoverResult::NotFound => {
                println!(
                    "MattMC not found in {}",
                    core::mattmc_install_dir()?.display()
                )
            }
        }
    }

    if let Some(steam_report) = report.steam {
        if steam_report.found == 0 {
            println!("No Steam games discovered.");
        } else {
            println!(
                "Steam discovery complete: found {}, added {}, already existed {}.",
                steam_report.found, steam_report.added, steam_report.already_exists
            );
        }
    }

    if let Some(emulator_report) = report.emulators {
        if emulator_report.found == 0 {
            println!("No emulator ROMs discovered.");
        } else {
            println!(
                "Emulator discovery complete: found {}, added {}, updated {}, already existed {}.",
                emulator_report.found,
                emulator_report.added,
                emulator_report.updated,
                emulator_report.already_exists
            );
        }
    }

    Ok(())
}

/// Each system's ROM and save folders, as core resolves them.
fn print_emulator_folders() -> CoreResult<()> {
    println!("ROM and save folders:");
    for system in core::emulator_systems() {
        let saves = if system.supports_save_sync {
            core::emulator_save_dir(system.key)?.display().to_string()
        } else {
            "save sync not supported".to_string()
        };
        println!(
            "  {:<10} ROMs: {}  Saves: {}",
            system.key,
            core::emulator_rom_dir(system.key)?.display(),
            saves
        );
    }
    Ok(())
}

fn core_status(system: &str) -> CoreResult<()> {
    let core_installed = core::is_emulator_core_installed(system)?;
    let save_sync_supported = core::emulator_supports_save_sync(system);

    println!("System: {}", system);
    println!("ROM folder: {}", core::emulator_rom_dir(system)?.display());
    println!(
        "Core installed: {}",
        if core_installed { "yes" } else { "no" }
    );
    println!(
        "Save sync supported: {}",
        if save_sync_supported { "yes" } else { "no" }
    );

    Ok(())
}

fn sync_roms(platform: &str, up: bool) -> CoreResult<()> {
    if platform.eq_ignore_ascii_case("mattmc") {
        if up {
            core::sync_mattmc_up()?;
            println!("Ran sync-up script for MattMC.");
        } else {
            core::sync_mattmc_down()?;
            println!("Ran sync-down script for MattMC.");
        }
        return Ok(());
    }

    if up {
        let printer = ProgressPrinter::new();
        let report = core::sync_roms_up(platform, &printer.progress())?;
        printer.finish();
        println!(
            "Sync Up ({}) complete: copied {}, unchanged {}, deleted {}.",
            platform.to_uppercase(),
            report.copied,
            report.unchanged,
            report.deleted
        );
    } else {
        let printer = ProgressPrinter::new();
        let (sync_report, emulator_report) = core::sync_roms_down(platform, &printer.progress())?;
        printer.finish();
        println!(
            "Sync Down ({}) complete: copied {}, unchanged {}, deleted {}.",
            platform.to_uppercase(),
            sync_report.copied,
            sync_report.unchanged,
            sync_report.deleted
        );
        println!(
            "Emulator discover: found {}, added {}, updated {}, already existed {}.",
            emulator_report.found,
            emulator_report.added,
            emulator_report.updated,
            emulator_report.already_exists
        );
    }

    Ok(())
}

fn sync_saves(system: &str, up: bool) -> CoreResult<()> {
    let printer = ProgressPrinter::new();
    let progress = printer.progress();
    let (label, report) = if up {
        ("Sync Saves Up", core::sync_saves_up(system, &progress)?)
    } else {
        ("Sync Saves Down", core::sync_saves_down(system, &progress)?)
    };
    printer.finish();

    println!(
        "{} ({}) complete: copied {}, unchanged {}, deleted {}.",
        label,
        system.to_uppercase(),
        report.copied,
        report.unchanged,
        report.deleted
    );
    Ok(())
}

fn settings(action: SettingsCommand) -> CoreResult<()> {
    match action {
        SettingsCommand::Get => {
            let paths = core::load_emulation_remote_paths()?;
            println!("Remote ROM root: {}", paths.roms_root_dir);
            println!("Remote Saves root: {}", paths.saves_root_dir);
        }
        SettingsCommand::Set {
            roms_root,
            saves_root,
        } => {
            let existing = core::load_emulation_remote_paths()?;
            let roms_root_dir = roms_root.as_deref().map(str::trim);
            let saves_root_dir = saves_root.as_deref().map(str::trim);

            let saved = core::save_emulation_remote_paths(
                roms_root_dir.unwrap_or(&existing.roms_root_dir),
                saves_root_dir.unwrap_or(&existing.saves_root_dir),
            )?;
            println!("Saved remote ROM root: {}", saved.roms_root_dir);
            println!("Saved remote Saves root: {}", saved.saves_root_dir);
        }
    }

    Ok(())
}
