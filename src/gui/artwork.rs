use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};

use eframe::egui::{self, Context, TextureHandle};

use crate::core::GameEntry;

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
                    let result = process_download_job(job);
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
                    let result = process_download_job(job);
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
                    if let Some(textures) = build_artwork_textures_from_payload(ctx, &key, payload) {
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
        clear_artwork_cache_files();

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

        if let Some(local_artwork_path) = find_local_game_artwork_path(&request) {
            if let Some(payload) = prepare_artwork_payload_from_path(&local_artwork_path, None) {
                if let Some(textures) = build_artwork_textures_from_payload(ctx, &request.key, payload) {
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
        if find_local_game_artwork_path(&request).is_some() {
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

fn clear_artwork_cache_files() {
    if let Some(steam_dir) = steam_artwork_cache_dir() {
        let _ = std::fs::remove_dir_all(&steam_dir);
    }

    if let Some(emulator_root_dir) = emulator_artwork_cache_root_dir() {
        let _ = std::fs::remove_dir_all(&emulator_root_dir);
    }

    if let Some(steam_dir) = steam_artwork_cache_dir() {
        let _ = std::fs::create_dir_all(steam_dir);
    }

    if let Some(emulator_images_dir) = emulator_artwork_images_cache_dir() {
        let _ = std::fs::create_dir_all(emulator_images_dir);
    }

    if let Some(emulator_index_dir) = emulator_artwork_index_cache_dir() {
        let _ = std::fs::create_dir_all(emulator_index_dir);
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
            Self::Mattmc => load_artwork_textures_from_svg_bytes(ctx, key, MATTMC_SVG_BYTES),
            Self::Steam | Self::Emulator | Self::Noop => None,
        }
    }

    fn has_cached_artwork(self, target: &str) -> bool {
        match self {
            Self::Steam => find_cached_steam_portrait_artwork_path(target).is_some(),
            Self::Emulator => find_cached_emulator_artwork_path(target).is_some(),
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

fn process_download_job(job: ArtworkDownloadJob) -> ArtworkDownloadResult {
    match job.runner {
        ArtworkRunnerKind::Steam => {
            if let Some(payload) = prepare_cached_steam_artwork_payload(&job.target) {
                return ArtworkDownloadResult::Ready {
                    key: job.key,
                    payload,
                };
            }

            match download_and_prepare_steam_artwork_payload(&job.target) {
                Some(payload) => ArtworkDownloadResult::Ready {
                    key: job.key,
                    payload,
                },
                None => ArtworkDownloadResult::Missing { key: job.key },
            }
        }
        ArtworkRunnerKind::Emulator => {
            if let Some(payload) = prepare_cached_emulator_artwork_payload(&job.target) {
                return ArtworkDownloadResult::Ready {
                    key: job.key,
                    payload,
                };
            }

            match download_and_prepare_emulator_artwork_payload(&job.target) {
                Some(payload) => ArtworkDownloadResult::Ready {
                    key: job.key,
                    payload,
                },
                None => ArtworkDownloadResult::Missing { key: job.key },
            }
        }
        ArtworkRunnerKind::Mattmc | ArtworkRunnerKind::Noop => {
            ArtworkDownloadResult::Missing { key: job.key }
        }
    }
}

fn prepare_cached_steam_artwork_payload(appid: &str) -> Option<PreparedArtwork> {
    let cached_path = find_cached_steam_portrait_artwork_path(appid)?;
    if let Some(payload) = prepare_artwork_payload_from_path(&cached_path, None) {
        return Some(payload);
    }

    let _ = std::fs::remove_file(cached_path);
    None
}

fn download_and_prepare_steam_artwork_payload(appid: &str) -> Option<PreparedArtwork> {
    let cached_path = download_and_cache_steam_portrait_artwork(appid)?;
    prepare_artwork_payload_from_path(&cached_path, None)
}

fn prepare_cached_emulator_artwork_payload(launch_target: &str) -> Option<PreparedArtwork> {
    let cached_path = find_cached_emulator_artwork_path(launch_target)?;
    if let Some(payload) = prepare_artwork_payload_from_path(&cached_path, None) {
        return Some(payload);
    }

    let _ = std::fs::remove_file(cached_path);
    None
}

fn download_and_prepare_emulator_artwork_payload(launch_target: &str) -> Option<PreparedArtwork> {
    let cached_path = download_and_cache_emulator_artwork(launch_target)?;
    prepare_artwork_payload_from_path(&cached_path, None)
}

fn find_cached_emulator_artwork_path(launch_target: &str) -> Option<PathBuf> {
    let images_dir = emulator_artwork_images_cache_dir()?;
    let image_hash = stable_hash_hex(launch_target);
    let candidates = [
        images_dir.join(format!("{}.png", image_hash)),
        images_dir.join(format!("{}.jpg", image_hash)),
        images_dir.join(format!("{}.jpeg", image_hash)),
    ];

    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }

        if is_valid_emulator_artwork(&candidate) {
            return Some(candidate);
        }

        let _ = std::fs::remove_file(&candidate);
    }

    None
}

fn download_and_cache_emulator_artwork(launch_target: &str) -> Option<PathBuf> {
    let (system, rom_path) = parse_emulator_launch_target(launch_target)?;
    let rom_stem = rom_path.file_stem()?.to_string_lossy().to_string();
    let system_catalog = emulator_system_catalog_path(&system)?;
    let (primary_titles, region_fallback_titles) =
        build_emulator_boxart_title_candidates(&rom_stem);
    if primary_titles.is_empty() {
        return None;
    }

    let images_dir = emulator_artwork_images_cache_dir()?;
    let image_hash = stable_hash_hex(launch_target);

    for artwork_set in ["Named_Boxarts", "Named_Titles", "Named_Snaps"] {
        for candidate_title in &primary_titles {
            for extension in ["png", "jpg"] {
                let target_path = images_dir.join(format!("{}.{}", image_hash, extension));
                if target_path.is_file() && is_valid_emulator_artwork(&target_path) {
                    return Some(target_path);
                }

                let artwork_url =
                    build_emulator_boxart_url(system_catalog, artwork_set, candidate_title, extension);
                if download_url_to_file_with_user_agent(
                    &artwork_url,
                    &target_path,
                    EMULATOR_ARTWORK_USER_AGENT,
                ) && target_path.is_file()
                    && is_valid_emulator_artwork(&target_path)
                {
                    return Some(target_path);
                }

                let _ = std::fs::remove_file(&target_path);
            }
        }
    }

    for candidate_title in &region_fallback_titles {
        for extension in ["png", "jpg"] {
            let target_path = images_dir.join(format!("{}.{}", image_hash, extension));
            if target_path.is_file() && is_valid_emulator_artwork(&target_path) {
                return Some(target_path);
            }

            let artwork_url =
                build_emulator_boxart_url(system_catalog, "Named_Boxarts", candidate_title, extension);
            if download_url_to_file_with_user_agent(
                &artwork_url,
                &target_path,
                EMULATOR_ARTWORK_USER_AGENT,
            ) && target_path.is_file()
                && is_valid_emulator_artwork(&target_path)
            {
                return Some(target_path);
            }

            let _ = std::fs::remove_file(&target_path);
        }
    }

    let fuzzy_query_titles: Vec<String> = primary_titles
        .iter()
        .chain(region_fallback_titles.iter())
        .cloned()
        .collect();

    for artwork_set in ["Named_Boxarts", "Named_Titles", "Named_Snaps"] {
        let Some(best_filename) =
            find_best_fuzzy_listing_match_filename(system_catalog, artwork_set, &fuzzy_query_titles)
        else {
            continue;
        };

        let extension = Path::new(&best_filename)
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase())
            .filter(|value| value == "png" || value == "jpg" || value == "jpeg")
            .unwrap_or_else(|| "png".to_string());

        let target_path = images_dir.join(format!("{}.{}", image_hash, extension));
        if target_path.is_file() && is_valid_emulator_artwork(&target_path) {
            return Some(target_path);
        }

        let artwork_url =
            build_emulator_boxart_file_url(system_catalog, artwork_set, &best_filename);
        if download_url_to_file_with_user_agent(
            &artwork_url,
            &target_path,
            EMULATOR_ARTWORK_USER_AGENT,
        ) && target_path.is_file()
            && is_valid_emulator_artwork(&target_path)
        {
            return Some(target_path);
        }

        let _ = std::fs::remove_file(&target_path);
    }

    None
}

fn build_emulator_boxart_title_candidates(rom_stem: &str) -> (Vec<String>, Vec<String>) {
    let base_trimmed = rom_stem.trim();
    if base_trimmed.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let stripped = strip_bracketed_segments(base_trimmed)
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");

    let mut primary_candidates = Vec::new();
    push_unique_candidate(&mut primary_candidates, base_trimmed);
    push_unique_candidate(&mut primary_candidates, &stripped);

    if !stripped.is_empty() {
        let punctuation_softened = stripped
            .replace(':', " -")
            .replace(" - ", " ")
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");
        push_unique_candidate(&mut primary_candidates, &punctuation_softened);

        let without_the = stripped
            .strip_prefix("The ")
            .or_else(|| stripped.strip_prefix("the "))
            .unwrap_or(&stripped)
            .trim();
        push_unique_candidate(&mut primary_candidates, without_the);

        if let Some(with_trailing_article) = move_leading_article_to_trailing(without_the) {
            push_unique_candidate(&mut primary_candidates, &with_trailing_article);
        }

        if let Some(with_leading_article) = move_trailing_article_to_leading(without_the) {
            push_unique_candidate(&mut primary_candidates, &with_leading_article);
        }

        if without_the.contains(" and ") {
            push_unique_candidate(&mut primary_candidates, &without_the.replace(" and ", " & "));
        }

        if without_the.contains(" & ") {
            push_unique_candidate(&mut primary_candidates, &without_the.replace(" & ", " and "));
        }
    }

    let mut region_fallback_candidates = Vec::new();
    let base_for_region = if stripped.is_empty() {
        base_trimmed
    } else {
        stripped.as_str()
    };
    if !base_for_region.is_empty() {
        for region_tag in ["(USA)", "(Europe)", "(Japan)", "(World)"] {
            let tagged = format!("{} {}", base_for_region, region_tag);
            push_unique_candidate(&mut region_fallback_candidates, &tagged);
        }
    }

    (primary_candidates, region_fallback_candidates)
}

fn move_trailing_article_to_leading(value: &str) -> Option<String> {
    for article in [", The", ", A", ", An"] {
        if let Some(base) = value.strip_suffix(article) {
            let article_word = article.trim_start_matches(',').trim();
            let candidate = format!("{} {}", article_word, base.trim());
            return Some(candidate.split_whitespace().collect::<Vec<&str>>().join(" "));
        }
    }

    None
}

fn move_leading_article_to_trailing(value: &str) -> Option<String> {
    for article in ["The ", "A ", "An "] {
        if let Some(base) = value.strip_prefix(article) {
            let article_word = article.trim();
            let candidate = format!("{}, {}", base.trim(), article_word);
            return Some(candidate.split_whitespace().collect::<Vec<&str>>().join(" "));
        }
    }

    None
}

fn push_unique_candidate(candidates: &mut Vec<String>, candidate: &str) {
    let normalized = candidate.split_whitespace().collect::<Vec<&str>>().join(" ");
    if normalized.is_empty() {
        return;
    }

    if candidates.iter().any(|existing| existing.eq_ignore_ascii_case(&normalized)) {
        return;
    }

    candidates.push(normalized);
}

fn build_emulator_boxart_url(
    system_catalog: &str,
    artwork_set: &str,
    title: &str,
    extension: &str,
) -> String {
    let encoded_catalog = encode_url_path_segment(system_catalog);
    let encoded_set = encode_url_path_segment(artwork_set);
    let encoded_title = encode_url_path_segment(title);
    format!(
        "https://thumbnails.libretro.com/{}/{}/{}.{}",
        encoded_catalog, encoded_set, encoded_title, extension
    )
}

fn build_emulator_boxart_file_url(system_catalog: &str, artwork_set: &str, file_name: &str) -> String {
    let encoded_catalog = encode_url_path_segment(system_catalog);
    let encoded_set = encode_url_path_segment(artwork_set);
    let encoded_file_name = encode_url_path_segment(file_name);
    format!(
        "https://thumbnails.libretro.com/{}/{}/{}",
        encoded_catalog, encoded_set, encoded_file_name
    )
}

fn find_best_fuzzy_listing_match_filename(
    system_catalog: &str,
    artwork_set: &str,
    query_titles: &[String],
) -> Option<String> {
    let listing = load_thumbnail_listing(system_catalog, artwork_set)?;
    if listing.is_empty() {
        return None;
    }

    let query_norms: Vec<String> = query_titles
        .iter()
        .map(|title| normalize_matching_title(title))
        .filter(|value| !value.is_empty())
        .collect();
    if query_norms.is_empty() {
        return None;
    }

    let mut best_score = 0.0f32;
    let mut best_filename: Option<String> = None;

    for file_name in listing {
        let Some(stem) = Path::new(&file_name).file_stem().and_then(|value| value.to_str()) else {
            continue;
        };

        let candidate_norm = normalize_matching_title(stem);
        if candidate_norm.is_empty() {
            continue;
        }

        let mut score = 0.0f32;
        for query_norm in &query_norms {
            score = score.max(fuzzy_title_similarity(query_norm, &candidate_norm));
            if score >= 0.999 {
                break;
            }
        }

        if score > best_score {
            best_score = score;
            best_filename = Some(file_name);
        }
    }

    if best_score >= 0.58 {
        best_filename
    } else {
        None
    }
}

fn fuzzy_title_similarity(left: &str, right: &str) -> f32 {
    if left == right {
        return 1.0;
    }

    let left_tokens: Vec<&str> = left.split_whitespace().collect();
    let right_tokens: Vec<&str> = right.split_whitespace().collect();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    let mut matches = 0usize;
    for token in &left_tokens {
        if right_tokens.iter().any(|value| value == token) {
            matches += 1;
        }
    }

    let union = left_tokens.len() + right_tokens.len() - matches;
    let token_jaccard = if union == 0 {
        0.0
    } else {
        matches as f32 / union as f32
    };

    let contains_bonus = if left.contains(right) || right.contains(left) {
        1.0
    } else {
        0.0
    };

    let prefix_bonus = if left.starts_with(right) || right.starts_with(left) {
        1.0
    } else {
        0.0
    };

    let length_ratio = (left.len().min(right.len()) as f32) / (left.len().max(right.len()) as f32);

    (token_jaccard * 0.55) + (contains_bonus * 0.20) + (prefix_bonus * 0.15) + (length_ratio * 0.10)
}

fn load_thumbnail_listing(system_catalog: &str, artwork_set: &str) -> Option<Vec<String>> {
    let cache_key = format!("{}|{}", system_catalog, artwork_set);
    let in_memory_cache = thumbnail_listing_memory_cache();

    if let Ok(cache) = in_memory_cache.lock() {
        if let Some(existing) = cache.get(&cache_key) {
            return Some(existing.clone());
        }
    }

    let listing = if let Some(cached_listing) = read_thumbnail_listing_from_disk(system_catalog, artwork_set) {
        cached_listing
    } else {
        let fetched_listing = fetch_thumbnail_listing_from_remote(system_catalog, artwork_set)?;
        let _ = write_thumbnail_listing_to_disk(system_catalog, artwork_set, &fetched_listing);
        fetched_listing
    };

    if let Ok(mut cache) = in_memory_cache.lock() {
        cache.insert(cache_key, listing.clone());
    }

    Some(listing)
}

fn thumbnail_listing_memory_cache() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn read_thumbnail_listing_from_disk(system_catalog: &str, artwork_set: &str) -> Option<Vec<String>> {
    let file_path = thumbnail_listing_cache_file_path(system_catalog, artwork_set)?;
    let contents = std::fs::read_to_string(file_path).ok()?;

    let mut lines = contents.lines();
    let header = lines.next().unwrap_or_default();
    let Some(timestamp) = header
        .strip_prefix("#ts=")
        .and_then(|value| value.parse::<u64>().ok())
    else {
        return None;
    };

    let now = current_unix_timestamp_seconds();
    if now.saturating_sub(timestamp) > EMULATOR_ARTWORK_INDEX_TTL_SECONDS {
        return None;
    }

    let mut listing = Vec::new();
    for line in lines {
        let file_name = line.trim();
        if file_name.is_empty() {
            continue;
        }

        listing.push(file_name.to_string());
    }

    if listing.is_empty() {
        None
    } else {
        Some(listing)
    }
}

fn write_thumbnail_listing_to_disk(
    system_catalog: &str,
    artwork_set: &str,
    listing: &[String],
) -> Result<(), String> {
    let Some(file_path) = thumbnail_listing_cache_file_path(system_catalog, artwork_set) else {
        return Err("Failed to resolve thumbnail listing cache file path".to_string());
    };

    let mut serialized = format!("#ts={}\n", current_unix_timestamp_seconds());
    for file_name in listing {
        serialized.push_str(file_name);
        serialized.push('\n');
    }

    std::fs::write(file_path, serialized)
        .map_err(|error| format!("Failed to write thumbnail listing cache: {}", error))
}

fn thumbnail_listing_cache_file_path(system_catalog: &str, artwork_set: &str) -> Option<PathBuf> {
    let cache_dir = emulator_artwork_index_cache_dir()?;
    let cache_key = format!("{}|{}", system_catalog, artwork_set);
    let file_name = format!("{}.tsv", stable_hash_hex(&cache_key));
    Some(cache_dir.join(file_name))
}

fn fetch_thumbnail_listing_from_remote(system_catalog: &str, artwork_set: &str) -> Option<Vec<String>> {
    let directory_url = format!(
        "https://thumbnails.libretro.com/{}/{}/",
        encode_url_path_segment(system_catalog),
        encode_url_path_segment(artwork_set),
    );

    let response = ureq::get(&directory_url)
        .set("User-Agent", EMULATOR_ARTWORK_USER_AGENT)
        .timeout(Duration::from_secs(18))
        .call()
        .ok()?;

    if !(200..=299).contains(&response.status()) {
        return None;
    }

    let body = response.into_string().ok()?;
    let mut listing = Vec::new();
    let mut cursor = 0usize;

    while let Some(href_start_rel) = body[cursor..].find("href=\"") {
        let href_start = cursor + href_start_rel + 6;
        let Some(href_end_rel) = body[href_start..].find('"') else {
            break;
        };
        let href_end = href_start + href_end_rel;
        let href_value = &body[href_start..href_end];
        cursor = href_end + 1;

        if href_value.starts_with('?') || href_value.starts_with('/') {
            continue;
        }

        let decoded = decode_url_component(&href_value.replace("&amp;", "&"));
        let file_name = decoded
            .split('/')
            .next_back()
            .unwrap_or_default()
            .trim();

        if file_name.is_empty() {
            continue;
        }

        let lower = file_name.to_lowercase();
        if !(lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")) {
            continue;
        }

        listing.push(file_name.to_string());
    }

    if listing.is_empty() {
        None
    } else {
        Some(listing)
    }
}

fn decode_url_component(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = bytes[index + 1];
            let lo = bytes[index + 2];
            if let (Some(hi_val), Some(lo_val)) = (hex_value(hi), hex_value(lo)) {
                output.push((hi_val << 4) | lo_val);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).to_string()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(10 + value - b'a'),
        b'A'..=b'F' => Some(10 + value - b'A'),
        _ => None,
    }
}

fn emulator_system_catalog_path(system: &str) -> Option<&'static str> {
    match normalize_matching_title(system).as_str() {
        "nes" => Some("Nintendo - Nintendo Entertainment System"),
        "gba" => Some("Nintendo - Game Boy Advance"),
        "gb" => Some("Nintendo - Game Boy"),
        "gbc" => Some("Nintendo - Game Boy Color"),
        "snes" => Some("Nintendo - Super Nintendo Entertainment System"),
        "n64" => Some("Nintendo - Nintendo 64"),
        "nds" => Some("Nintendo - Nintendo DS"),
        "3ds" => Some("Nintendo - Nintendo 3DS"),
        "genesis" | "megadrive" | "md" => Some("Sega - Mega Drive - Genesis"),
        "sms" => Some("Sega - Master System - Mark III"),
        "gg" => Some("Sega - Game Gear"),
        "psx" | "ps1" => Some("Sony - PlayStation"),
        "psp" => Some("Sony - PlayStation Portable"),
        "atari2600" | "a2600" => Some("Atari - 2600"),
        _ => None,
    }
}

fn emulator_artwork_cache_root_dir() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let cache_root = PathBuf::from(home)
        .join(".basalt")
        .join("cache")
        .join("emulator_artwork");

    if std::fs::create_dir_all(&cache_root).is_err() {
        return None;
    }

    Some(cache_root)
}

fn emulator_artwork_images_cache_dir() -> Option<PathBuf> {
    let cache_dir = emulator_artwork_cache_root_dir()?.join(EMULATOR_ARTWORK_IMAGES_PATH);
    if std::fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }
    Some(cache_dir)
}

fn emulator_artwork_index_cache_dir() -> Option<PathBuf> {
    let cache_dir = emulator_artwork_cache_root_dir()?.join(EMULATOR_ARTWORK_INDEX_PATH);
    if std::fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }
    Some(cache_dir)
}

fn current_unix_timestamp_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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
    let mut parts = launch_target.splitn(3, '|');
    let backend = parts.next()?.trim().to_lowercase();
    let system = parts.next()?.trim().to_string();
    let rom_path = parts.next()?.trim();

    if backend != "retroarch" || system.is_empty() || rom_path.is_empty() {
        return None;
    }

    Some((system, PathBuf::from(rom_path)))
}

fn find_local_game_artwork_path(request: &ArtworkRequest) -> Option<PathBuf> {
    let artwork_dir = local_game_artwork_dir()?;
    let candidate_bases = build_local_artwork_name_candidates(request);

    for base_name in &candidate_bases {
        for extension in LOCAL_ARTWORK_EXTENSIONS {
            let candidate = artwork_dir.join(format!("{}.{}", base_name, extension));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let read_dir = std::fs::read_dir(&artwork_dir).ok()?;
    for entry in read_dir {
        let Ok(entry) = entry else {
            continue;
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        if !is_supported_local_artwork_extension(&extension) {
            continue;
        }

        let file_stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(normalize_local_artwork_basename)
            .unwrap_or_default();
        if file_stem.is_empty() {
            continue;
        }

        if candidate_bases.iter().any(|candidate| candidate == &file_stem) {
            return Some(path);
        }
    }

    None
}

fn build_local_artwork_name_candidates(request: &ArtworkRequest) -> Vec<String> {
    let mut candidates = Vec::new();

    push_local_candidate(&mut candidates, &request.display_name);
    push_local_candidate(&mut candidates, &request.key);
    push_local_candidate(&mut candidates, &request.target);

    if request.runner == ArtworkRunnerKind::Steam {
        if let Some(appid) = extract_steam_appid(&request.target) {
            push_local_candidate(&mut candidates, &appid);
        }
    }

    if request.runner == ArtworkRunnerKind::Emulator {
        if let Some((_, rom_path)) = parse_emulator_launch_target(&request.target) {
            if let Some(file_stem) = rom_path.file_stem().and_then(|value| value.to_str()) {
                push_local_candidate(&mut candidates, file_stem);
            }

            if let Some(file_name) = rom_path.file_name().and_then(|value| value.to_str()) {
                push_local_candidate(&mut candidates, file_name);
            }
        }
    }

    let normalized_title = normalize_matching_title(&request.display_name);
    push_local_candidate(&mut candidates, &normalized_title);
    push_local_candidate(&mut candidates, &normalized_title.replace(' ', "_"));
    push_local_candidate(&mut candidates, &normalized_title.replace(' ', "-"));

    candidates
}

fn push_local_candidate(candidates: &mut Vec<String>, raw_value: &str) {
    let normalized = normalize_local_artwork_basename(raw_value);
    if normalized.is_empty() {
        return;
    }

    if !candidates.iter().any(|existing| existing == &normalized) {
        candidates.push(normalized);
    }
}

fn normalize_local_artwork_basename(raw_value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;

    for character in raw_value.trim().chars() {
        let lowered = character.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            normalized.push(lowered);
            previous_was_space = false;
        } else if matches!(lowered, ' ' | '-' | '_') {
            if !previous_was_space {
                normalized.push(' ');
                previous_was_space = true;
            }
        }
    }

    normalized.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn is_supported_local_artwork_extension(extension: &str) -> bool {
    LOCAL_ARTWORK_EXTENSIONS
        .iter()
        .any(|value| value.eq_ignore_ascii_case(extension))
}

fn local_game_artwork_dir() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(LOCAL_GAME_ARTWORK_DIR);
    if std::fs::create_dir_all(&manifest_dir).is_ok() {
        return Some(manifest_dir);
    }

    let workspace_relative = PathBuf::from(LOCAL_GAME_ARTWORK_DIR);
    if std::fs::create_dir_all(&workspace_relative).is_ok() {
        return Some(workspace_relative);
    }

    None
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

fn prepare_artwork_payload_from_path(
    path: &Path,
    blur_background_rgb: Option<[u8; 3]>,
) -> Option<PreparedArtwork> {
    let image_bytes = std::fs::read(path).ok()?;
    let foreground_rgba = image::load_from_memory(&image_bytes).ok()?.to_rgba8();
    prepare_artwork_payload_from_rgba(foreground_rgba, blur_background_rgb)
}

fn prepare_artwork_payload_from_rgba(
    foreground_rgba: image::RgbaImage,
    blur_background_rgb: Option<[u8; 3]>,
) -> Option<PreparedArtwork> {
    let foreground_rgba = resize_for_prepared_artwork(
        foreground_rgba,
        PREPARED_ARTWORK_MAX_WIDTH,
        PREPARED_ARTWORK_MAX_HEIGHT,
    );

    let background_rgba = if let Some(background_rgb) = blur_background_rgb {
        composite_on_solid_background(&foreground_rgba, background_rgb)
    } else {
        foreground_rgba.clone()
    };

    let width = usize::try_from(foreground_rgba.width()).ok()?;
    let height = usize::try_from(foreground_rgba.height()).ok()?;

    Some(PreparedArtwork {
        width,
        height,
        foreground_rgba: foreground_rgba.into_raw(),
        background_rgba: background_rgba.into_raw(),
    })
}

fn resize_for_prepared_artwork(
    rgba: image::RgbaImage,
    max_width: u32,
    max_height: u32,
) -> image::RgbaImage {
    let width = rgba.width();
    let height = rgba.height();

    if width == 0 || height == 0 || width <= max_width && height <= max_height {
        return rgba;
    }

    let width_scale = max_width as f32 / width as f32;
    let height_scale = max_height as f32 / height as f32;
    let scale = width_scale.min(height_scale).min(1.0);

    let resized_width = ((width as f32) * scale).round().max(1.0) as u32;
    let resized_height = ((height as f32) * scale).round().max(1.0) as u32;

    image::imageops::resize(
        &rgba,
        resized_width,
        resized_height,
        image::imageops::FilterType::Triangle,
    )
}

fn build_artwork_textures_from_payload(
    ctx: &Context,
    key: &str,
    payload: PreparedArtwork,
) -> Option<ArtworkTextures> {
    let foreground_color_image = egui::ColorImage::from_rgba_unmultiplied(
        [payload.width, payload.height],
        &payload.foreground_rgba,
    );
    let background_color_image = egui::ColorImage::from_rgba_unmultiplied(
        [payload.width, payload.height],
        &payload.background_rgba,
    );

    let foreground = ctx.load_texture(
        format!("game-artwork-fg-{}", key),
        foreground_color_image,
        egui::TextureOptions::LINEAR,
    );
    let background_blur = ctx.load_texture(
        format!("game-artwork-bg-{}", key),
        background_color_image,
        egui::TextureOptions::LINEAR,
    );

    Some(ArtworkTextures {
        foreground,
        _background_blur: background_blur,
    })
}

fn load_artwork_textures_from_svg_bytes(
    ctx: &Context,
    key: &str,
    svg_bytes: &[u8],
) -> Option<ArtworkTextures> {
    let usvg_options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(svg_bytes, &usvg_options).ok()?;
    let base_size = tree.size().to_int_size();
    let largest_dim = base_size.width().max(base_size.height()) as f32;
    if largest_dim <= 0.0 {
        return None;
    }

    let scale = (MATTMC_SVG_TARGET_MAX_DIMENSION as f32 / largest_dim).max(1.0);
    let target_width = ((base_size.width() as f32) * scale).round().max(1.0) as u32;
    let target_height = ((base_size.height() as f32) * scale).round().max(1.0) as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_width, target_height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let foreground_rgba = image::RgbaImage::from_raw(
        target_width,
        target_height,
        pixmap.data().to_vec(),
    )?;

    build_artwork_textures_from_rgba(
        ctx,
        key,
        foreground_rgba,
        Some(MATTMC_BLUR_BACKGROUND_RGB),
    )
}

fn build_artwork_textures_from_rgba(
    ctx: &Context,
    key: &str,
    foreground_rgba: image::RgbaImage,
    blur_background_rgb: Option<[u8; 3]>,
) -> Option<ArtworkTextures> {
    let payload = prepare_artwork_payload_from_rgba(foreground_rgba, blur_background_rgb)?;
    build_artwork_textures_from_payload(ctx, key, payload)
}

fn composite_on_solid_background(
    foreground: &image::RgbaImage,
    background_rgb: [u8; 3],
) -> image::RgbaImage {
    let mut output = image::RgbaImage::new(foreground.width(), foreground.height());

    for (x, y, pixel) in foreground.enumerate_pixels() {
        let alpha = pixel.0[3] as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;

        let r = (pixel.0[0] as f32 * alpha + background_rgb[0] as f32 * inv_alpha).round() as u8;
        let g = (pixel.0[1] as f32 * alpha + background_rgb[1] as f32 * inv_alpha).round() as u8;
        let b = (pixel.0[2] as f32 * alpha + background_rgb[2] as f32 * inv_alpha).round() as u8;

        output.put_pixel(x, y, image::Rgba([r, g, b, 255]));
    }

    output
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

fn find_cached_steam_portrait_artwork_path(appid: &str) -> Option<PathBuf> {
    let cache_dir = steam_artwork_cache_dir()?;
    let cached_candidates = [
        cache_dir.join(format!("{}_library_600x900_2x.jpg", appid)),
        cache_dir.join(format!("{}_library_600x900.jpg", appid)),
        cache_dir.join(format!("{}_library_600x900.png", appid)),
        cache_dir.join(format!("{}_library_600x900_2x_alt.jpg", appid)),
        cache_dir.join(format!("{}_library_600x900_alt.jpg", appid)),
        cache_dir.join(format!("{}_library_600x900_alt.png", appid)),
    ];

    cached_candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
}

fn download_and_cache_steam_portrait_artwork(appid: &str) -> Option<PathBuf> {
    let cache_dir = steam_artwork_cache_dir()?;

    if let Some(existing_cached) = find_cached_steam_portrait_artwork_path(appid) {
        return Some(existing_cached);
    }

    let urls_and_targets = [
        (
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900_2x.jpg",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900_2x.jpg", appid)),
        ),
        (
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900.jpg", appid)),
        ),
        (
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.png",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900.png", appid)),
        ),
        (
            format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900_2x.jpg",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900_2x_alt.jpg", appid)),
        ),
        (
            format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900.jpg",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900_alt.jpg", appid)),
        ),
        (
            format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900.png",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900_alt.png", appid)),
        ),
    ];

    for (url, target_path) in urls_and_targets {
        if target_path.is_file() {
            if is_valid_portrait_artwork(&target_path) {
                return Some(target_path);
            }

            let _ = std::fs::remove_file(&target_path);
        }

        if download_url_to_file(&url, &target_path)
            && target_path.is_file()
            && is_valid_portrait_artwork(&target_path)
        {
            return Some(target_path);
        }

        let _ = std::fs::remove_file(&target_path);
    }

    None
}

fn download_url_to_file(url: &str, target_path: &Path) -> bool {
    download_url_to_file_with_user_agent(url, target_path, "Basalt-Steam-Artwork")
}

fn download_url_to_file_with_user_agent(url: &str, target_path: &Path, user_agent: &str) -> bool {
    const MAX_RETRIES: usize = 2;
    const HTTP_TIMEOUT_SECONDS: u64 = 12;

    for _ in 0..=MAX_RETRIES {
        let request = ureq::get(url)
            .set("User-Agent", user_agent)
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS));
        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(status, _)) => {
                if (500..=599).contains(&status) {
                    continue;
                }

                return false;
            }
            Err(_) => continue,
        };

        let status = response.status();
        if !(200..=299).contains(&status) {
            continue;
        }

        let mut reader = response.into_reader();
        let mut file = match File::create(target_path) {
            Ok(file) => file,
            Err(_) => {
                let _ = std::fs::remove_file(target_path);
                continue;
            }
        };

        match copy(&mut reader, &mut file) {
            Ok(_) => return true,
            Err(_) => {
                let _ = std::fs::remove_file(target_path);
            }
        }
    }

    false
}

fn steam_artwork_cache_dir() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let cache_dir = PathBuf::from(home)
        .join(".basalt")
        .join("cache")
        .join("steam_artwork");

    if std::fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }

    Some(cache_dir)
}

fn is_valid_portrait_artwork(path: &Path) -> bool {
    let Ok(image_reader) = image::ImageReader::open(path) else {
        return false;
    };

    let Ok((width, height)) = image_reader.into_dimensions() else {
        return false;
    };

    if width < 300 || height < 450 {
        return false;
    }

    let aspect_ratio = width as f32 / height as f32;
    (0.60..=0.74).contains(&aspect_ratio)
}

fn is_valid_emulator_artwork(path: &Path) -> bool {
    let Ok(image_reader) = image::ImageReader::open(path) else {
        return false;
    };

    let Ok((width, height)) = image_reader.into_dimensions() else {
        return false;
    };

    width >= EMULATOR_ARTWORK_MIN_WIDTH && height >= EMULATOR_ARTWORK_MIN_HEIGHT
}
