use std::env;

use crate::core;

pub fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("add") => {
            if args.len() != 4 {
                return Err(usage_add());
            }

            core::add_game(args[2].trim(), args[3].trim())?;
            println!("Game added successfully.");
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
        Some("launch") => {
            if args.len() != 3 {
                return Err(usage_launch());
            }

            core::launch_game(args[2].trim())
        }
        _ => Err(full_usage()),
    }
}

fn usage_add() -> String {
    "Usage: basalt add <name> <script_path>".to_string()
}

fn usage_launch() -> String {
    "Usage: basalt launch <name>".to_string()
}

fn full_usage() -> String {
    [
        "Basalt CLI",
        "Usage:",
        "  basalt add <name> <script_path>",
        "  basalt list",
        "  basalt launch <name>",
    ]
    .join("\n")
}
