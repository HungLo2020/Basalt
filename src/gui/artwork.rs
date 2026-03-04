use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};
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
const MAX_PENDING_DOWNLOADS: usize = 6;
const MAX_DOWNLOAD_QUEUE: usize = 48;
const MAX_RESULT_QUEUE: usize = 96;
const ARTWORK_WORKER_COUNT: usize = 4;
const MAX_TEXTURE_UPLOADS_PER_TICK: usize = 2;

#[derive(Clone)]
pub(super) struct ArtworkTextures {
    pub(super) foreground: TextureHandle,
    pub(super) background_blur: TextureHandle,
}

pub(super) struct ArtworkStore {
    textures: HashMap<String, ArtworkTextures>,
    missing: HashSet<String>,
    requested: HashSet<String>,
    download_tx: Sender<ArtworkDownloadJob>,
    result_rx: Receiver<ArtworkDownloadResult>,
}

impl ArtworkStore {
    pub(super) fn new() -> Self {
        let (download_tx, download_rx) = bounded::<ArtworkDownloadJob>(MAX_DOWNLOAD_QUEUE);
        let (result_tx, result_rx) = bounded::<ArtworkDownloadResult>(MAX_RESULT_QUEUE);

        for _ in 0..ARTWORK_WORKER_COUNT {
            let worker_download_rx = download_rx.clone();
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
            requested: HashSet::new(),
            download_tx,
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

            match result {
                ArtworkDownloadResult::Ready { key, payload } => {
                    self.requested.remove(&key);

                    if let Some(textures) = build_artwork_textures_from_payload(ctx, &key, payload) {
                        self.textures.insert(key.clone(), textures);
                        self.missing.remove(&key);
                        has_updates = true;
                    } else {
                        self.missing.insert(key);
                    }
                }
                ArtworkDownloadResult::Missing { key } => {
                    self.requested.remove(&key);
                    self.missing.insert(key);
                }
            }

            processed += 1;
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
        }

        self.textures.retain(|key, _| visible_keys.contains(key));
        self.requested.retain(|key| visible_keys.contains(key));
        self.missing.retain(|key| visible_keys.contains(key));
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

        if let Some(builtin_textures) = request.runner.load_builtin_artwork(ctx, &request.key) {
            self.textures
                .insert(request.key.clone(), builtin_textures.clone());
            return Some(builtin_textures);
        }

        self.request_download(request);
        None
    }

    fn request_download(&mut self, request: ArtworkRequest) {
        if self.missing.contains(&request.key) || self.requested.contains(&request.key) {
            return;
        }

        let has_cached_artwork = request.runner.has_cached_artwork(&request.target);

        if !has_cached_artwork && self.requested.len() >= MAX_PENDING_DOWNLOADS {
            return;
        }

        let Some(job) = request
            .runner
            .to_download_job(request.key.clone(), request.target.clone())
        else {
            return;
        };

        match self.download_tx.try_send(job) {
            Ok(_) => {
                self.requested.insert(request.key);
            }
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {
                self.missing.insert(request.key);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ArtworkRunnerKind {
    Mattmc,
    Steam,
    Noop,
}

impl ArtworkRunnerKind {
    fn from_game(game: &GameEntry) -> Self {
        if game.name.eq_ignore_ascii_case("MattMC") {
            Self::Mattmc
        } else if game.runner_kind.as_str() == "steam" {
            Self::Steam
        } else {
            Self::Noop
        }
    }

    fn build_request(self, game: &GameEntry) -> Option<ArtworkRequest> {
        match self {
            Self::Mattmc => Some(ArtworkRequest {
                key: "mattmc:default".to_string(),
                target: String::new(),
                runner: self,
            }),
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

    fn load_builtin_artwork(self, ctx: &Context, key: &str) -> Option<ArtworkTextures> {
        match self {
            Self::Mattmc => load_artwork_textures_from_svg_bytes(ctx, key, MATTMC_SVG_BYTES),
            Self::Steam | Self::Noop => None,
        }
    }

    fn has_cached_artwork(self, target: &str) -> bool {
        match self {
            Self::Steam => find_cached_steam_portrait_artwork_path(target).is_some(),
            Self::Mattmc | Self::Noop => false,
        }
    }

    fn to_download_job(self, key: String, target: String) -> Option<ArtworkDownloadJob> {
        match self {
            Self::Steam => Some(ArtworkDownloadJob {
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

    let blur_source = if let Some(background_rgb) = blur_background_rgb {
        composite_on_solid_background(&foreground_rgba, background_rgb)
    } else {
        foreground_rgba.clone()
    };

    let mut blurred_rgba = image::DynamicImage::ImageRgba8(blur_source)
        .blur(10.0)
        .to_rgba8();

    for pixel in blurred_rgba.pixels_mut() {
        pixel.0[0] = ((pixel.0[0] as u16 * 70) / 100) as u8;
        pixel.0[1] = ((pixel.0[1] as u16 * 70) / 100) as u8;
        pixel.0[2] = ((pixel.0[2] as u16 * 70) / 100) as u8;
    }

    let width = usize::try_from(foreground_rgba.width()).ok()?;
    let height = usize::try_from(foreground_rgba.height()).ok()?;

    Some(PreparedArtwork {
        width,
        height,
        foreground_rgba: foreground_rgba.into_raw(),
        background_rgba: blurred_rgba.into_raw(),
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
        background_blur,
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
    const MAX_RETRIES: usize = 2;
    const HTTP_TIMEOUT_SECONDS: u64 = 12;

    for _ in 0..=MAX_RETRIES {
        let request = ureq::get(url).timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS));
        let response = match request.call() {
            Ok(response) => response,
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
