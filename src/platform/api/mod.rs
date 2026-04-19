use std::path::{Path, PathBuf};
use std::process::Output;

use super::platforms;

pub fn home_dir() -> Result<PathBuf, String> {
    platforms::home_dir()
}

pub fn app_dir() -> Result<PathBuf, String> {
    platforms::app_dir()
}

pub fn command_exists(command_name: &str) -> bool {
    platforms::command_exists(command_name)
}

pub fn steam_candidate_roots(home: &Path) -> Vec<PathBuf> {
    platforms::steam_candidate_roots(home)
}

pub fn mattmc_launch_script_candidates() -> &'static [&'static str] {
    platforms::mattmc_launch_script_candidates()
}

pub fn normalize_script_path(raw_script_path: &str) -> Result<String, String> {
    platforms::normalize_script_path(raw_script_path)
}

pub fn launch_script(script_path: &str) -> Result<(), String> {
    platforms::launch_script(script_path)
}

pub fn launch_script_with_stdin(script_path: &str, stdin_content: &str) -> Result<(), String> {
    platforms::launch_script_with_stdin(script_path, stdin_content)
}

pub fn run_command(command_name: &str, args: &[&str]) -> Result<Output, String> {
    platforms::run_command(command_name, args)
}
