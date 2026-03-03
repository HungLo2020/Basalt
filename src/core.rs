use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const APP_DIR_NAME: &str = ".basalt";
const REGISTRY_FILE_NAME: &str = "games.tsv";

#[derive(Clone)]
pub struct GameEntry {
    pub name: String,
    pub script_path: String,
}

pub fn add_game(name: &str, raw_script_path: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Game name cannot be empty".to_string());
    }

    if name.contains('\t') || name.contains('\n') {
        return Err("Game name cannot contain tabs or newlines".to_string());
    }

    let script_path = Path::new(raw_script_path);
    if !script_path.exists() || !script_path.is_file() {
        return Err(format!(
            "Script does not exist or is not a file: {}",
            raw_script_path
        ));
    }

    let has_sh_extension = script_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("sh"))
        .unwrap_or(false);

    if !has_sh_extension {
        return Err("Only bash scripts are supported right now (expected .sh file)".to_string());
    }

    let canonical_script_path = fs::canonicalize(script_path)
        .map_err(|err| format!("Failed to resolve script path: {}", err))?;

    let canonical_script_path_str = canonical_script_path
        .to_str()
        .ok_or_else(|| "Script path contains invalid UTF-8".to_string())?
        .to_string();

    let mut entries = load_entries()?;
    if entries.iter().any(|entry| entry.name == name) {
        return Err(format!("A game with name '{}' already exists", name));
    }

    entries.push(GameEntry {
        name: name.to_string(),
        script_path: canonical_script_path_str,
    });

    save_entries(&entries)
}

pub fn list_games() -> Result<Vec<GameEntry>, String> {
    load_entries()
}

pub fn launch_game(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Game name cannot be empty".to_string());
    }

    let entries = load_entries()?;
    let entry = entries
        .into_iter()
        .find(|game| game.name == name)
        .ok_or_else(|| format!("No game found with name '{}'", name))?;

    let script_path = Path::new(&entry.script_path);
    if !script_path.exists() || !script_path.is_file() {
        return Err(format!(
            "Saved script path does not exist or is not a file: {}",
            entry.script_path
        ));
    }

    let status = Command::new("bash")
        .arg(script_path)
        .status()
        .map_err(|err| format!("Failed to launch script: {}", err))?;

    if !status.success() {
        return Err(format!(
            "Script exited with non-zero status: {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string())
        ));
    }

    Ok(())
}

fn get_registry_path() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    Ok(Path::new(&home).join(APP_DIR_NAME).join(REGISTRY_FILE_NAME))
}

fn ensure_registry_dir() -> Result<(), String> {
    let registry_path = get_registry_path()?;
    let registry_dir = registry_path
        .parent()
        .ok_or_else(|| "Unable to determine registry directory".to_string())?;

    fs::create_dir_all(registry_dir)
        .map_err(|err| format!("Failed to create registry directory: {}", err))?;
    Ok(())
}

fn load_entries() -> Result<Vec<GameEntry>, String> {
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

        let mut parts = line.splitn(2, '\t');
        let name = parts.next().unwrap_or_default().to_string();
        let script_path = parts.next().unwrap_or_default().to_string();

        if !name.is_empty() && !script_path.is_empty() {
            entries.push(GameEntry { name, script_path });
        }
    }

    Ok(entries)
}

fn save_entries(entries: &[GameEntry]) -> Result<(), String> {
    ensure_registry_dir()?;
    let registry_path = get_registry_path()?;

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&registry_path)
        .map_err(|err| format!("Failed to open registry file for writing: {}", err))?;

    for entry in entries {
        let line = format!("{}\t{}\n", entry.name, entry.script_path);
        file.write_all(line.as_bytes())
            .map_err(|err| format!("Failed to write registry file: {}", err))?;
    }

    Ok(())
}
