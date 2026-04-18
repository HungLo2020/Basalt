use std::path::{Path, PathBuf};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux_x86_64;
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod windows_x86_64;
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
mod fallback;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use linux_x86_64 as implementation;
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
use windows_x86_64 as implementation;
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
use fallback as implementation;

pub fn home_dir() -> Result<PathBuf, String> {
    implementation::home_dir()
}

pub fn app_dir() -> Result<PathBuf, String> {
    implementation::app_dir()
}

pub fn command_exists(command_name: &str) -> bool {
    implementation::command_exists(command_name)
}

pub fn steam_candidate_roots(home: &Path) -> Vec<PathBuf> {
    implementation::steam_candidate_roots(home)
}

pub fn mattmc_launch_script_name() -> &'static str {
    implementation::mattmc_launch_script_name()
}
