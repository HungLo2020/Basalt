use std::path::{Path, PathBuf};

use super::{
    ArtworkKind, ArtworkRequest, LOCAL_ARTWORK_EXTENSIONS, extract_steam_appid,
    normalize_matching_title, parse_emulator_launch_target,
};
use crate::{platform, storage};

const ARTWORK_DIR_NAME: &str = "artwork";

/// Folders searched for override images, highest priority first:
/// 1. the user's own folder (`~/.local/share/basalt/artwork` on Linux)
/// 2. in debug builds, the repository's `resources/gameartwork`
/// 3. artwork installed with the package (`/usr/share/basalt/artwork` on Linux)
pub fn override_artwork_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(data_dir) = storage::data_dir() {
        dirs.push(data_dir.join(ARTWORK_DIR_NAME));
    }

    if cfg!(debug_assertions) {
        dirs.push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/gameartwork"
        )));
    }

    dirs.extend(
        platform::system_data_dirs()
            .into_iter()
            .map(|dir| dir.join(ARTWORK_DIR_NAME)),
    );

    dirs
}

/// The user's override folder, created if missing so it is easy to find and drop images into.
pub fn user_override_artwork_dir() -> Option<PathBuf> {
    let dir = storage::data_dir().ok()?.join(ARTWORK_DIR_NAME);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub(super) fn find_local_game_artwork_path(request: &ArtworkRequest) -> Option<PathBuf> {
    let candidate_bases = build_local_artwork_name_candidates(request);
    override_artwork_dirs()
        .iter()
        .filter(|dir| dir.is_dir())
        .find_map(|dir| find_in_dir(dir, &candidate_bases))
}

fn find_in_dir(artwork_dir: &Path, candidate_bases: &[String]) -> Option<PathBuf> {
    for base_name in candidate_bases {
        for extension in LOCAL_ARTWORK_EXTENSIONS {
            let candidate = artwork_dir.join(format!("{}.{}", base_name, extension));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let read_dir = std::fs::read_dir(artwork_dir).ok()?;
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

        if candidate_bases
            .iter()
            .any(|candidate| candidate == &file_stem)
        {
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

    if request.kind == ArtworkKind::Steam
        && let Some(appid) = extract_steam_appid(&request.target)
    {
        push_local_candidate(&mut candidates, &appid);
    }

    if request.kind == ArtworkKind::Emulator
        && let Some((_, rom_path)) = parse_emulator_launch_target(&request.target)
    {
        if let Some(file_stem) = rom_path.file_stem().and_then(|value| value.to_str()) {
            push_local_candidate(&mut candidates, file_stem);
        }

        if let Some(file_name) = rom_path.file_name().and_then(|value| value.to_str()) {
            push_local_candidate(&mut candidates, file_name);
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
        } else if matches!(lowered, ' ' | '-' | '_') && !previous_was_space {
            normalized.push(' ');
            previous_was_space = true;
        }
    }

    normalized
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn is_supported_local_artwork_extension(extension: &str) -> bool {
    LOCAL_ARTWORK_EXTENSIONS
        .iter()
        .any(|value| value.eq_ignore_ascii_case(extension))
}
