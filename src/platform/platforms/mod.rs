use std::path::{Path, PathBuf};
use std::process::Output;

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
mod fallback;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux_x86_64;
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod windows_x86_64;

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
use fallback as implementation;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use linux_x86_64 as implementation;
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
use windows_x86_64 as implementation;

pub(super) fn home_dir() -> Result<PathBuf, String> {
    implementation::home_dir()
}

pub(super) fn app_dir() -> Result<PathBuf, String> {
    implementation::app_dir()
}

pub(super) fn command_exists(command_name: &str) -> bool {
    implementation::command_exists(command_name)
}

pub(super) fn steam_candidate_roots(home: &Path) -> Vec<PathBuf> {
    implementation::steam_candidate_roots(home)
}

pub(super) fn mattmc_launch_script_candidates() -> &'static [&'static str] {
    implementation::mattmc_launch_script_candidates()
}

pub(super) fn mattmc_sync_script_name() -> &'static str {
    implementation::mattmc_sync_script_name()
}

pub(super) fn mattmc_release_zip_suffix() -> &'static str {
    implementation::mattmc_release_zip_suffix()
}

pub(super) fn normalize_script_path(raw_script_path: &str) -> Result<String, String> {
    implementation::normalize_script_path(raw_script_path)
}

pub(super) fn launch_script(script_path: &str) -> Result<(), String> {
    implementation::launch_script(script_path)
}

pub(super) fn launch_script_with_stdin(
    script_path: &str,
    stdin_content: &str,
) -> Result<(), String> {
    implementation::launch_script_with_stdin(script_path, stdin_content)
}

pub(super) fn run_command(command_name: &str, args: &[&str]) -> Result<Output, String> {
    implementation::run_command(command_name, args)
}

pub(super) fn basalt_update_asset_suffix() -> &'static str {
    implementation::basalt_update_asset_suffix()
}

pub(super) fn basalt_update_asset_marker() -> &'static str {
    implementation::basalt_update_asset_marker()
}

pub(super) fn install_basalt_update_and_restart(installer_path: &Path) -> Result<(), String> {
    implementation::install_basalt_update_and_restart(installer_path)
}

pub(super) fn can_install_basalt_updates() -> bool {
    implementation::can_install_basalt_updates()
}
