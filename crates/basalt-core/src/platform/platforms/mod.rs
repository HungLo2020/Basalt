use std::process::Output;

#[cfg(not(target_os = "linux"))]
compile_error!("Basalt only supports Linux.");

mod linux;

use crate::error::CoreResult;
use linux as implementation;

pub(super) fn command_exists(command_name: &str) -> bool {
    implementation::command_exists(command_name)
}

pub(super) fn mattmc_launch_script_candidates() -> &'static [&'static str] {
    implementation::mattmc_launch_script_candidates()
}

pub(super) fn mattmc_sync_script_name() -> &'static str {
    implementation::mattmc_sync_script_name()
}

pub(super) fn mattmc_update_script_name() -> &'static str {
    implementation::mattmc_update_script_name()
}

pub(super) fn mattmc_release_zip_suffix() -> &'static str {
    implementation::mattmc_release_zip_suffix()
}

pub(super) fn normalize_script_path(raw_script_path: &str) -> CoreResult<String> {
    implementation::normalize_script_path(raw_script_path)
}

pub(super) fn launch_script(script_path: &str) -> CoreResult<()> {
    implementation::launch_script(script_path)
}

pub(super) fn launch_script_with_stdin(script_path: &str, stdin_content: &str) -> CoreResult<()> {
    implementation::launch_script_with_stdin(script_path, stdin_content)
}

pub(super) fn run_command(command_name: &str, args: &[&str]) -> CoreResult<Output> {
    implementation::run_command(command_name, args)
}
