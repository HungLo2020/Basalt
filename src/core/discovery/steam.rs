use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::{add_game, is_already_exists_error};

pub fn discover_steam_entries() -> Result<(usize, usize, usize), String> {
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
        let steam_target = format!("steam://rungameid/{}", appid);

        match add_game(&entry_name, &steam_target) {
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
            let dir_entry =
                dir_entry.map_err(|err| format!("Failed to read Steam library entry: {}", err))?;
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
        (Some(appid_value), Some(name_value))
            if !appid_value.is_empty() && !name_value.is_empty() =>
        {
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