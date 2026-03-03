use std::path::Path;

mod discovery;
mod registry;
mod runners;

use runners::RunnerKind;

#[derive(Clone)]
pub struct GameEntry {
    pub name: String,
    pub runner_kind: RunnerKind,
    pub launch_target: String,
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

    let resolved_target = runners::resolve_add_target(raw_script_path)?;

    let mut entries = registry::load_entries()?;
    if entries.iter().any(|entry| entry.name == name) {
        return Err(format!("A game with name '{}' already exists", name));
    }

    if entries
        .iter()
        .any(|entry| {
            entry.runner_kind == resolved_target.runner_kind
                && entry.launch_target == resolved_target.launch_target
        })
    {
        return Err(format!(
            "A game with target '{}' already exists",
            resolved_target.launch_target
        ));
    }

    entries.push(GameEntry {
        name: name.to_string(),
        runner_kind: resolved_target.runner_kind,
        launch_target: resolved_target.launch_target,
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

    runners::launch(entry.runner_kind, &entry.launch_target)
}

pub fn run_game_sibling_script(game_name: &str, sibling_script_name: &str) -> Result<(), String> {
    if game_name.is_empty() {
        return Err("Game name cannot be empty".to_string());
    }

    if sibling_script_name.is_empty() {
        return Err("Script name cannot be empty".to_string());
    }

    let entries = registry::load_entries()?;
    let entry = entries
        .into_iter()
        .find(|game| game.name == game_name)
        .ok_or_else(|| format!("No game found with name '{}'", game_name))?;

    if entry.runner_kind != RunnerKind::Bash {
        return Err(format!(
            "Game '{}' does not use the bash runner, so sibling scripts are not supported",
            game_name
        ));
    }

    let launch_script_path = Path::new(&entry.launch_target);
    if !launch_script_path.exists() || !launch_script_path.is_file() {
        return Err(format!(
            "Saved script path does not exist or is not a file: {}",
            entry.launch_target
        ));
    }

    let parent_directory = launch_script_path.parent().ok_or_else(|| {
        format!(
            "Could not determine parent directory for script: {}",
            entry.launch_target
        )
    })?;

    let sibling_script_path = parent_directory.join(sibling_script_name);
    if !sibling_script_path.exists() || !sibling_script_path.is_file() {
        return Err(format!(
            "No script found for '{}' at {}",
            game_name,
            sibling_script_path.display()
        ));
    }

    runners::bashrunner::launch(
        sibling_script_path
            .to_str()
            .ok_or_else(|| "Sibling script path contains invalid UTF-8".to_string())?,
    )
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
        || (error_message.starts_with("A game with target '")
            && error_message.ends_with("' already exists"))
}