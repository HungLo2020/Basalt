use std::path::{Path, PathBuf};

use super::cache;
use super::validation;
pub(super) fn fetch_steam_portrait_artwork(appid: &str) -> Option<PathBuf> {
    if let Some(cached_path) = find_cached_steam_portrait_artwork_path(appid) {
        if validation::is_valid_portrait_artwork(&cached_path) {
            return Some(cached_path);
        }
        let _ = std::fs::remove_file(cached_path);
    }

    download_and_cache_steam_portrait_artwork(appid)
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

fn download_and_cache_steam_portrait_artwork(appid: &str) -> Option<PathBuf> {
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
            if validation::is_valid_portrait_artwork(&target_path) {
                return Some(target_path);
            }

            let _ = std::fs::remove_file(&target_path);
        }

        if download_url_to_file(&url, &target_path)
            && target_path.is_file()
            && validation::is_valid_portrait_artwork(&target_path)
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
            .header("User-Agent", user_agent)
            .config()
            .timeout_global(Some(std::time::Duration::from_secs(HTTP_TIMEOUT_SECONDS)))
            .build();
        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(status)) => {
                if (500..=599).contains(&status) {
                    continue;
                }

                return false;
            }
            Err(_) => continue,
        };

        if !response.status().is_success() {
            continue;
        }

        let mut reader = response.into_body().into_reader();
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
