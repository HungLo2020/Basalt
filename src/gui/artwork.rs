use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::thread;

use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};

use eframe::egui::{Context, TextureHandle};

use crate::core::{self, EmulationLaunchTarget, GameEntry};

#[path = "artwork/cache.rs"]
mod cache;
#[path = "artwork/download_workers.rs"]
mod download_workers;
#[path = "artwork/local_overrides.rs"]
mod local_overrides;
#[path = "artwork/matching_index.rs"]
mod matching_index;
#[path = "artwork/texture_prep.rs"]
mod texture_prep;

const MATTMC_SVG_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/assets/icons/apps/MattMC.svg"
));
const MATTMC_SVG_TARGET_MAX_DIMENSION: u32 = 1024;
const MATTMC_BLUR_BACKGROUND_RGB: [u8; 3] = [56, 68, 88];
const PREPARED_ARTWORK_MAX_WIDTH: u32 = 360;
const PREPARED_ARTWORK_MAX_HEIGHT: u32 = 540;
const MAX_STEAM_DOWNLOAD_QUEUE: usize = 48;
const MAX_EMULATOR_DOWNLOAD_QUEUE: usize = 48;
const MAX_RESULT_QUEUE: usize = 96;
const MAX_TEXTURE_UPLOADS_PER_TICK: usize = 6;
const EMULATOR_ARTWORK_USER_AGENT: &str = "Basalt-Emulator-Artwork";
const EMULATOR_ARTWORK_IMAGES_PATH: &str = "images";
const EMULATOR_ARTWORK_INDEX_PATH: &str = "index";
const EMULATOR_ARTWORK_INDEX_TTL_SECONDS: u64 = 60 * 60 * 24;
const EMULATOR_ARTWORK_KEY_VERSION: &str = "v2";
const EMULATOR_ARTWORK_MIN_WIDTH: u32 = 120;
const EMULATOR_ARTWORK_MIN_HEIGHT: u32 = 120;
const LOCAL_GAME_ARTWORK_DIR: &str = "resources/gameartwork";
const LOCAL_ARTWORK_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

#[derive(Clone)]
pub(super) struct ArtworkTextures {
    pub(super) foreground: TextureHandle,
    pub(super) _background_blur: TextureHandle,
}

pub(super) struct ArtworkStore {
    textures: HashMap<String, ArtworkTextures>,
    missing: HashSet<String>,
    requested_by_runner: HashMap<String, ArtworkRunnerKind>,
    metadata_prefetch_queue: Vec<ArtworkRequest>,
    max_pending_steam_downloads: usize,
    max_pending_emulator_downloads: usize,
    steam_download_tx: Sender<ArtworkDownloadJob>,
    emulator_download_tx: Sender<ArtworkDownloadJob>,
    result_rx: Receiver<ArtworkDownloadResult>,
}

impl ArtworkStore {
    pub(super) fn new() -> Self {
        let host_thread_count = detect_host_thread_count();
        let (steam_worker_count, emulator_worker_count) =
            compute_artwork_worker_counts(host_thread_count);
        let max_pending_steam_downloads = (steam_worker_count * 3).clamp(4, 24);
        let max_pending_emulator_downloads = (emulator_worker_count * 3).clamp(6, 64);

        let (steam_download_tx, steam_download_rx) =
            bounded::<ArtworkDownloadJob>(MAX_STEAM_DOWNLOAD_QUEUE);
        let (emulator_download_tx, emulator_download_rx) =
            bounded::<ArtworkDownloadJob>(MAX_EMULATOR_DOWNLOAD_QUEUE);
        let (result_tx, result_rx) = bounded::<ArtworkDownloadResult>(MAX_RESULT_QUEUE);

        for _ in 0..steam_worker_count {
            let worker_download_rx = steam_download_rx.clone();
            let worker_result_tx = result_tx.clone();

            thread::spawn(move || {
                while let Ok(job) = worker_download_rx.recv() {
                    let result = download_workers::process_download_job(job);
                    if worker_result_tx.send(result).is_err() {
                        break;
                    }
                }
            });
        }

        for _ in 0..emulator_worker_count {
            let worker_download_rx = emulator_download_rx.clone();
            let worker_result_tx = result_tx.clone();

            thread::spawn(move || {
                while let Ok(job) = worker_download_rx.recv() {
                    let result = download_workers::process_download_job(job);
                    if worker_result_tx.send(result).is_err() {
                        break;
                    }
                }
            });
        }

        Self {
            textures: HashMap::new(),
            missing: HashSet::new(),
            requested_by_runner: HashMap::new(),
            metadata_prefetch_queue: Vec::new(),
            max_pending_steam_downloads,
            max_pending_emulator_downloads,
            steam_download_tx,
            emulator_download_tx,
            result_rx,
        }
    }

    pub(super) fn poll_download_results(&mut self, ctx: &Context) {
        let mut has_updates = false;
        let mut processed = 0usize;

        while processed < MAX_TEXTURE_UPLOADS_PER_TICK {
            let result = match self.result_rx.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            };

            let key_for_state = match &result {
                ArtworkDownloadResult::Ready { key, .. } => key.clone(),
                ArtworkDownloadResult::Missing { key } => key.clone(),
            };
            let _ = self.requested_by_runner.remove(&key_for_state);

            match result {
                ArtworkDownloadResult::Ready { key, payload } => {
                    if let Some(textures) = texture_prep::build_artwork_textures_from_payload(ctx, &key, payload) {
                        self.textures.insert(key.clone(), textures);
                        self.missing.remove(&key);
                        has_updates = true;
                    } else {
                        self.missing.insert(key);
                    }
                }
                ArtworkDownloadResult::Missing { key } => {
                    self.missing.insert(key);
                }
            }

            processed += 1;
        }

        if has_updates {
            ctx.request_repaint();
        }

        self.pump_metadata_prefetch_queue();
    }

    pub(super) fn prepare_for_games(&mut self, games: &[GameEntry]) {
        let mut visible_keys = HashSet::new();
        let mut metadata_prefetch_requests = Vec::new();

        for game in games {
            let runner = ArtworkRunnerKind::from_game(game);
            let Some(request) = runner.build_request(game) else {
                continue;
            };

            visible_keys.insert(request.key.clone());

            if request.runner == ArtworkRunnerKind::Steam
                || request.runner == ArtworkRunnerKind::Emulator
            {
                metadata_prefetch_requests.push(request);
            }
        }

        self.textures.retain(|key, _| visible_keys.contains(key));
        self.requested_by_runner
            .retain(|key, _| visible_keys.contains(key));
        self.missing.retain(|key| visible_keys.contains(key));

        self.metadata_prefetch_queue = metadata_prefetch_requests;
        self.pump_metadata_prefetch_queue();
    }

    pub(super) fn refresh_metadata_for_games(&mut self, games: &[GameEntry]) {
        let _ = core::clear_artwork_cache();

        self.textures.clear();
        self.missing.clear();
        self.requested_by_runner.clear();
        self.metadata_prefetch_queue.clear();

        self.prepare_for_games(games);
    }

    pub(super) fn artwork_for_game(
        &mut self,
        ctx: &Context,
        game: &GameEntry,
    ) -> Option<ArtworkTextures> {
        let runner = ArtworkRunnerKind::from_game(game);
        let request = runner.build_request(game)?;

        self.artwork_for_request(ctx, request)
    }

    pub(super) fn mattmc_artwork(&mut self, ctx: &Context) -> Option<ArtworkTextures> {
        self.artwork_for_request(
            ctx,
            ArtworkRequest {
                key: "mattmc:default".to_string(),
                target: String::new(),
                display_name: "MattMC".to_string(),
                runner: ArtworkRunnerKind::Mattmc,
            },
        )
    }

    fn artwork_for_request(
        &mut self,
        ctx: &Context,
        request: ArtworkRequest,
    ) -> Option<ArtworkTextures> {

        if let Some(existing_texture) = self.textures.get(&request.key) {
            return Some(existing_texture.clone());
        }

        if let Some(local_artwork_path) = local_overrides::find_local_game_artwork_path(&request) {
            if let Some(payload) = texture_prep::prepare_artwork_payload_from_path(&local_artwork_path, None) {
                if let Some(textures) = texture_prep::build_artwork_textures_from_payload(ctx, &request.key, payload) {
                    self.textures
                        .insert(request.key.clone(), textures.clone());
                    self.missing.remove(&request.key);
                    return Some(textures);
                }
            }
        }

        if let Some(builtin_textures) = request.runner.load_builtin_artwork(ctx, &request.key) {
            self.textures
                .insert(request.key.clone(), builtin_textures.clone());
            return Some(builtin_textures);
        }

        let _ = self.request_download(request);
        None
    }

    fn request_download(&mut self, request: ArtworkRequest) -> bool {
        if local_overrides::find_local_game_artwork_path(&request).is_some() {
            return true;
        }

        if self.missing.contains(&request.key) || self.requested_by_runner.contains_key(&request.key) {
            return true;
        }

        let has_cached_artwork = request.runner.has_cached_artwork(&request.target);
        let pending_count_for_runner = self
            .requested_by_runner
            .values()
            .filter(|runner| **runner == request.runner)
            .count();
        let max_pending_for_runner = match request.runner {
            ArtworkRunnerKind::Steam => self.max_pending_steam_downloads,
            ArtworkRunnerKind::Emulator => self.max_pending_emulator_downloads,
            ArtworkRunnerKind::Mattmc | ArtworkRunnerKind::Noop => 0,
        };

        if !has_cached_artwork && pending_count_for_runner >= max_pending_for_runner {
            return false;
        }

        let Some(job) = request
            .runner
            .to_download_job(request.key.clone(), request.target.clone())
        else {
            return true;
        };

        let enqueue_result = match request.runner {
            ArtworkRunnerKind::Steam => self.steam_download_tx.try_send(job),
            ArtworkRunnerKind::Emulator => self.emulator_download_tx.try_send(job),
            ArtworkRunnerKind::Mattmc | ArtworkRunnerKind::Noop => return true,
        };

        match enqueue_result {
            Ok(_) => {
                self.requested_by_runner.insert(request.key, request.runner);
                true
            }
            Err(TrySendError::Full(_)) => false,
            Err(TrySendError::Disconnected(_)) => {
                self.missing.insert(request.key);
                true
            }
        }
    }

    fn pump_metadata_prefetch_queue(&mut self) {
        if self.metadata_prefetch_queue.is_empty() {
            return;
        }

        let queued_requests = std::mem::take(&mut self.metadata_prefetch_queue);
        let mut remaining_queue = Vec::new();
        for request in queued_requests {
            if self.textures.contains_key(&request.key)
                || self.missing.contains(&request.key)
                || self.requested_by_runner.contains_key(&request.key)
            {
                continue;
            }

            if !self.request_download(request.clone()) {
                remaining_queue.push(request);
            }
        }

        self.metadata_prefetch_queue = remaining_queue;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ArtworkRunnerKind {
    Mattmc,
    Steam,
    Emulator,
    Noop,
}

impl ArtworkRunnerKind {
    fn from_game(game: &GameEntry) -> Self {
        if game.name.eq_ignore_ascii_case("MattMC") {
            Self::Mattmc
        } else if game.runner_kind.as_str() == "steam" {
            Self::Steam
        } else if game.runner_kind.as_str() == "emulator" {
            Self::Emulator
        } else {
            Self::Noop
        }
    }

    fn build_request(self, game: &GameEntry) -> Option<ArtworkRequest> {
        match self {
            Self::Mattmc => Some(ArtworkRequest {
                key: "mattmc:default".to_string(),
                target: String::new(),
                display_name: game.name.clone(),
                runner: self,
            }),
            Self::Steam => {
                let appid = extract_steam_appid(&game.launch_target)?;
                Some(ArtworkRequest {
                    key: format!("steam:{}", appid),
                    target: appid,
                    display_name: game.name.clone(),
                    runner: self,
                })
            }
            Self::Emulator => {
                Some(ArtworkRequest {
                    key: format!(
                        "emulator:{}:{}",
                        EMULATOR_ARTWORK_KEY_VERSION,
                        stable_hash_hex(&game.launch_target)
                    ),
                    target: game.launch_target.clone(),
                    display_name: game.name.clone(),
                    runner: self,
                })
            }
            Self::Noop => None,
        }
    }

    fn load_builtin_artwork(self, ctx: &Context, key: &str) -> Option<ArtworkTextures> {
        match self {
            Self::Mattmc => texture_prep::load_artwork_textures_from_svg_bytes(ctx, key, MATTMC_SVG_BYTES),
            Self::Steam | Self::Emulator | Self::Noop => None,
        }
    }

    fn has_cached_artwork(self, target: &str) -> bool {
        match self {
            Self::Steam => download_workers::find_cached_steam_portrait_artwork_path(target).is_some(),
            Self::Emulator => download_workers::find_cached_emulator_artwork_path(target).is_some(),
            Self::Mattmc | Self::Noop => false,
        }
    }

    fn to_download_job(self, key: String, target: String) -> Option<ArtworkDownloadJob> {
        match self {
            Self::Steam | Self::Emulator => Some(ArtworkDownloadJob {
                key,
                target,
                runner: self,
            }),
            Self::Mattmc | Self::Noop => None,
        }
    }
}

#[derive(Clone)]
struct ArtworkRequest {
    key: String,
    target: String,
    display_name: String,
    runner: ArtworkRunnerKind,
}

enum ArtworkDownloadResult {
    Ready { key: String, payload: PreparedArtwork },
    Missing { key: String },
}

struct ArtworkDownloadJob {
    key: String,
    target: String,
    runner: ArtworkRunnerKind,
}

struct PreparedArtwork {
    width: usize,
    height: usize,
    foreground_rgba: Vec<u8>,
    background_rgba: Vec<u8>,
}

fn emulator_system_catalog_path(system: &str) -> Option<&'static str> {
    core::emulator_artwork_catalog_path(system)
}

fn detect_host_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4)
}

fn compute_artwork_worker_counts(host_threads: usize) -> (usize, usize) {
    let clamped_threads = host_threads.max(2);
    let emulator_workers = (clamped_threads / 2).clamp(2, 12);

    let steam_workers = if clamped_threads >= 16 {
        4
    } else if clamped_threads >= 8 {
        3
    } else {
        2
    };

    (steam_workers, emulator_workers)
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

    normalized.split_whitespace().collect::<Vec<&str>>().join(" ")
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
        let is_unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
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

    if trimmed.chars().all(|char_value| char_value.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }

    for prefix in [
        "steam://rungameid/",
        "steam://run/",
        "steam:appid:",
        "steam-appid:",
    ] {
        if let Some(value) = trimmed.strip_prefix(prefix) {
            if value.chars().all(|char_value| char_value.is_ascii_digit()) {
                return Some(value.to_string());
            }
        }
    }

    None
}
