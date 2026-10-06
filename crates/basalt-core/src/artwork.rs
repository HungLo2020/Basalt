//! Game artwork: which image a game should show, and where it lives on disk.
//!
//! Core resolves artwork to image *files* (bundled overrides, the download cache, or a fresh
//! download). Decoding, resizing, scheduling, and display are the front end's job.

use std::path::PathBuf;

use super::mattmc::MATTMC_GAME_NAME;
use super::runners::RunnerKind;
use super::{EmulationLaunchTarget, GameEntry, emulator_artwork_catalog_path};

mod cache;
mod fetch;
mod local_overrides;
mod matching_index;
mod validation;

pub use cache::clear_artwork_cache;
pub use local_overrides::{override_artwork_dirs, user_override_artwork_dir};

const EMULATOR_ARTWORK_USER_AGENT: &str = "Basalt-Emulator-Artwork";
const EMULATOR_ARTWORK_IMAGES_PATH: &str = "images";
const EMULATOR_ARTWORK_INDEX_PATH: &str = "index";
const EMULATOR_ARTWORK_INDEX_TTL_SECONDS: u64 = 60 * 60 * 24;
const EMULATOR_ARTWORK_KEY_VERSION: &str = "v2";
const EMULATOR_ARTWORK_MIN_WIDTH: u32 = 120;
const EMULATOR_ARTWORK_MIN_HEIGHT: u32 = 120;
const LOCAL_ARTWORK_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// Where a game's artwork comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArtworkKind {
    /// Built-in MattMC icon; the front end supplies the image itself.
    Mattmc,
    /// Steam library portrait art, downloaded from the Steam CDN.
    Steam,
    /// Box art from the libretro thumbnail server.
    Emulator,
}

/// A request for one game's artwork. `key` is stable per artwork and suitable as a cache or
/// texture key in front ends.
#[derive(Clone, Debug)]
pub struct ArtworkRequest {
    pub key: String,
    pub kind: ArtworkKind,
    pub display_name: String,
    target: String,
}

impl ArtworkRequest {
    /// The artwork request for a library entry, or `None` for entries without artwork
    /// (plain scripts).
    pub fn for_game(game: &GameEntry) -> Option<Self> {
        if game.is_mattmc() {
            return Some(Self::mattmc());
        }

        match game.runner_kind {
            RunnerKind::Steam => {
                let appid = extract_steam_appid(&game.launch_target)?;
                Some(Self {
                    key: format!("steam:{}", appid),
                    kind: ArtworkKind::Steam,
                    display_name: game.name.clone(),
                    target: appid,
                })
            }
            RunnerKind::Emulator => Some(Self {
                key: format!(
                    "emulator:{}:{}",
                    EMULATOR_ARTWORK_KEY_VERSION,
                    stable_hash_hex(&game.launch_target)
                ),
                kind: ArtworkKind::Emulator,
                display_name: game.name.clone(),
                target: game.launch_target.clone(),
            }),
            RunnerKind::Bash => None,
        }
    }

    /// The MattMC artwork shown on the Install screen, independent of any library entry.
    pub fn mattmc() -> Self {
        Self {
            key: "mattmc:default".to_string(),
            kind: ArtworkKind::Mattmc,
            display_name: MATTMC_GAME_NAME.to_string(),
            target: String::new(),
        }
    }

    /// A user-supplied image from `resources/gameartwork` that overrides any downloaded art.
    pub fn local_override_path(&self) -> Option<PathBuf> {
        local_overrides::find_local_game_artwork_path(self)
    }

    /// The already-downloaded image for this request, without touching the network.
    pub fn cached_path(&self) -> Option<PathBuf> {
        match self.kind {
            ArtworkKind::Steam => fetch::find_cached_steam_portrait_artwork_path(&self.target),
            ArtworkKind::Emulator => fetch::find_cached_emulator_artwork_path(&self.target),
            ArtworkKind::Mattmc => None,
        }
    }

    /// Returns a valid cached image, downloading one if needed. Blocks on network I/O, so call
    /// it from a worker thread. `None` means no artwork could be found.
    pub fn fetch(&self) -> Option<PathBuf> {
        match self.kind {
            ArtworkKind::Steam => fetch::fetch_steam_portrait_artwork(&self.target),
            ArtworkKind::Emulator => fetch::fetch_emulator_artwork(&self.target),
            ArtworkKind::Mattmc => None,
        }
    }
}

fn emulator_system_catalog_path(system: &str) -> Option<&'static str> {
    emulator_artwork_catalog_path(system)
}

fn parse_emulator_launch_target(launch_target: &str) -> Option<(String, PathBuf)> {
    let parsed_target = EmulationLaunchTarget::decode(launch_target).ok()?;
    Some((
        parsed_target.system_key().to_string(),
        parsed_target.rom_path().to_path_buf(),
    ))
}

fn normalize_matching_title(raw: &str) -> String {
    let stripped = strip_bracketed_segments(raw);

    let mut normalized = String::new();
    let mut previous_was_space = false;
    for character in stripped.chars() {
        let lower = character.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            normalized.push(lower);
            previous_was_space = false;
        } else if !previous_was_space {
            normalized.push(' ');
            previous_was_space = true;
        }
    }

    normalized
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn strip_bracketed_segments(raw: &str) -> String {
    let mut output = String::new();
    let mut round_depth = 0usize;
    let mut square_depth = 0usize;
    let mut curly_depth = 0usize;

    for character in raw.chars() {
        match character {
            '(' => round_depth += 1,
            ')' => {
                round_depth = round_depth.saturating_sub(1);
            }
            '[' => square_depth += 1,
            ']' => {
                square_depth = square_depth.saturating_sub(1);
            }
            '{' => curly_depth += 1,
            '}' => {
                curly_depth = curly_depth.saturating_sub(1);
            }
            _ => {
                if round_depth == 0 && square_depth == 0 && curly_depth == 0 {
                    output.push(character);
                }
            }
        }
    }

    output
}

fn stable_hash_hex(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }

    format!("{:016x}", hash)
}

fn encode_url_path_segment(raw: &str) -> String {
    let mut encoded = String::new();
    for byte in raw.bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if is_unreserved {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{:02X}", byte));
        }
    }
    encoded
}

fn extract_steam_appid(launch_target: &str) -> Option<String> {
    let trimmed = launch_target.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed
        .chars()
        .all(|char_value| char_value.is_ascii_digit())
    {
        return Some(trimmed.to_string());
    }

    for prefix in [
        "steam://rungameid/",
        "steam://run/",
        "steam:appid:",
        "steam-appid:",
    ] {
        if let Some(value) = trimmed.strip_prefix(prefix)
            && value.chars().all(|char_value| char_value.is_ascii_digit())
        {
            return Some(value.to_string());
        }
    }

    None
}
