use std::collections::HashSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const APP_DIR_NAME: &str = ".basalt";
const REGISTRY_FILE_NAME: &str = "games.tsv";
const MATTMC_ENTRY_NAME: &str = "MattMC";

#[derive(Clone)]
pub struct GameEntry {
    pub name: String,
    pub script_path: String,
}

pub enum DiscoverResult {
    Added,
    AlreadyExists,
    NotFound,
}

pub struct DiscoverReport {
    pub mattmc: DiscoverResult,
    pub steam_found: usize,
    pub steam_added: usize,
    pub steam_already_exists: usize,
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

    if entries
        .iter()
        .any(|entry| entry.script_path == canonical_script_path_str)
    {
        return Err(format!(
            "A game with script '{}' already exists",
            canonical_script_path_str
        ));
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

pub fn remove_game(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Game name cannot be empty".to_string());
    }

    let mut entries = load_entries()?;
    let original_len = entries.len();

    entries.retain(|entry| entry.name != name);

    if entries.len() == original_len {
        return Err(format!("No game found with name '{}'", name));
    }

    save_entries(&entries)
}

pub fn remove_all_games() -> Result<usize, String> {
    let entries = load_entries()?;
    let removed_count = entries.len();

    save_entries(&[])?;

    let discovered_steam_dir = get_app_dir()?.join("discovered").join("steam");
    if discovered_steam_dir.exists() {
        fs::remove_dir_all(&discovered_steam_dir)
            .map_err(|err| format!("Failed to clean discovered Steam scripts: {}", err))?;
    }

    Ok(removed_count)
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

pub fn discover_games() -> Result<DiscoverReport, String> {
    let mattmc = discover_mattmc_entry()?;
    let (steam_found, steam_added, steam_already_exists) = discover_steam_entries()?;

    Ok(DiscoverReport {
        mattmc,
        steam_found,
        steam_added,
        steam_already_exists,
    })
}

fn discover_mattmc_entry() -> Result<DiscoverResult, String> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    let mattmc_script = Path::new(&home)
        .join("Documents")
        .join("MattMC")
        .join("run-mattmc.sh");

    if !mattmc_script.exists() || !mattmc_script.is_file() {
        return Ok(DiscoverResult::NotFound);
    }

    let mattmc_script_str = mattmc_script
        .to_str()
        .ok_or_else(|| "MattMC script path contains invalid UTF-8".to_string())?;

    match add_game(MATTMC_ENTRY_NAME, mattmc_script_str) {
        Ok(_) => Ok(DiscoverResult::Added),
        Err(err) if is_already_exists_error(&err) => Ok(DiscoverResult::AlreadyExists),
        Err(err) => Err(err),
    }
}

fn discover_steam_entries() -> Result<(usize, usize, usize), String> {
    let manifest_paths = collect_steam_manifest_paths()?;

    let mut steam_found = 0usize;
    let mut steam_added = 0usize;
    let mut steam_already_exists = 0usize;

    for manifest_path in manifest_paths {
        let Some((appid, name)) = parse_steam_manifest(&manifest_path)? else {
            continue;
        };

        steam_found += 1;

        let entry_name = name;
        let script_path = ensure_steam_launch_script(&appid)?;

        let script_path_str = script_path
            .to_str()
            .ok_or_else(|| "Steam script path contains invalid UTF-8".to_string())?
            .to_string();

        match add_game(&entry_name, &script_path_str) {
            Ok(_) => {
                steam_added += 1;
            }
            Err(err) if is_already_exists_error(&err) => {
                steam_already_exists += 1;
            }
            Err(err) => return Err(err),
        }
    }

    Ok((steam_found, steam_added, steam_already_exists))
}

fn is_already_exists_error(error_message: &str) -> bool {
    (error_message.starts_with("A game with name '") && error_message.ends_with("' already exists"))
        || (error_message.starts_with("A game with script '")
            && error_message.ends_with("' already exists"))
}

fn collect_steam_manifest_paths() -> Result<Vec<PathBuf>, String> {
    let library_paths = discover_steam_library_paths()?;
    let mut manifests = Vec::new();

    for library_path in library_paths {
        let steamapps_path = library_path.join("steamapps");
        if !steamapps_path.exists() || !steamapps_path.is_dir() {
            continue;
        }

        let dir_entries = fs::read_dir(&steamapps_path)
            .map_err(|err| format!("Failed to read Steam library directory: {}", err))?;

        for dir_entry in dir_entries {
            let dir_entry = dir_entry
                .map_err(|err| format!("Failed to read Steam library entry: {}", err))?;
            let path = dir_entry.path();

            if !path.is_file() {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            if file_name.starts_with("appmanifest_") && file_name.ends_with(".acf") {
                manifests.push(path);
            }
        }
    }

    Ok(manifests)
}

fn discover_steam_library_paths() -> Result<Vec<PathBuf>, String> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    let home_path = Path::new(&home);

    let candidate_roots = [
        home_path.join(".local").join("share").join("Steam"),
        home_path.join(".steam").join("steam"),
        home_path
            .join(".var")
            .join("app")
            .join("com.valvesoftware.Steam")
            .join(".local")
            .join("share")
            .join("Steam"),
    ];

    let mut discovered = HashSet::new();

    for steam_root in candidate_roots {
        if !steam_root.exists() || !steam_root.is_dir() {
            continue;
        }

        discovered.insert(steam_root.clone());

        let library_folders_path = steam_root.join("steamapps").join("libraryfolders.vdf");
        if !library_folders_path.exists() || !library_folders_path.is_file() {
            continue;
        }

        for library_path in parse_steam_libraryfolders_vdf(&library_folders_path)? {
            discovered.insert(library_path);
        }
    }

    Ok(discovered.into_iter().collect())
}

fn parse_steam_libraryfolders_vdf(path: &Path) -> Result<Vec<PathBuf>, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read Steam libraryfolders.vdf: {}", err))?;

    let mut libraries = Vec::new();

    for line in contents.lines() {
        if let Some(raw_value) = extract_vdf_value(line, "path") {
            let normalized = raw_value.replace("\\\\", "\\");
            libraries.push(PathBuf::from(normalized));
        }
    }

    Ok(libraries)
}

fn parse_steam_manifest(path: &Path) -> Result<Option<(String, String)>, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read Steam app manifest: {}", err))?;

    let mut appid: Option<String> = None;
    let mut name: Option<String> = None;

    for line in contents.lines() {
        if appid.is_none() {
            appid = extract_vdf_value(line, "appid");
        }

        if name.is_none() {
            name = extract_vdf_value(line, "name");
        }

        if appid.is_some() && name.is_some() {
            break;
        }
    }

    Ok(match (appid, name) {
        (Some(appid_value), Some(name_value)) if !appid_value.is_empty() && !name_value.is_empty() => {
            Some((appid_value, name_value))
        }
        _ => None,
    })
}

fn extract_vdf_value(line: &str, key: &str) -> Option<String> {
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 4 && parts[1] == key {
        Some(parts[3].to_string())
    } else {
        None
    }
}

fn ensure_steam_launch_script(appid: &str) -> Result<PathBuf, String> {
    let app_dir = get_app_dir()?;
    let scripts_dir = app_dir.join("discovered").join("steam");

    fs::create_dir_all(&scripts_dir)
        .map_err(|err| format!("Failed to create Steam scripts directory: {}", err))?;

    let script_path = scripts_dir.join(format!("steam_{}.sh", appid));
    let script_contents = format!(
        "#!/usr/bin/env bash\nset -euo pipefail\n\nif command -v steam >/dev/null 2>&1; then\n  steam -applaunch \"{}\"\nelif command -v flatpak >/dev/null 2>&1 && flatpak info com.valvesoftware.Steam >/dev/null 2>&1; then\n  flatpak run com.valvesoftware.Steam -applaunch \"{}\"\nelse\n  echo \"Steam is not installed or not on PATH.\" >&2\n  exit 1\nfi\n",
        appid, appid
    );

    fs::write(&script_path, script_contents)
        .map_err(|err| format!("Failed to write Steam launch script: {}", err))?;

    fs::canonicalize(&script_path)
        .map_err(|err| format!("Failed to resolve Steam launch script path: {}", err))
}

fn get_app_dir() -> Result<PathBuf, String> {
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
