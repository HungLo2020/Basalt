use std::fs;
use std::path::Path;
use std::process::Command;

mod discovery;
mod registry;

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

    let mut entries = registry::load_entries()?;
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

    registry::save_entries(&entries)
}

pub fn list_games() -> Result<Vec<GameEntry>, String> {
    registry::load_entries()
}

pub fn remove_game(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Game name cannot be empty".to_string());
    }

    let mut entries = registry::load_entries()?;
    let original_len = entries.len();

    entries.retain(|entry| entry.name != name);

    if entries.len() == original_len {
        return Err(format!("No game found with name '{}'", name));
    }

    registry::save_entries(&entries)
}

pub fn remove_all_games() -> Result<usize, String> {
    let entries = registry::load_entries()?;
    let removed_count = entries.len();

    registry::save_entries(&[])?;

    let discovered_steam_dir = registry::get_app_dir()?.join("discovered").join("steam");
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

    let entries = registry::load_entries()?;
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
    let mattmc = discovery::mattmc::discover_mattmc_entry()?;
    let (steam_found, steam_added, steam_already_exists) = discovery::steam::discover_steam_entries()?;

    Ok(DiscoverReport {
        mattmc,
        steam_found,
        steam_added,
        steam_already_exists,
    })
}

pub(crate) fn is_already_exists_error(error_message: &str) -> bool {
    (error_message.starts_with("A game with name '") && error_message.ends_with("' already exists"))
        || (error_message.starts_with("A game with script '")
            && error_message.ends_with("' already exists"))
}