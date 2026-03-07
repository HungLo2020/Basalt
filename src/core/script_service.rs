use std::path::Path;

use super::error::{CoreError, CoreResult};
use super::registry;
use super::runners;
use super::runners::RunnerKind;

const MATTMC_GAME_NAME: &str = "MattMC";
const MATTMC_SYNC_SCRIPT: &str = "SyncGameData.sh";

pub fn sync_mattmc() -> CoreResult<()> {
    run_game_sibling_script(MATTMC_GAME_NAME, MATTMC_SYNC_SCRIPT)
}

pub fn sync_mattmc_up() -> CoreResult<()> {
    run_game_sibling_script_with_input(MATTMC_GAME_NAME, MATTMC_SYNC_SCRIPT, "up\n")
}

pub fn sync_mattmc_down() -> CoreResult<()> {
    run_game_sibling_script_with_input(MATTMC_GAME_NAME, MATTMC_SYNC_SCRIPT, "down\n")
}

pub fn run_game_sibling_script(game_name: &str, sibling_script_name: &str) -> CoreResult<()> {
    let sibling_script_path = resolve_game_sibling_script_path(game_name, sibling_script_name)?;
    runners::bashrunner::launch(&sibling_script_path)?;
    Ok(())
}

pub fn run_game_sibling_script_with_input(
    game_name: &str,
    sibling_script_name: &str,
    stdin_content: &str,
) -> CoreResult<()> {
    if stdin_content.is_empty() {
        return Err(CoreError::new("Script stdin content cannot be empty"));
    }

    let sibling_script_path = resolve_game_sibling_script_path(game_name, sibling_script_name)?;
    runners::bashrunner::launch_with_stdin(&sibling_script_path, stdin_content)?;
    Ok(())
}

fn resolve_game_sibling_script_path(
    game_name: &str,
    sibling_script_name: &str,
) -> CoreResult<String> {
    if game_name.is_empty() {
        return Err(CoreError::new("Game name cannot be empty"));
    }

    if sibling_script_name.is_empty() {
        return Err(CoreError::new("Script name cannot be empty"));
    }

    let entries = registry::load_entries()?;
    let entry = entries
        .into_iter()
        .find(|game| game.name == game_name)
        .ok_or_else(|| CoreError::new(format!("No game found with name '{}'", game_name)))?;

    if entry.runner_kind != RunnerKind::Bash {
        return Err(CoreError::new(format!(
            "Game '{}' does not use the bash runner, so sibling scripts are not supported",
            game_name
        )));
    }

    let launch_script_path = Path::new(&entry.launch_target);
    if !launch_script_path.exists() || !launch_script_path.is_file() {
        return Err(CoreError::new(format!(
            "Saved script path does not exist or is not a file: {}",
            entry.launch_target
        )));
    }

    let parent_directory = launch_script_path.parent().ok_or_else(|| {
        CoreError::new(format!(
            "Could not determine parent directory for script: {}",
            entry.launch_target
        ))
    })?;

    let sibling_script_path = parent_directory.join(sibling_script_name);
    if !sibling_script_path.exists() || !sibling_script_path.is_file() {
        return Err(CoreError::new(format!(
            "No script found for '{}' at {}",
            game_name,
            sibling_script_path.display()
        )));
    }

    sibling_script_path
        .to_str()
        .ok_or_else(|| CoreError::new("Sibling script path contains invalid UTF-8"))
        .map(|value| value.to_string())
}
