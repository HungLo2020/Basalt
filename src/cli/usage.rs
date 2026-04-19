pub fn usage_add() -> String {
    "Usage: basalt add <name> <target>".to_string()
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
    "Usage: basalt discover [--steam] [--mattmc] [--emulators]".to_string()
}

pub fn usage_install_core() -> String {
    "Usage: basalt install-core <system>".to_string()
}

pub fn usage_core_status() -> String {
    "Usage: basalt core-status <system>".to_string()
}

pub fn usage_sync_up() -> String {
    "Usage: basalt sync-up <platform>".to_string()
}

pub fn usage_sync_down() -> String {
    "Usage: basalt sync-down <platform>".to_string()
}

pub fn usage_sync_saves_up() -> String {
    "Usage: basalt sync-saves-up <system>".to_string()
}

pub fn usage_sync_saves_down() -> String {
    "Usage: basalt sync-saves-down <system>".to_string()
}

pub fn usage_settings() -> String {
    [
        "Usage:",
        "  basalt settings get",
        "  basalt settings set [--roms-root <path>] [--saves-root <path>]",
    ]
    .join("\n")
}

pub fn usage_settings_get() -> String {
    "Usage: basalt settings get".to_string()
}

pub fn usage_settings_set() -> String {
    "Usage: basalt settings set [--roms-root <path>] [--saves-root <path>]".to_string()
}

pub fn usage_refresh_metadata() -> String {
    "Usage: basalt refresh-metadata".to_string()
}

pub fn usage_sync_mattmc() -> String {
    "Usage: basalt sync-mattmc".to_string()
}

pub fn full_usage() -> String {
    [
        "Basalt CLI",
        "Usage:",
        "  basalt                        Launch GUI (default when no switch is provided)",
        "  basalt help                   Show this help message",
        "  basalt -h | --help           Show this help message",
        "  basalt add <name> <target>",
        "                               Add a game via script file, Steam appid/URL, or emulator launch target",
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
        "  basalt discover [--steam] [--mattmc] [--emulators]",
        "                               Discover games and add new entries (no switches = all discover runners)",
        "                               --steam discovers Steam games only",
        "                               --mattmc discovers MattMC only",
        "                               --emulators discovers emulator ROM entries only",
        "  basalt install-core <system>",
        "                               Install a RetroArch core for one system key (e.g. nes, gba, snes, atari2600, nds, 3ds)",
        "  basalt core-status <system>",
        "                               Show whether a system core is installed and if save sync is supported",
        "  basalt install-emulators",
        "                               Install RetroArch runtime plus built-in emulator cores",
        "  basalt install-mattmc",
        "                               Download latest MattMC release into ~/Games/MattMC",
        "  basalt launch <name>",
        "                               Launch a saved game by name",
        "  basalt backup-mattmc",
        "                               Run backup.sh from MattMC launch script directory",
        "  basalt sync-mattmc",
        "                               Run SyncGameData.sh from MattMC launch script directory",
        "  basalt sync-up <platform>",
        "                               Sync up for a platform: use mattmc or emulator system key (e.g. nes, gba)",
        "  basalt sync-down <platform>",
        "                               Sync down for a platform: mattmc or emulator system key (emulators also run discover)",
        "  basalt sync-saves-up <system>",
        "                               Sync local save files to remote for a system key",
        "  basalt sync-saves-down <system>",
        "                               Sync remote save files to local for a system key",
        "  basalt settings get",
        "                               Show remote ROM/Saves root directories used by sync commands",
        "  basalt settings set [--roms-root <path>] [--saves-root <path>]",
        "                               Update remote ROM/Saves root directories",
        "  basalt refresh-metadata",
        "                               Clear cached artwork metadata and images",
    ]
    .join("\n")
}