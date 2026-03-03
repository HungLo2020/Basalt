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
        "  basalt list",
        "                               List all added games",
        "  basalt launch <name>",
        "                               Launch a saved game script by name",
    ]
    .join("\n")
}
