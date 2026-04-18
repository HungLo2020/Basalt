use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use super::GameEntry;
use super::runners::RunnerKind;

const REGISTRY_FILE_NAME: &str = "games.tsv";
const BLACKLIST_FILE_NAME: &str = "blacklist.txt";
const DEFAULT_BLACKLIST_TEMPLATE: &str =
    "# Basalt blacklist\n# One game name per line.\n# Lines starting with # are ignored.\n";

pub(super) fn get_app_dir() -> Result<PathBuf, String> {
    crate::platform::app_dir()
}

fn get_registry_path() -> Result<PathBuf, String> {
    Ok(get_app_dir()?.join(REGISTRY_FILE_NAME))
}

fn get_blacklist_path() -> Result<PathBuf, String> {
    Ok(get_app_dir()?.join(BLACKLIST_FILE_NAME))
}

fn ensure_registry_dir() -> Result<(), String> {
    let app_dir = get_app_dir()?;

    fs::create_dir_all(app_dir)
        .map_err(|err| format!("Failed to create registry directory: {}", err))?;
    Ok(())
}

fn ensure_blacklist_file() -> Result<(), String> {
    ensure_registry_dir()?;
    let blacklist_path = get_blacklist_path()?;

    if blacklist_path.exists() {
        return Ok(());
    }

    let seed_contents = bundled_blacklist_contents();
    fs::write(
        &blacklist_path,
        seed_contents
            .as_deref()
            .unwrap_or(DEFAULT_BLACKLIST_TEMPLATE),
    )
        .map_err(|err| format!("Failed to create blacklist file: {}", err))?;

    Ok(())
}

pub(super) fn load_blacklisted_names() -> Result<HashSet<String>, String> {
    ensure_blacklist_file()?;

    let blacklist_path = get_blacklist_path()?;
    let mut names = read_blacklisted_names_from_file(&blacklist_path)?;
    if !names.is_empty() {
        return Ok(names);
    }

    let existing_contents = fs::read_to_string(&blacklist_path).unwrap_or_default();
    if existing_contents.trim() == DEFAULT_BLACKLIST_TEMPLATE.trim() {
        if let Some(seed_contents) = bundled_blacklist_contents() {
            if has_active_blacklist_entries(&seed_contents) {
                fs::write(&blacklist_path, seed_contents)
                    .map_err(|err| format!("Failed to backfill blacklist file: {}", err))?;
                names = read_blacklisted_names_from_file(&blacklist_path)?;
            }
        }
    }

    Ok(names)
}

fn read_blacklisted_names_from_file(path: &Path) -> Result<HashSet<String>, String> {
    let file = fs::File::open(path)
        .map_err(|err| format!("Failed to open blacklist file: {}", err))?;

    let reader = BufReader::new(file);
    let mut names = HashSet::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|err| format!("Failed to read blacklist file: {}", err))?;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        names.insert(trimmed.to_lowercase());
    }

    Ok(names)
}

fn bundled_blacklist_contents() -> Option<String> {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(BLACKLIST_FILE_NAME),
    )
    .ok()
}

fn has_active_blacklist_entries(contents: &str) -> bool {
    contents
        .lines()
        .map(|line| line.trim())
        .any(|trimmed| !trimmed.is_empty() && !trimmed.starts_with('#'))
}

pub(super) fn load_entries() -> Result<Vec<GameEntry>, String> {
    let registry_path = get_registry_path()?;
    if !registry_path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&registry_path)
        .map_err(|err| format!("Failed to open registry file: {}", err))?;

    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|err| format!("Failed to read registry file: {}", err))?;
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.splitn(3, '\t');
        let name = parts.next().unwrap_or_default().to_string();
        let second = parts.next().unwrap_or_default().to_string();
        let third = parts.next().unwrap_or_default().to_string();

        if !name.is_empty() {
            if !third.is_empty() {
                if let Some(runner_kind) = RunnerKind::from_str(&second) {
                    entries.push(GameEntry {
                        name,
                        runner_kind,
                        launch_target: third,
                    });
                }
            } else if !second.is_empty() {
                entries.push(GameEntry {
                    name,
                    runner_kind: RunnerKind::Bash,
                    launch_target: second,
                });
            }
        }
    }

    Ok(entries)
}

pub(super) fn save_entries(entries: &[GameEntry]) -> Result<(), String> {
    ensure_registry_dir()?;
    let registry_path = get_registry_path()?;

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&registry_path)
        .map_err(|err| format!("Failed to open registry file for writing: {}", err))?;

    for entry in entries {
        let line = format!(
            "{}\t{}\t{}\n",
            entry.name,
            entry.runner_kind.as_str(),
            entry.launch_target
        );
        file.write_all(line.as_bytes())
            .map_err(|err| format!("Failed to write registry file: {}", err))?;
    }

    Ok(())
}