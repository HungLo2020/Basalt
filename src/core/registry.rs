use std::collections::HashSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use super::GameEntry;
use super::runners::RunnerKind;

const APP_DIR_NAME: &str = ".basalt";
const REGISTRY_FILE_NAME: &str = "games.tsv";
const BLACKLIST_FILE_NAME: &str = "blacklist.txt";

pub(super) fn get_app_dir() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    Ok(Path::new(&home).join(APP_DIR_NAME))
}

fn get_registry_path() -> Result<PathBuf, String> {
    Ok(get_app_dir()?.join(REGISTRY_FILE_NAME))
}

fn get_blacklist_path() -> Result<PathBuf, String> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join(BLACKLIST_FILE_NAME))
}

fn ensure_registry_dir() -> Result<(), String> {
    let app_dir = get_app_dir()?;

    fs::create_dir_all(app_dir)
        .map_err(|err| format!("Failed to create registry directory: {}", err))?;
    Ok(())
}

fn ensure_blacklist_file() -> Result<(), String> {
    let blacklist_path = get_blacklist_path()?;

    let blacklist_parent = blacklist_path
        .parent()
        .ok_or_else(|| "Failed to determine blacklist parent directory".to_string())?;
    fs::create_dir_all(blacklist_parent)
        .map_err(|err| format!("Failed to create blacklist directory: {}", err))?;

    if blacklist_path.exists() {
        return Ok(());
    }

    let default_contents = "# Basalt blacklist\n# One game name per line.\n# Lines starting with # are ignored.\n";
    fs::write(&blacklist_path, default_contents)
        .map_err(|err| format!("Failed to create blacklist file: {}", err))?;

    Ok(())
}

pub(super) fn load_blacklisted_names() -> Result<HashSet<String>, String> {
    ensure_blacklist_file()?;

    let blacklist_path = get_blacklist_path()?;
    let file = fs::File::open(&blacklist_path)
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