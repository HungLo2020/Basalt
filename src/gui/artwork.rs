use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui::{self, Context, TextureHandle};

use crate::core::GameEntry;

pub(super) struct ArtworkStore {
    textures: HashMap<String, TextureHandle>,
    missing: HashSet<String>,
    requested: HashSet<String>,
    download_tx: Sender<ArtworkDownloadJob>,
    result_rx: Receiver<ArtworkDownloadResult>,
}

impl ArtworkStore {
    pub(super) fn new() -> Self {
        let (download_tx, download_rx) = mpsc::channel::<ArtworkDownloadJob>();
        let (result_tx, result_rx) = mpsc::channel::<ArtworkDownloadResult>();

        thread::spawn(move || {
            while let Ok(job) = download_rx.recv() {
                let cached = match job.runner {
                    ArtworkRunnerKind::Steam => {
                        download_and_cache_steam_portrait_artwork(&job.target).is_some()
                    }
                    ArtworkRunnerKind::Noop => false,
                };

                if result_tx.send(ArtworkDownloadResult { key: job.key, cached }).is_err() {
                    break;
                }
            }
        });

        Self {
            textures: HashMap::new(),
            missing: HashSet::new(),
            requested: HashSet::new(),
            download_tx,
            result_rx,
        }
    }

    pub(super) fn poll_download_results(&mut self, ctx: &Context) {
        let mut has_updates = false;

        while let Ok(result) = self.result_rx.try_recv() {
            self.requested.remove(&result.key);
            if result.cached {
                self.missing.remove(&result.key);
                has_updates = true;
            } else {
                self.missing.insert(result.key);
            }
        }

        if has_updates {
            ctx.request_repaint();
        }
    }

    pub(super) fn prepare_for_games(&mut self, games: &[GameEntry]) {
        let mut visible_keys = HashSet::new();

        for game in games {
            let runner = ArtworkRunnerKind::from_game(game);
            let Some(request) = runner.build_request(game) else {
                continue;
            };

            visible_keys.insert(request.key.clone());

            if request.runner.find_cached_artwork_path(&request.target).is_some() {
                self.missing.remove(&request.key);
                continue;
            }

            self.request_download(request);
        }

        self.textures.retain(|key, _| visible_keys.contains(key));
        self.requested.retain(|key| visible_keys.contains(key));
        self.missing.retain(|key| visible_keys.contains(key));
    }

    pub(super) fn texture_for_game(&mut self, ctx: &Context, game: &GameEntry) -> Option<TextureHandle> {
        let runner = ArtworkRunnerKind::from_game(game);
        let request = runner.build_request(game)?;

        if let Some(existing_texture) = self.textures.get(&request.key) {
            return Some(existing_texture.clone());
        }

        if let Some(cached_path) = request.runner.find_cached_artwork_path(&request.target) {
            if let Some(texture_handle) = load_texture_from_path(ctx, &request.key, &cached_path) {
                self.textures
                    .insert(request.key.clone(), texture_handle.clone());
                return Some(texture_handle);
            }

            let _ = std::fs::remove_file(cached_path);
        }

        self.request_download(request);
        None
    }

    fn request_download(&mut self, request: ArtworkRequest) {
        if self.missing.contains(&request.key) || self.requested.contains(&request.key) {
            return;
        }

        let Some(job) = request.runner.to_download_job(request.key.clone(), request.target.clone())
        else {
            self.missing.insert(request.key);
            return;
        };

        if self.download_tx.send(job).is_ok() {
            self.requested.insert(request.key);
        } else {
            self.missing.insert(request.key);
        }
    }
}

#[derive(Clone, Copy)]
enum ArtworkRunnerKind {
    Steam,
    Noop,
}

impl ArtworkRunnerKind {
    fn from_game(game: &GameEntry) -> Self {
        if game.runner_kind.as_str() == "steam" {
            Self::Steam
        } else {
            Self::Noop
        }
    }

    fn build_request(self, game: &GameEntry) -> Option<ArtworkRequest> {
        match self {
            Self::Steam => {
                let appid = extract_steam_appid(&game.launch_target)?;
                Some(ArtworkRequest {
                    key: format!("steam:{}", appid),
                    target: appid,
                    runner: self,
                })
            }
            Self::Noop => None,
        }
    }

    fn find_cached_artwork_path(self, target: &str) -> Option<PathBuf> {
        match self {
            Self::Steam => find_cached_steam_portrait_artwork_path(target),
            Self::Noop => None,
        }
    }

    fn to_download_job(self, key: String, target: String) -> Option<ArtworkDownloadJob> {
        match self {
            Self::Steam => Some(ArtworkDownloadJob {
                key,
                target,
                runner: self,
            }),
            Self::Noop => None,
        }
    }
}

#[derive(Clone)]
struct ArtworkRequest {
    key: String,
    target: String,
    runner: ArtworkRunnerKind,
}

struct ArtworkDownloadResult {
    key: String,
    cached: bool,
}

struct ArtworkDownloadJob {
    key: String,
    target: String,
    runner: ArtworkRunnerKind,
}

fn load_texture_from_path(ctx: &Context, key: &str, path: &Path) -> Option<TextureHandle> {
    let image_bytes = std::fs::read(path).ok()?;
    let decoded = image::load_from_memory(&image_bytes).ok()?.to_rgba8();

    let width = usize::try_from(decoded.width()).ok()?;
    let height = usize::try_from(decoded.height()).ok()?;
    let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], decoded.as_raw());

    Some(ctx.load_texture(
        format!("game-artwork-{}", key),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
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

    for candidate in cached_candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn download_and_cache_steam_portrait_artwork(appid: &str) -> Option<PathBuf> {
    let cache_dir = steam_artwork_cache_dir()?;
    if let Some(existing_cached) = find_cached_steam_portrait_artwork_path(appid) {
        if is_valid_portrait_artwork(&existing_cached) {
            return Some(existing_cached);
        }

        let _ = std::fs::remove_file(existing_cached);
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

        let output = Command::new("curl")
            .args(["-fsSL", "--retry", "2", "--output"])
            .arg(&target_path)
            .arg(&url)
            .output()
            .ok()?;

        if output.status.success() && target_path.is_file() && is_valid_portrait_artwork(&target_path)
        {
            return Some(target_path);
        }

        let _ = std::fs::remove_file(&target_path);
    }

    None
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

    let Ok(image_data) = image_reader.decode() else {
        return false;
    };

    let width = image_data.width();
    let height = image_data.height();
    if width < 300 || height < 450 {
        return false;
    }

    let aspect_ratio = width as f32 / height as f32;
    (0.60..=0.74).contains(&aspect_ratio)
}
