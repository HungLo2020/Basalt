//! Game artwork: which image a game should show, and where it lives on disk.
//!
//! Core resolves artwork to image *files* (user overrides, files a game ships, the download
//! cache, or a fresh download). Decoding, resizing, scheduling, and display are the front end's
//! job. Catalog games follow STANDARDS.md ("Artwork"): a user override wins, then the game's
//! shipped `app/basalt/cover.*`, then its catalog `artwork.cover_url`, then its shipped icon.

use std::path::{Path, PathBuf};

use super::runners::RunnerKind;
use crate::{GameDefinition, GameEntry};

mod cache;
mod catalog_art;
mod fetch;
mod local_overrides;
mod validation;

pub use cache::clear_artwork_cache;

const LOCAL_ARTWORK_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];
/// Games whose icon is bundled with Basalt, used when the game provides no artwork itself.
const BUNDLED_ICONS: [&str; 1] = ["MattMC"];

/// Where a game's artwork comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArtworkKind {
    /// A catalog game: its shipped files or its catalog cover URL.
    Catalog,
    /// Steam library portrait art, downloaded from the Steam CDN.
    Steam,
}

/// A request for one game's artwork. `key` is stable per artwork and suitable as a cache or
/// texture key in front ends.
#[derive(Clone, Debug)]
pub struct ArtworkRequest {
    pub key: String,
    pub kind: ArtworkKind,
    pub display_name: String,
    /// Steam app ID, or catalog game ID.
    target: String,
    install_root: Option<PathBuf>,
    cover_url: Option<String>,
}

impl ArtworkRequest {
    /// The artwork request for a library entry, or `None` for entries without artwork
    /// (plain scripts that are not catalog games).
    pub fn for_game(game: &GameEntry) -> Option<Self> {
        match game.runner_kind {
            RunnerKind::Steam => {
                let appid = extract_steam_appid(&game.launch_target)?;
                Some(Self {
                    key: format!("steam:{}", appid),
                    kind: ArtworkKind::Steam,
                    display_name: game.name.clone(),
                    target: appid,
                    install_root: None,
                    cover_url: None,
                })
            }
            RunnerKind::Bash | RunnerKind::Catalog => {
                let definition = crate::catalog_games()
                    .ok()?
                    .into_iter()
                    .find(|definition| definition.name == game.name)?;
                Some(Self::for_catalog(&definition, Some(installation_root(game))))
            }
        }
    }

    /// The artwork for a catalog game, installed at `install_root` or not installed.
    pub fn for_catalog(definition: &GameDefinition, install_root: Option<PathBuf>) -> Self {
        Self {
            key: format!("catalog:{}", definition.id),
            kind: ArtworkKind::Catalog,
            display_name: definition.name.clone(),
            target: definition.id.clone(),
            install_root,
            cover_url: definition
                .artwork
                .as_ref()
                .and_then(|artwork| artwork.cover_url.clone()),
        }
    }

    /// One request per catalog game, using the installation registered in `games` when there
    /// is one. Library tiles and Install page tiles share these keys.
    pub fn for_catalog_games(games: &[GameEntry]) -> Vec<Self> {
        crate::catalog_games()
            .unwrap_or_default()
            .iter()
            .map(|definition| {
                let root = games
                    .iter()
                    .find(|game| game.name == definition.name && game.runner_kind != RunnerKind::Steam)
                    .map(installation_root);
                Self::for_catalog(definition, root)
            })
            .collect()
    }

    /// A user-supplied image (see `local_overrides`) that overrides any other art.
    pub fn local_override_path(&self) -> Option<PathBuf> {
        local_overrides::find_local_game_artwork_path(self)
    }

    /// Artwork available without the network: a downloaded Steam portrait, or a catalog game's
    /// shipped cover or previously downloaded cover.
    pub fn cached_path(&self) -> Option<PathBuf> {
        match self.kind {
            ArtworkKind::Steam => fetch::find_cached_steam_portrait_artwork_path(&self.target),
            ArtworkKind::Catalog => catalog_art::shipped_cover(self.install_root.as_deref())
                .or_else(|| catalog_art::cached_cover(&self.target, self.cover_url.as_deref())),
        }
    }

    /// True if finding artwork may need the network, so it belongs on a download worker.
    pub fn needs_download(&self) -> bool {
        match self.kind {
            ArtworkKind::Steam => true,
            ArtworkKind::Catalog => self.cover_url.is_some(),
        }
    }

    /// Returns a valid image, downloading one if needed, after `cached_path` found nothing.
    /// Blocks on network I/O, so call it from a worker thread. For catalog games this falls back
    /// to the shipped icon. `None` means no artwork could be found.
    pub fn fetch(&self) -> Option<PathBuf> {
        match self.kind {
            ArtworkKind::Steam => fetch::fetch_steam_portrait_artwork(&self.target),
            ArtworkKind::Catalog => self
                .cover_url
                .as_deref()
                .and_then(|url| catalog_art::download_cover(&self.target, url))
                .or_else(|| catalog_art::shipped_icon(self.install_root.as_deref())),
        }
    }

    /// True if Basalt bundles an icon for this game, to show when nothing else is found.
    pub fn has_bundled_icon(&self) -> bool {
        self.kind == ArtworkKind::Catalog && BUNDLED_ICONS.contains(&self.target.as_str())
    }

    /// The catalog game ID or Steam app ID.
    pub fn target(&self) -> &str {
        &self.target
    }
}

/// Where a registered catalog game is installed: the folder holding its manifest, or for an
/// older script registration (such as MattMC's `run-mattmc.sh`), the folder holding the script.
fn installation_root(game: &GameEntry) -> PathBuf {
    let target = Path::new(&game.launch_target);
    target.parent().unwrap_or(target).to_path_buf()
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

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use crate::game_layout::tests::temp_root;

    fn png(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::new(30, 40).save(path).unwrap();
    }

    fn entry(name: &str, runner_kind: RunnerKind, target: &Path) -> GameEntry {
        GameEntry {
            name: name.into(),
            runner_kind,
            launch_target: target.to_string_lossy().into_owned(),
        }
    }

    #[test]
    fn catalog_games_get_requests_rooted_at_their_installation() {
        let root = temp_root("art-request");
        let installed = entry("MicroMages", RunnerKind::Catalog, &root.join(".basalt-game.json"));
        let request = ArtworkRequest::for_game(&installed).unwrap();
        assert_eq!(request.key, "catalog:MicroMages");
        assert_eq!(request.kind, ArtworkKind::Catalog);
        assert!(!request.has_bundled_icon());
        assert!(!request.needs_download(), "no cover_url in the bundled entry");
        // Nothing shipped yet: no artwork at all.
        assert!(request.cached_path().is_none() && request.fetch().is_none());
        png(&root.join("app/basalt/icon.png"));
        assert!(request.cached_path().is_none(), "an icon is only a fallback");
        assert_eq!(request.fetch(), Some(root.join("app/basalt/icon.png")));
        png(&root.join("app/basalt/cover.png"));
        assert_eq!(request.cached_path(), Some(root.join("app/basalt/cover.png")));

        // An old run-mattmc.sh registration still gets MattMC's bundled icon.
        let legacy = entry("MattMC", RunnerKind::Bash, &root.join("run-mattmc.sh"));
        let mattmc = ArtworkRequest::for_game(&legacy).unwrap();
        assert_eq!(mattmc.key, "catalog:MattMC");
        assert!(mattmc.has_bundled_icon());
        // Plain scripts that are not catalog games have no artwork.
        assert!(ArtworkRequest::for_game(&entry("Celeste", RunnerKind::Bash, &root.join("c.sh"))).is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn every_catalog_game_gets_one_request_installed_or_not() {
        let root = temp_root("art-all");
        let catalog = crate::catalog_games().unwrap();
        let games = [entry(&catalog[0].name, RunnerKind::Catalog, &root.join(".basalt-game.json"))];
        let requests = ArtworkRequest::for_catalog_games(&games);
        let keys: Vec<&str> = requests.iter().map(|r| r.key.as_str()).collect();
        let expected: Vec<String> = catalog
            .iter()
            .map(|game| format!("catalog:{}", game.id))
            .collect();
        assert_eq!(keys, expected);
        for request in &requests {
            let expected_root = (request.display_name == games[0].name).then_some(root.as_path());
            assert_eq!(request.install_root.as_deref(), expected_root);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cover_url_makes_a_request_download_but_a_shipped_cover_wins() {
        let root = temp_root("art-url");
        let mut definition = crate::find_game("MicroMages").unwrap();
        definition.artwork = Some(crate::ArtworkDefinition {
            cover_url: Some("https://example.invalid/cover.png".into()),
        });
        let request = ArtworkRequest::for_catalog(&definition, Some(root.clone()));
        assert!(request.needs_download());
        png(&root.join("app/basalt/cover.jpg"));
        assert_eq!(request.cached_path(), Some(root.join("app/basalt/cover.jpg")));
        let _ = std::fs::remove_dir_all(root);
    }
}
