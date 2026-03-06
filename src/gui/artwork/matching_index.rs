use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::cache;
use super::{
    encode_url_path_segment,
    normalize_matching_title,
    stable_hash_hex,
    strip_bracketed_segments,
    EMULATOR_ARTWORK_INDEX_TTL_SECONDS,
    EMULATOR_ARTWORK_USER_AGENT,
};

pub(super) fn build_emulator_boxart_title_candidates(rom_stem: &str) -> (Vec<String>, Vec<String>) {
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

pub(super) fn build_emulator_boxart_url(
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

pub(super) fn build_emulator_boxart_file_url(
    system_catalog: &str,
    artwork_set: &str,
    file_name: &str,
) -> String {
    let encoded_catalog = encode_url_path_segment(system_catalog);
    let encoded_set = encode_url_path_segment(artwork_set);
    let encoded_file_name = encode_url_path_segment(file_name);
    format!(
        "https://thumbnails.libretro.com/{}/{}/{}",
        encoded_catalog, encoded_set, encoded_file_name
    )
}

pub(super) fn find_best_fuzzy_listing_match_filename(
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

    let now = cache::current_unix_timestamp_seconds();
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

    let mut serialized = format!("#ts={}\n", cache::current_unix_timestamp_seconds());
    for file_name in listing {
        serialized.push_str(file_name);
        serialized.push('\n');
    }

    std::fs::write(file_path, serialized)
        .map_err(|error| format!("Failed to write thumbnail listing cache: {}", error))
}

fn thumbnail_listing_cache_file_path(system_catalog: &str, artwork_set: &str) -> Option<std::path::PathBuf> {
    let cache_dir = cache::emulator_artwork_index_cache_dir()?;
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
        .timeout(std::time::Duration::from_secs(18))
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
