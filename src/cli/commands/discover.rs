use crate::core;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt discover".to_string());
    }

    let report = core::discover_games()?;

    match report.mattmc {
        core::DiscoverResult::Added => {
            println!("Discovered MattMC and added it.");
        }
        core::DiscoverResult::AlreadyExists => {
            println!("MattMC entry already exists.");
        }
        core::DiscoverResult::NotFound => {
            println!("MattMC not found at ~/Documents/MattMC/run-mattmc.sh");
        }
    }

    if report.steam_found == 0 {
        println!("No Steam games discovered.");
    } else {
        println!(
            "Steam discovery complete: found {}, added {}, already existed {}.",
            report.steam_found, report.steam_added, report.steam_already_exists
        );
    }

    Ok(())
}