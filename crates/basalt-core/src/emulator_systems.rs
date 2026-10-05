#[derive(Clone, Copy)]
pub struct EmulatorSystemSpec {
    pub system_key: &'static str,
    pub core_file: &'static str,
    pub archive_url: &'static str,
    pub rom_extensions: &'static [&'static str],
    pub supports_save_sync: bool,
    pub short_name: &'static str,
    pub full_name: &'static str,
    pub core_name: &'static str,
    pub artwork_catalog_path: &'static str,
    pub artwork_aliases: &'static [&'static str],
}

/// Public, presentation-neutral description of a supported emulator system. Front ends decide
/// how to label it.
#[derive(Clone, Copy, Debug)]
pub struct EmulatorSystem {
    /// Stable key used in ROM/save folder names and launch targets (e.g. "nes").
    pub key: &'static str,
    /// Short label, e.g. "NES" or "Atari 2600".
    pub short_name: &'static str,
    /// Long label, e.g. "Nintendo DS".
    pub full_name: &'static str,
    /// The RetroArch core used, e.g. "Nestopia".
    pub core_name: &'static str,
    pub supports_save_sync: bool,
}

const EMULATOR_SYSTEMS: [EmulatorSystemSpec; 6] = [
    EmulatorSystemSpec {
        system_key: "nes",
        core_file: "nestopia_libretro.so",
        archive_url:
            "https://buildbot.libretro.com/nightly/linux/x86_64/latest/nestopia_libretro.so.zip",
        rom_extensions: &["nes", "fds", "unf", "unif"],
        supports_save_sync: true,
        short_name: "NES",
        full_name: "NES",
        core_name: "Nestopia",
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
        short_name: "GBA",
        full_name: "GBA",
        core_name: "mGBA",
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
        short_name: "SNES",
        full_name: "SNES",
        core_name: "Snes9x",
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
        short_name: "Atari 2600",
        full_name: "Atari 2600",
        core_name: "Stella",
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
        short_name: "NDS",
        full_name: "Nintendo DS",
        core_name: "melonDS",
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
        short_name: "3DS",
        full_name: "Nintendo 3DS",
        core_name: "Citra",
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

pub fn emulator_systems() -> Vec<EmulatorSystem> {
    EMULATOR_SYSTEMS
        .iter()
        .map(|spec| EmulatorSystem {
            key: spec.system_key,
            short_name: spec.short_name,
            full_name: spec.full_name,
            core_name: spec.core_name,
            supports_save_sync: spec.supports_save_sync,
        })
        .collect()
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