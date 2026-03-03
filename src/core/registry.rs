use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use super::GameEntry;
use super::runners::RunnerKind;

const APP_DIR_NAME: &str = ".basalt";
const REGISTRY_FILE_NAME: &str = "games.tsv";

pub(super) fn get_app_dir() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    Ok(Path::new(&home).join(APP_DIR_NAME))
}

fn get_registry_path() -> Result<PathBuf, String> {
    Ok(get_app_dir()?.join(REGISTRY_FILE_NAME))
}

fn ensure_registry_dir() -> Result<(), String> {
    let app_dir = get_app_dir()?;

    fs::create_dir_all(app_dir)
        .map_err(|err| format!("Failed to create registry directory: {}", err))?;
    Ok(())
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