use std::path::{Path, PathBuf};

use super::cache;
use super::matching_index;
use super::texture_prep;
use super::{
    emulator_system_catalog_path,
    parse_emulator_launch_target,
    stable_hash_hex,
    ArtworkDownloadJob,
    ArtworkDownloadResult,
    PreparedArtwork,
    EMULATOR_ARTWORK_USER_AGENT,
};

pub(super) fn process_download_job(job: ArtworkDownloadJob) -> ArtworkDownloadResult {
    match job.runner {
        super::ArtworkRunnerKind::Steam => {
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
        super::ArtworkRunnerKind::Emulator => {
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
        super::ArtworkRunnerKind::Mattmc | super::ArtworkRunnerKind::Noop => {
            ArtworkDownloadResult::Missing { key: job.key }
        }
    }
}

pub(super) fn prepare_cached_steam_artwork_payload(appid: &str) -> Option<PreparedArtwork> {
    let cached_path = find_cached_steam_portrait_artwork_path(appid)?;
    if let Some(payload) = texture_prep::prepare_artwork_payload_from_path(&cached_path, None) {
        return Some(payload);
    }

    let _ = std::fs::remove_file(cached_path);
    None
}

pub(super) fn download_and_prepare_steam_artwork_payload(appid: &str) -> Option<PreparedArtwork> {
    let cached_path = download_and_cache_steam_portrait_artwork(appid)?;
    texture_prep::prepare_artwork_payload_from_path(&cached_path, None)
}

pub(super) fn prepare_cached_emulator_artwork_payload(launch_target: &str) -> Option<PreparedArtwork> {
    let cached_path = find_cached_emulator_artwork_path(launch_target)?;
    if let Some(payload) = texture_prep::prepare_artwork_payload_from_path(&cached_path, None) {
        return Some(payload);
    }

    let _ = std::fs::remove_file(cached_path);
    None
}

pub(super) fn download_and_prepare_emulator_artwork_payload(launch_target: &str) -> Option<PreparedArtwork> {
    let cached_path = download_and_cache_emulator_artwork(launch_target)?;
    texture_prep::prepare_artwork_payload_from_path(&cached_path, None)
}

pub(super) fn find_cached_emulator_artwork_path(launch_target: &str) -> Option<PathBuf> {
    let images_dir = cache::emulator_artwork_images_cache_dir()?;
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

        if texture_prep::is_valid_emulator_artwork(&candidate) {
            return Some(candidate);
        }

        let _ = std::fs::remove_file(&candidate);
    }

    None
}

pub(super) fn download_and_cache_emulator_artwork(launch_target: &str) -> Option<PathBuf> {
    let (system, rom_path) = parse_emulator_launch_target(launch_target)?;
    let rom_stem = rom_path.file_stem()?.to_string_lossy().to_string();
    let system_catalog = emulator_system_catalog_path(&system)?;
    let (primary_titles, region_fallback_titles) =
        matching_index::build_emulator_boxart_title_candidates(&rom_stem);
    if primary_titles.is_empty() {
        return None;
    }

    let images_dir = cache::emulator_artwork_images_cache_dir()?;
    let image_hash = stable_hash_hex(launch_target);

    for artwork_set in ["Named_Boxarts", "Named_Titles", "Named_Snaps"] {
        for candidate_title in &primary_titles {
            for extension in ["png", "jpg"] {
                let target_path = images_dir.join(format!("{}.{}", image_hash, extension));
                if target_path.is_file() && texture_prep::is_valid_emulator_artwork(&target_path) {
                    return Some(target_path);
                }

                let artwork_url =
                    matching_index::build_emulator_boxart_url(system_catalog, artwork_set, candidate_title, extension);
                if download_url_to_file_with_user_agent(
                    &artwork_url,
                    &target_path,
                    EMULATOR_ARTWORK_USER_AGENT,
                ) && target_path.is_file()
                    && texture_prep::is_valid_emulator_artwork(&target_path)
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
            if target_path.is_file() && texture_prep::is_valid_emulator_artwork(&target_path) {
                return Some(target_path);
            }

            let artwork_url =
                matching_index::build_emulator_boxart_url(system_catalog, "Named_Boxarts", candidate_title, extension);
            if download_url_to_file_with_user_agent(
                &artwork_url,
                &target_path,
                EMULATOR_ARTWORK_USER_AGENT,
            ) && target_path.is_file()
                && texture_prep::is_valid_emulator_artwork(&target_path)
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
            matching_index::find_best_fuzzy_listing_match_filename(system_catalog, artwork_set, &fuzzy_query_titles)
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
        if target_path.is_file() && texture_prep::is_valid_emulator_artwork(&target_path) {
            return Some(target_path);
        }

        let artwork_url =
            matching_index::build_emulator_boxart_file_url(system_catalog, artwork_set, &best_filename);
        if download_url_to_file_with_user_agent(
            &artwork_url,
            &target_path,
            EMULATOR_ARTWORK_USER_AGENT,
        ) && target_path.is_file()
            && texture_prep::is_valid_emulator_artwork(&target_path)
        {
            return Some(target_path);
        }

        let _ = std::fs::remove_file(&target_path);
    }

    None
}

pub(super) fn find_cached_steam_portrait_artwork_path(appid: &str) -> Option<PathBuf> {
    let cache_dir = cache::steam_artwork_cache_dir()?;
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

pub(super) fn download_and_cache_steam_portrait_artwork(appid: &str) -> Option<PathBuf> {
    let cache_dir = cache::steam_artwork_cache_dir()?;

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
            if texture_prep::is_valid_portrait_artwork(&target_path) {
                return Some(target_path);
            }

            let _ = std::fs::remove_file(&target_path);
        }

        if download_url_to_file(&url, &target_path)
            && target_path.is_file()
            && texture_prep::is_valid_portrait_artwork(&target_path)
        {
            return Some(target_path);
        }

        let _ = std::fs::remove_file(&target_path);
    }

    None
}

pub(super) fn download_url_to_file(url: &str, target_path: &Path) -> bool {
    download_url_to_file_with_user_agent(url, target_path, "Basalt-Steam-Artwork")
}

pub(super) fn download_url_to_file_with_user_agent(
    url: &str,
    target_path: &Path,
    user_agent: &str,
) -> bool {
    const MAX_RETRIES: usize = 2;
    const HTTP_TIMEOUT_SECONDS: u64 = 12;

    for _ in 0..=MAX_RETRIES {
        let request = ureq::get(url)
            .set("User-Agent", user_agent)
            .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECONDS));
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
        let mut file = match std::fs::File::create(target_path) {
            Ok(file) => file,
            Err(_) => {
                let _ = std::fs::remove_file(target_path);
                continue;
            }
        };

        match std::io::copy(&mut reader, &mut file) {
            Ok(_) => return true,
            Err(_) => {
                let _ = std::fs::remove_file(target_path);
            }
        }
    }

    false
}
