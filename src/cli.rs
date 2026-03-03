use crate::core;

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help") | Some("-h") | Some("--help") => {
            println!("{}", full_usage());
            Ok(())
        }
        Some("add") => {
            if args.len() != 3 {
                return Err(usage_add());
            }

            core::add_game(args[1].trim(), args[2].trim())?;
            println!("Game added successfully.");
            Ok(())
        }
        Some("remove") => {
            if args.len() != 2 {
                return Err(usage_remove());
            }

            core::remove_game(args[1].trim())?;
            println!("Game removed successfully.");
            Ok(())
        }
        Some("list") => {
            let entries = core::list_games()?;

            if entries.is_empty() {
                println!("No games added yet.");
                return Ok(());
            }

            for entry in entries {
                println!("{}\t{}", entry.name, entry.script_path);
            }

            Ok(())
        }
        Some("discover") => {
            if args.len() != 1 {
                return Err("Usage: basalt discover".to_string());
            }

            match core::discover_mattmc()? {
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

            Ok(())
        }
        Some("launch") => {
            if args.len() != 2 {
                return Err(usage_launch());
            }

            core::launch_game(args[1].trim())
        }
        Some(other) => Err(format!("Unknown command: {}\n\n{}", other, full_usage())),
        None => Err(full_usage()),
    }
}

fn usage_add() -> String {
    "Usage: basalt add <name> <script_path>".to_string()
}

fn usage_remove() -> String {
    "Usage: basalt remove <name>".to_string()
}

fn usage_launch() -> String {
    "Usage: basalt launch <name>".to_string()
}

fn full_usage() -> String {
    [
        "Basalt CLI",
        "Usage:",
        "  basalt                        Launch GUI (default when no switch is provided)",
        "  basalt help                   Show this help message",
        "  basalt -h | --help           Show this help message",
        "  basalt add <name> <script_path>",
        "                               Add a game backed by a bash script (.sh)",
        "  basalt remove <name>",
        "                               Remove a saved game by name",
        "  basalt list",
        "                               List all added games",
        "  basalt discover",
        "                               Discover ~/Documents/MattMC/run-mattmc.sh and add MattMC",
        "  basalt launch <name>",
        "                               Launch a saved game script by name",
    ]
    .join("\n")
}
