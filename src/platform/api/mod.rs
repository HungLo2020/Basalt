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

pub fn mattmc_sync_script_name() -> &'static str {
    platforms::mattmc_sync_script_name()
}

pub fn mattmc_update_script_name() -> &'static str {
    platforms::mattmc_update_script_name()
}

pub fn mattmc_release_zip_suffix() -> &'static str {
    platforms::mattmc_release_zip_suffix()
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

pub fn basalt_update_asset_suffix() -> &'static str {
    platforms::basalt_update_asset_suffix()
}

pub fn basalt_update_asset_marker() -> &'static str {
    platforms::basalt_update_asset_marker()
}

pub fn install_basalt_update_and_restart(installer_path: &Path) -> Result<(), String> {
    platforms::install_basalt_update_and_restart(installer_path)
}

pub fn can_install_basalt_updates() -> bool {
    platforms::can_install_basalt_updates()
}
