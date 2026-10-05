use std::fs;
use std::path::Path;

use crate::platform;

use super::paths;

const RETROARCH_FLATPAK_APP_ID: &str = "org.libretro.RetroArch";

pub(super) enum RuntimeCommand {
    RetroArchBinary,
    RetroArchFlatpak,
}

pub(super) fn ensure_runtime_command() -> Result<RuntimeCommand, String> {
    if let Some(command) = detect_runtime_command() {
        return Ok(command);
    }

    attempt_runtime_install()?;

    detect_runtime_command().ok_or_else(|| {
        "RetroArch is not available. Automatic installation failed; retry from Install screen or run `basalt install-emulators`.".to_string()
    })
}

pub(super) fn launch_retroarch(
    runtime_command: &RuntimeCommand,
    system: &str,
    rom_path: &Path,
    core_path: &Path,
) -> Result<(), String> {
    let save_directory = paths::saves_root_dir()?.join(system);
    fs::create_dir_all(&save_directory)
        .map_err(|error| format!("Failed to create emulator save directory: {}", error))?;

    let rom_path_string = rom_path
        .to_str()
        .ok_or_else(|| "ROM path contains invalid UTF-8".to_string())?;
    let core_path_string = core_path
        .to_str()
        .ok_or_else(|| "Core path contains invalid UTF-8".to_string())?;
    let append_config_path =
        paths::retroarch_runtime_dir()?.join(format!("basalt-{}-paths.cfg", system));
    let save_directory_string = save_directory
        .to_str()
        .ok_or_else(|| "Save directory path contains invalid UTF-8".to_string())?;
    let autoconfig_directory = paths::retroarch_autoconfig_root_dir()?;
    let autoconfig_directory_string = autoconfig_directory
        .to_str()
        .ok_or_else(|| "Autoconfig path contains invalid UTF-8".to_string())?;
    let append_config_contents = format!(
        "savefile_directory = \"{}\"\nsavestate_directory = \"{}\"\nsavefiles_in_content_dir = \"false\"\nsavestates_in_content_dir = \"false\"\nsort_savefiles_enable = \"false\"\nsort_savestates_enable = \"false\"\nsort_savefiles_by_content_enable = \"false\"\nsort_savestates_by_content_enable = \"false\"\nvideo_fullscreen = \"true\"\ninput_autodetect_enable = \"true\"\njoypad_autoconfig_dir = \"{}\"\n",
        save_directory_string, save_directory_string, autoconfig_directory_string
    );
    fs::write(&append_config_path, append_config_contents)
        .map_err(|error| format!("Failed to write RetroArch append config: {}", error))?;
    let append_config_string = append_config_path
        .to_str()
        .ok_or_else(|| "RetroArch append config path contains invalid UTF-8".to_string())?;

    let mut launch_args = vec![
        "--fullscreen",
        "-L",
        core_path_string,
        "--appendconfig",
        append_config_string,
        rom_path_string,
    ];

    let (command_name, args) = match runtime_command {
        RuntimeCommand::RetroArchBinary => ("retroarch", launch_args),
        RuntimeCommand::RetroArchFlatpak => {
            let mut flatpak_args = vec!["run", RETROARCH_FLATPAK_APP_ID];
            flatpak_args.append(&mut launch_args);
            ("flatpak", flatpak_args)
        }
    };

    let output = platform::run_command(command_name, &args)
        .map_err(|error| format!("Failed to launch emulator runtime: {}", error))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr_text = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let exit_code = output
            .status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string());

        let details = if !stderr_text.is_empty() {
            stderr_text
        } else if !stdout_text.is_empty() {
            stdout_text
        } else {
            "No additional runtime output".to_string()
        };

        Err(format!(
            "Emulator launch failed (exit={}): {}",
            exit_code, details
        ))
    }
}

pub(super) fn run_command(command: &str, args: &[&str]) -> Result<(), String> {
    let output = platform::run_command(command, args)?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Command failed: {} {}\n{}",
            command,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(super) fn command_exists(command_name: &str) -> bool {
    platform::command_exists(command_name)
}

fn detect_runtime_command() -> Option<RuntimeCommand> {
    if command_exists("retroarch") {
        return Some(RuntimeCommand::RetroArchBinary);
    }

    if flatpak_app_is_installed() {
        return Some(RuntimeCommand::RetroArchFlatpak);
    }

    None
}

fn attempt_runtime_install() -> Result<(), String> {
    if command_exists("apt-get") {
        let _ = try_install_with_apt();
        if command_exists("retroarch") {
            return Ok(());
        }
    }

    if command_exists("flatpak") {
        let _ = run_command(
            "flatpak",
            &[
                "remote-add",
                "--if-not-exists",
                "flathub",
                "https://flathub.org/repo/flathub.flatpakrepo",
            ],
        );
        let _ = run_command(
            "flatpak",
            &["install", "-y", "flathub", RETROARCH_FLATPAK_APP_ID],
        );
        if flatpak_app_is_installed() {
            return Ok(());
        }
    }

    Err("Failed to automatically install RetroArch runtime.".to_string())
}

fn try_install_with_apt() -> Result<(), String> {
    if is_root_user() {
        run_command("apt-get", &["update"])?;
        run_command("apt-get", &["install", "-y", "retroarch"])?;
        return Ok(());
    }

    if command_exists("sudo") {
        run_command("sudo", &["-n", "apt-get", "update"])?;
        run_command("sudo", &["-n", "apt-get", "install", "-y", "retroarch"])?;
        return Ok(());
    }

    Err("Apt-based RetroArch installation requires root or passwordless sudo.".to_string())
}

fn flatpak_app_is_installed() -> bool {
    if !command_exists("flatpak") {
        return false;
    }

    platform::run_command("flatpak", &["info", RETROARCH_FLATPAK_APP_ID])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn is_root_user() -> bool {
    platform::run_command("id", &["-u"])
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim() == "0")
        .unwrap_or(false)
}
