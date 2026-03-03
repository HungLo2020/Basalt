use crate::core;
use crate::core::DiscoverRunner;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    let mut runners = Vec::new();

    for argument in args.iter().skip(1) {
        match argument.as_str() {
            "--mattmc" => runners.push(DiscoverRunner::Mattmc),
            "--steam" => runners.push(DiscoverRunner::Steam),
            _ => return Err(usage::usage_discover()),
        }
    }

    let report = if runners.is_empty() {
        core::discover_games()?
    } else {
        core::discover_with_runners(&runners)?
    };

    if let Some(mattmc_result) = report.mattmc {
        match mattmc_result {
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

    Ok(())
}