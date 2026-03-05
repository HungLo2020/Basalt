pub fn usage_add() -> String {
    "Usage: basalt add <name> <script_path>".to_string()
}

pub fn usage_remove() -> String {
    "Usage: basalt remove <name>".to_string()
}

pub fn usage_add_to_playlist() -> String {
    "Usage: basalt add-to-playlist <playlist> <name>".to_string()
}

pub fn usage_remove_from_playlist() -> String {
    "Usage: basalt remove-from-playlist <playlist> <name>".to_string()
}

pub fn usage_remove_all() -> String {
    "Usage: basalt remove-all".to_string()
}

pub fn usage_launch() -> String {
    "Usage: basalt launch <name>".to_string()
}

pub fn usage_discover() -> String {
    "Usage: basalt discover [--steam] [--mattmc]".to_string()
}

pub fn full_usage() -> String {
    [
        "Basalt CLI",
        "Usage:",
        "  basalt                        Launch GUI (default when no switch is provided)",
        "  basalt help                   Show this help message",
        "  basalt -h | --help           Show this help message",
        "  basalt add <name> <script_path>",
        "                               Add a game via bash script (.sh) or Steam appid/URL",
        "  basalt remove <name>",
        "                               Remove a saved game by name",
        "  basalt add-to-playlist <playlist> <name>",
        "                               Add a game to an existing playlist (e.g. Favorites)",
        "  basalt remove-from-playlist <playlist> <name>",
        "                               Remove a game from an existing playlist",
        "  basalt remove-all",
        "                               Remove all saved games",
        "  basalt list",
        "                               List all added games",
        "  basalt discover [--steam] [--mattmc]",
        "                               Discover games and add new entries (no switches = all discover runners)",
        "                               --steam discovers Steam games only",
        "                               --mattmc discovers MattMC only",
        "  basalt install-mattmc",
        "                               Download latest MattMC release into ~/Documents/MattMC",
        "  basalt launch <name>",
        "                               Launch a saved game script by name",
        "  basalt backup-mattmc",
        "                               Run backup.sh from MattMC launch script directory",
        "  basalt sync-mattmc",
        "                               Run SyncGameData.sh from MattMC launch script directory",
    ]
    .join("\n")
}