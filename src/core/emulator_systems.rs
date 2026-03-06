#[derive(Clone, Copy)]
pub struct EmulatorSystemSpec {
    pub system_key: &'static str,
    pub core_file: &'static str,
    pub archive_url: &'static str,
    pub rom_extensions: &'static [&'static str],
    pub supports_save_sync: bool,
    pub install_tile_key: &'static str,
    pub install_title: &'static str,
    pub install_description: &'static str,
    pub artwork_catalog_path: &'static str,
    pub artwork_aliases: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub struct EmulatorInstallTileSpec {
    pub key: &'static str,
    pub system_key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

const EMULATOR_SYSTEMS: [EmulatorSystemSpec; 6] = [
    EmulatorSystemSpec {
        system_key: "nes",
        core_file: "nestopia_libretro.so",
        archive_url:
            "https://buildbot.libretro.com/nightly/linux/x86_64/latest/nestopia_libretro.so.zip",
        rom_extensions: &["nes", "fds", "unf", "unif"],
        supports_save_sync: true,
        install_tile_key: "core-nes",
        install_title: "NES Core",
        install_description: "RetroArch Nestopia core for NES ROMs.",
        artwork_catalog_path: "Nintendo - Nintendo Entertainment System",
        artwork_aliases: &[],
    },
    EmulatorSystemSpec {
        system_key: "gba",
        core_file: "mgba_libretro.so",
        archive_url:
            "https://buildbot.libretro.com/nightly/linux/x86_64/latest/mgba_libretro.so.zip",
        rom_extensions: &["gba"],
        supports_save_sync: true,
        install_tile_key: "core-gba",
        install_title: "GBA Core",
        install_description: "RetroArch mGBA core for GBA ROMs.",
        artwork_catalog_path: "Nintendo - Game Boy Advance",
        artwork_aliases: &[],
    },
    EmulatorSystemSpec {
        system_key: "snes",
        core_file: "snes9x_libretro.so",
        archive_url:
            "https://buildbot.libretro.com/nightly/linux/x86_64/latest/snes9x_libretro.so.zip",
        rom_extensions: &["sfc", "smc", "swc", "fig", "bs"],
        supports_save_sync: true,
        install_tile_key: "core-snes",
        install_title: "SNES Core",
        install_description: "RetroArch Snes9x core for SNES ROMs.",
        artwork_catalog_path: "Nintendo - Super Nintendo Entertainment System",
        artwork_aliases: &[],
    },
    EmulatorSystemSpec {
        system_key: "atari2600",
        core_file: "stella_libretro.so",
        archive_url:
            "https://buildbot.libretro.com/nightly/linux/x86_64/latest/stella_libretro.so.zip",
        rom_extensions: &["a26", "bin", "rom"],
        supports_save_sync: false,
        install_tile_key: "core-atari2600",
        install_title: "Atari 2600 Core",
        install_description: "RetroArch Stella core for Atari 2600 ROMs.",
        artwork_catalog_path: "Atari - 2600",
        artwork_aliases: &["a2600"],
    },
    EmulatorSystemSpec {
        system_key: "nds",
        core_file: "melonds_libretro.so",
        archive_url:
            "https://buildbot.libretro.com/nightly/linux/x86_64/latest/melonds_libretro.so.zip",
        rom_extensions: &["nds"],
        supports_save_sync: true,
        install_tile_key: "core-nds",
        install_title: "NDS Core",
        install_description: "RetroArch melonDS core for Nintendo DS ROMs.",
        artwork_catalog_path: "Nintendo - Nintendo DS",
        artwork_aliases: &[],
    },
    EmulatorSystemSpec {
        system_key: "3ds",
        core_file: "citra_libretro.so",
        archive_url:
            "https://buildbot.libretro.com/nightly/linux/x86_64/latest/citra_libretro.so.zip",
        rom_extensions: &["3ds", "cci", "cxi", "3dsx"],
        supports_save_sync: true,
        install_tile_key: "core-3ds",
        install_title: "3DS Core",
        install_description: "RetroArch Citra core for Nintendo 3DS ROMs.",
        artwork_catalog_path: "Nintendo - Nintendo 3DS",
        artwork_aliases: &[],
    },
];

const EXTRA_ARTWORK_MAPPINGS: [(&str, &str); 7] = [
    ("gb", "Nintendo - Game Boy"),
    ("gbc", "Nintendo - Game Boy Color"),
    ("n64", "Nintendo - Nintendo 64"),
    ("genesis", "Sega - Mega Drive - Genesis"),
    ("megadrive", "Sega - Mega Drive - Genesis"),
    ("md", "Sega - Mega Drive - Genesis"),
    ("psp", "Sony - PlayStation Portable"),
];

pub fn emulator_system_specs() -> &'static [EmulatorSystemSpec] {
    &EMULATOR_SYSTEMS
}

pub fn emulation_install_tiles() -> Vec<EmulatorInstallTileSpec> {
    let mut tiles = EMULATOR_SYSTEMS
        .iter()
        .map(|spec| EmulatorInstallTileSpec {
            key: spec.install_tile_key,
            system_key: spec.system_key,
            title: spec.install_title,
            description: spec.install_description,
        })
        .collect::<Vec<EmulatorInstallTileSpec>>();

    tiles.sort_by(|left, right| left.title.cmp(right.title));
    tiles
}

pub fn discoverable_system_keys() -> Vec<&'static str> {
    EMULATOR_SYSTEMS.iter().map(|spec| spec.system_key).collect()
}

pub fn emulator_system(system: &str) -> Option<&'static EmulatorSystemSpec> {
    let normalized = normalize_system_key(system)?;
    EMULATOR_SYSTEMS
        .iter()
        .find(|spec| spec.system_key == normalized)
}

pub fn emulator_artwork_catalog_path(system: &str) -> Option<&'static str> {
    let normalized = normalize_system_key(system)?;

    if let Some(spec) = EMULATOR_SYSTEMS.iter().find(|spec| {
        spec.system_key == normalized
            || spec
                .artwork_aliases
                .iter()
                .any(|alias| *alias == normalized)
    }) {
        return Some(spec.artwork_catalog_path);
    }

    EXTRA_ARTWORK_MAPPINGS
        .iter()
        .find_map(|(key, catalog)| (*key == normalized).then_some(*catalog))
}

fn normalize_system_key(system: &str) -> Option<String> {
    let normalized = system.trim().to_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}