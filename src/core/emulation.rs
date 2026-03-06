use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{copy, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const RETROARCH_FLATPAK_APP_ID: &str = "org.libretro.RetroArch";
const JOYPAD_AUTOCONFIG_REPO_API_URL: &str =
    "https://api.github.com/repos/libretro/retroarch-joypad-autoconfig/contents";
const REMOTE_ROMS_ROOT_DIR: &str = "/mnt/storage/OneDrive/Apps/Games/Emulators/ROMs";
const REMOTE_SAVES_ROOT_DIR: &str = "/mnt/storage/OneDrive/Apps/Games/Emulators/Saves";

const NES_SYSTEM: &str = "nes";
const GBA_SYSTEM: &str = "gba";

struct CoreSpec {
    system: &'static str,
    core_file: &'static str,
    archive_url: &'static str,
    rom_extensions: &'static [&'static str],
}

const CORE_SPECS: [CoreSpec; 2] = [
    CoreSpec {
        system: NES_SYSTEM,
        core_file: "nestopia_libretro.so",
        archive_url: "https://buildbot.libretro.com/nightly/linux/x86_64/latest/nestopia_libretro.so.zip",
        rom_extensions: &["nes", "fds", "unf", "unif"],
    },
    CoreSpec {
        system: GBA_SYSTEM,
        core_file: "mgba_libretro.so",
        archive_url: "https://buildbot.libretro.com/nightly/linux/x86_64/latest/mgba_libretro.so.zip",
        rom_extensions: &["gba"],
    },
];

enum RuntimeCommand {
    RetroArchBinary,
    RetroArchFlatpak,
}

pub struct EmulationInstallReport {
    pub runtime_ready: bool,
    pub cores_ready: usize,
}

pub struct RomSyncReport {
    pub copied: usize,
    pub unchanged: usize,
    pub deleted: usize,
}

#[derive(Clone, Copy)]
enum RomSyncDirection {
    Up,
    Down,
}

pub fn install_runtime_and_cores() -> Result<EmulationInstallReport, String> {
    ensure_emulator_directories()?;
    let runtime_command = ensure_runtime_command()?;

    let _ = ensure_xbox_autoconfig_profiles();

    let mut cores_ready = 0usize;
    for core_spec in CORE_SPECS {
        if ensure_core_installed(core_spec, &runtime_command)?.exists() {
            cores_ready += 1;
        }
    }

    Ok(EmulationInstallReport {
        runtime_ready: true,
        cores_ready,
    })
}

pub fn install_core_for_system(system: &str) -> Result<(), String> {
    ensure_emulator_directories()?;
    let runtime_command = ensure_runtime_command()?;
    let core_spec = core_spec_for_system(system)
        .ok_or_else(|| format!("Unsupported emulator system: {}", system))?;
    ensure_core_installed(core_spec, &runtime_command)?;
    Ok(())
}

pub fn is_core_installed_for_system(system: &str) -> Result<bool, String> {
    let core_spec = core_spec_for_system(system)
        .ok_or_else(|| format!("Unsupported emulator system: {}", system))?;
    let core_path = retroarch_cores_dir()?.join(core_spec.core_file);
    Ok(core_path.exists() && core_path.is_file())
}

pub fn sync_roms_up_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync_roms_for_system(system, RomSyncDirection::Up)
}

pub fn sync_roms_down_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync_roms_for_system(system, RomSyncDirection::Down)
}

pub fn sync_saves_up_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync_saves_for_system(system, RomSyncDirection::Up)
}

pub fn sync_saves_down_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync_saves_for_system(system, RomSyncDirection::Down)
}

pub fn discoverable_systems() -> &'static [&'static str] {
    &[NES_SYSTEM, GBA_SYSTEM]
}

pub fn is_supported_rom_for_system(system: &str, file_path: &Path) -> bool {
    let Some(extension) = file_path.extension().and_then(|value| value.to_str()) else {
        return false;
    };

    let normalized_extension = extension.to_lowercase();
    core_spec_for_system(system)
        .map(|core_spec| {
            core_spec
                .rom_extensions
                .iter()
                .any(|expected| *expected == normalized_extension)
        })
        .unwrap_or(false)
}

pub fn roms_root_dir() -> Result<PathBuf, String> {
    Ok(emulators_root_dir()?.join("roms"))
}

pub fn saves_root_dir() -> Result<PathBuf, String> {
    Ok(emulators_root_dir()?.join("saves"))
}

pub fn ensure_emulator_directories() -> Result<(), String> {
    let root = emulators_root_dir()?;
    let roms_root = root.join("roms");
    let saves_root = root.join("saves");
    let runtime_root = root.join("runtime").join("retroarch");
    let cores_root = runtime_root.join("cores");

    fs::create_dir_all(&roms_root)
        .map_err(|error| format!("Failed to create emulator ROM root: {}", error))?;
    fs::create_dir_all(&saves_root)
        .map_err(|error| format!("Failed to create emulator save root: {}", error))?;
    fs::create_dir_all(&cores_root)
        .map_err(|error| format!("Failed to create RetroArch core directory: {}", error))?;

    for system in discoverable_systems() {
        fs::create_dir_all(roms_root.join(system))
            .map_err(|error| format!("Failed to create ROM directory for {}: {}", system, error))?;
        fs::create_dir_all(saves_root.join(system)).map_err(|error| {
            format!(
                "Failed to create save directory for {}: {}",
                system, error
            )
        })?;
    }

    Ok(())
}

pub fn build_launch_target(system: &str, rom_path: &Path) -> Result<String, String> {
    if core_spec_for_system(system).is_none() {
        return Err(format!("Unsupported emulator system: {}", system));
    }

    let canonical_rom_path = canonicalize_or_keep(rom_path);
    let rom_path_string = canonical_rom_path
        .to_str()
        .ok_or_else(|| "ROM path contains invalid UTF-8".to_string())?;

    Ok(format!("retroarch|{}|{}", system, rom_path_string))
}

pub fn launch_target(launch_target: &str) -> Result<(), String> {
    ensure_emulator_directories()?;
    let runtime_command = ensure_runtime_command()?;
    let _ = ensure_xbox_autoconfig_profiles();
    let (system, rom_path) = parse_launch_target(launch_target)?;

    if !rom_path.exists() || !rom_path.is_file() {
        return Err(format!("ROM file does not exist: {}", rom_path.display()));
    }

    if !is_supported_rom_for_system(&system, &rom_path) {
        return Err(format!(
            "ROM extension is not supported for system '{}': {}",
            system,
            rom_path.display()
        ));
    }

    let core_spec = core_spec_for_system(&system)
        .ok_or_else(|| format!("Unsupported emulator system: {}", system))?;
    let core_path = ensure_core_installed(core_spec, &runtime_command)?;

    let save_directory = saves_root_dir()?.join(&system);
    fs::create_dir_all(&save_directory)
        .map_err(|error| format!("Failed to create emulator save directory: {}", error))?;

    let rom_path_string = rom_path
        .to_str()
        .ok_or_else(|| "ROM path contains invalid UTF-8".to_string())?;
    let core_path_string = core_path
        .to_str()
        .ok_or_else(|| "Core path contains invalid UTF-8".to_string())?;
    let append_config_path = retroarch_runtime_dir()?.join(format!("basalt-{}-paths.cfg", system));
    let save_directory_string = save_directory
        .to_str()
        .ok_or_else(|| "Save directory path contains invalid UTF-8".to_string())?;
    let autoconfig_directory = retroarch_autoconfig_root_dir()?;
    let autoconfig_directory_string = autoconfig_directory
        .to_str()
        .ok_or_else(|| "Autoconfig path contains invalid UTF-8".to_string())?;
    let append_config_contents = format!(
        "savefile_directory = \"{}\"\nsavestate_directory = \"{}\"\nsavefiles_in_content_dir = \"false\"\nsavestates_in_content_dir = \"false\"\nsort_savefiles_enable = \"false\"\nsort_savestates_enable = \"false\"\nsort_savefiles_by_content_enable = \"false\"\nsort_savestates_by_content_enable = \"false\"\ninput_autodetect_enable = \"true\"\njoypad_autoconfig_dir = \"{}\"\n",
        save_directory_string, save_directory_string, autoconfig_directory_string
    );
    fs::write(&append_config_path, append_config_contents)
        .map_err(|error| format!("Failed to write RetroArch append config: {}", error))?;
    let append_config_string = append_config_path
        .to_str()
        .ok_or_else(|| "RetroArch append config path contains invalid UTF-8".to_string())?;

    let mut command = match runtime_command {
        RuntimeCommand::RetroArchBinary => Command::new("retroarch"),
        RuntimeCommand::RetroArchFlatpak => {
            let mut flatpak_command = Command::new("flatpak");
            flatpak_command.args(["run", RETROARCH_FLATPAK_APP_ID]);
            flatpak_command
        }
    };

    let output = command
        .args([
            "-L",
            core_path_string,
            "--appendconfig",
            append_config_string,
            rom_path_string,
        ])
        .output()
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

fn parse_launch_target(launch_target: &str) -> Result<(String, PathBuf), String> {
    let mut parts = launch_target.splitn(3, '|');
    let backend = parts.next().unwrap_or_default();
    let system = parts.next().unwrap_or_default();
    let rom_path = parts.next().unwrap_or_default();

    if backend != "retroarch" {
        return Err(format!("Unsupported emulator backend '{}'.", backend));
    }

    if system.is_empty() || rom_path.is_empty() {
        return Err("Malformed emulator launch target.".to_string());
    }

    Ok((system.to_string(), PathBuf::from(rom_path)))
}

fn ensure_runtime_command() -> Result<RuntimeCommand, String> {
    if let Some(command) = detect_runtime_command() {
        return Ok(command);
    }

    attempt_runtime_install()?;

    detect_runtime_command().ok_or_else(|| {
        "RetroArch is not available. Automatic installation failed; retry from Install screen or run `basalt install-emulators`.".to_string()
    })
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
        let _ = run_command("flatpak", &["remote-add", "--if-not-exists", "flathub", "https://flathub.org/repo/flathub.flatpakrepo"]);
        let _ = run_command("flatpak", &["install", "-y", "flathub", RETROARCH_FLATPAK_APP_ID]);
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

fn ensure_core_installed(core_spec: CoreSpec, _runtime_command: &RuntimeCommand) -> Result<PathBuf, String> {
    let core_path = retroarch_cores_dir()?.join(core_spec.core_file);
    if core_path.exists() {
        return Ok(core_path);
    }

    if !command_exists("unzip") {
        return Err("unzip is required to install RetroArch cores automatically.".to_string());
    }

    let archive_name = format!("{}.zip", core_spec.core_file);
    let archive_path = retroarch_runtime_dir()?.join(archive_name);
    download_file(core_spec.archive_url, &archive_path)?;

    let archive_string = archive_path
        .to_str()
        .ok_or_else(|| "Core archive path contains invalid UTF-8".to_string())?;
    let cores_dir_string = retroarch_cores_dir()?
        .to_str()
        .ok_or_else(|| "Core directory path contains invalid UTF-8".to_string())?
        .to_string();

    run_command("unzip", &["-o", archive_string, "-d", &cores_dir_string])?;

    if let Err(error) = fs::remove_file(&archive_path) {
        eprintln!(
            "Warning: Failed to remove temporary core archive at {}: {}",
            archive_path.display(),
            error
        );
    }

    if core_path.exists() {
        Ok(core_path)
    } else {
        Err(format!(
            "Core installation did not produce expected file: {}",
            core_path.display()
        ))
    }
}

fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    let response = ureq::get(url)
        .set("User-Agent", "Basalt-Emulation-Installer")
        .call()
        .map_err(|error| format!("Failed to download {}: {}", url, error))?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create download directory: {}", error))?;
    }

    let mut reader = response.into_reader();
    let mut file = fs::File::create(destination)
        .map_err(|error| format!("Failed to create file {}: {}", destination.display(), error))?;
    copy(&mut reader, &mut file)
        .map_err(|error| format!("Failed to save {}: {}", destination.display(), error))?;

    Ok(())
}

fn run_command(command: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| format!("Failed to execute {}: {}", command, error))?;

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

fn command_exists(command_name: &str) -> bool {
    let Some(path_value) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path_value).any(|directory| {
        let candidate = directory.join(command_name);
        candidate.exists() && candidate.is_file()
    })
}

fn flatpak_app_is_installed() -> bool {
    if !command_exists("flatpak") {
        return false;
    }

    Command::new("flatpak")
        .args(["info", RETROARCH_FLATPAK_APP_ID])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn is_root_user() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim() == "0")
        .unwrap_or(false)
}

fn emulators_root_dir() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    Ok(Path::new(&home).join("Games").join("Emulators"))
}

fn retroarch_runtime_dir() -> Result<PathBuf, String> {
    Ok(emulators_root_dir()?.join("runtime").join("retroarch"))
}

fn retroarch_cores_dir() -> Result<PathBuf, String> {
    Ok(retroarch_runtime_dir()?.join("cores"))
}

fn retroarch_autoconfig_root_dir() -> Result<PathBuf, String> {
    Ok(retroarch_runtime_dir()?.join("autoconfig"))
}

fn core_spec_for_system(system: &str) -> Option<CoreSpec> {
    let normalized_system = system.to_lowercase();
    CORE_SPECS
        .iter()
        .find(|core_spec| core_spec.system == normalized_system)
        .map(|core_spec| CoreSpec {
            system: core_spec.system,
            core_file: core_spec.core_file,
            archive_url: core_spec.archive_url,
            rom_extensions: core_spec.rom_extensions,
        })
}

fn canonicalize_or_keep(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn sync_roms_for_system(system: &str, direction: RomSyncDirection) -> Result<RomSyncReport, String> {
    let system_key = normalize_system_key(system)?;

    let local_dir = roms_root_dir()?.join(&system_key);
    let remote_dir = Path::new(REMOTE_ROMS_ROOT_DIR).join(&system_key);

    let (source_dir, destination_dir) = match direction {
        RomSyncDirection::Up => (local_dir, remote_dir),
        RomSyncDirection::Down => (remote_dir, local_dir),
    };

    if !source_dir.exists() || !source_dir.is_dir() {
        return Err(format!(
            "Source ROM directory does not exist: {}",
            source_dir.display()
        ));
    }

    sync_directory_contents(
        &source_dir,
        &destination_dir,
        "ROM",
        |_: &Path| true,
    )
}

fn sync_saves_for_system(system: &str, direction: RomSyncDirection) -> Result<RomSyncReport, String> {
    let system_key = normalize_system_key(system)?;

    let local_dir = saves_root_dir()?.join(&system_key);
    let remote_dir = Path::new(REMOTE_SAVES_ROOT_DIR).join(&system_key);

    fs::create_dir_all(&local_dir).map_err(|error| {
        format!(
            "Failed to create local save directory {}: {}",
            local_dir.display(),
            error
        )
    })?;

    fs::create_dir_all(&remote_dir).map_err(|error| {
        format!(
            "Failed to create remote save directory {}: {}",
            remote_dir.display(),
            error
        )
    })?;

    let (source_dir, destination_dir) = match direction {
        RomSyncDirection::Up => (local_dir, remote_dir),
        RomSyncDirection::Down => (remote_dir, local_dir),
    };

    sync_directory_contents(
        &source_dir,
        &destination_dir,
        "save",
        is_syncable_save_file,
    )
}

fn sync_directory_contents<F>(
    source_dir: &Path,
    destination_dir: &Path,
    file_label: &str,
    include_file: F,
) -> Result<RomSyncReport, String>
where
    F: Fn(&Path) -> bool,
{

    fs::create_dir_all(&destination_dir).map_err(|error| {
        format!(
            "Failed to create destination {} directory {}: {}",
            file_label,
            destination_dir.display(),
            error
        )
    })?;

    let mut source_files = Vec::new();
    collect_files_recursive(&source_dir, &mut source_files)?;
    source_files.retain(|path| include_file(path));

    let mut destination_files = Vec::new();
    collect_files_recursive(&destination_dir, &mut destination_files)?;
    destination_files.retain(|path| include_file(path));

    let mut source_relative_paths = HashSet::new();

    let mut copied = 0usize;
    let mut unchanged = 0usize;
    let mut deleted = 0usize;

    for source_file in source_files {
        let relative_path = source_file.strip_prefix(&source_dir).map_err(|error| {
            format!(
                "Failed to compute relative {} path for {}: {}",
                file_label,
                source_file.display(),
                error
            )
        })?;

        source_relative_paths.insert(relative_path.to_path_buf());

        let destination_file = destination_dir.join(relative_path);
        if destination_file.exists() && destination_file.is_dir() {
            fs::remove_dir_all(&destination_file).map_err(|error| {
                format!(
                    "Failed to remove conflicting destination {} directory {}: {}",
                    file_label,
                    destination_file.display(),
                    error
                )
            })?;
        }

        if let Some(parent_dir) = destination_file.parent() {
            fs::create_dir_all(parent_dir).map_err(|error| {
                format!(
                    "Failed to create destination {} subdirectory {}: {}",
                    file_label,
                    parent_dir.display(),
                    error
                )
            })?;
        }

        let should_copy = if destination_file.exists() && destination_file.is_file() {
            !files_identical(&source_file, &destination_file)?
        } else {
            true
        };

        if should_copy {
            fs::copy(&source_file, &destination_file).map_err(|error| {
                format!(
                    "Failed to copy {} {} -> {}: {}",
                    file_label,
                    source_file.display(),
                    destination_file.display(),
                    error
                )
            })?;
            copied += 1;
        } else {
            unchanged += 1;
        }
    }

    for destination_file in destination_files {
        let relative_path = destination_file
            .strip_prefix(&destination_dir)
            .map_err(|error| {
                format!(
                    "Failed to compute destination relative {} path for {}: {}",
                    file_label,
                    destination_file.display(),
                    error
                )
            })?;

        if source_relative_paths.contains(relative_path) {
            continue;
        }

        if destination_file.exists() && destination_file.is_file() {
            fs::remove_file(&destination_file).map_err(|error| {
                format!(
                    "Failed to delete destination {} {}: {}",
                    file_label,
                    destination_file.display(),
                    error
                )
            })?;
            deleted += 1;
        }
    }

    remove_empty_subdirectories(&destination_dir)?;

    Ok(RomSyncReport {
        copied,
        unchanged,
        deleted,
    })
}

fn is_syncable_save_file(file_path: &Path) -> bool {
    let Some(file_name) = file_path.file_name().and_then(|value| value.to_str()) else {
        return true;
    };

    !file_name.to_lowercase().contains(".state")
}

fn normalize_system_key(system: &str) -> Result<String, String> {
    let normalized = system.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("System key cannot be empty".to_string());
    }

    if normalized
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        Ok(normalized)
    } else {
        Err(format!(
            "System key '{}' contains invalid characters",
            system
        ))
    }
}

fn collect_files_recursive(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Failed to read ROM directory {}: {}", root.display(), error))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Failed to read ROM directory entry: {}", error))?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }

    Ok(())
}

fn remove_empty_subdirectories(root: &Path) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Failed to read ROM directory {}: {}", root.display(), error))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Failed to read ROM directory entry: {}", error))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        remove_empty_subdirectories(&path)?;

        let mut sub_entries = fs::read_dir(&path).map_err(|error| {
            format!(
                "Failed to read ROM subdirectory {}: {}",
                path.display(),
                error
            )
        })?;

        if sub_entries.next().is_none() {
            fs::remove_dir(&path).map_err(|error| {
                format!(
                    "Failed to remove empty ROM subdirectory {}: {}",
                    path.display(),
                    error
                )
            })?;
        }
    }

    Ok(())
}

fn files_identical(first: &Path, second: &Path) -> Result<bool, String> {
    let first_metadata = fs::metadata(first)
        .map_err(|error| format!("Failed to stat {}: {}", first.display(), error))?;
    let second_metadata = fs::metadata(second)
        .map_err(|error| format!("Failed to stat {}: {}", second.display(), error))?;

    if first_metadata.len() != second_metadata.len() {
        return Ok(false);
    }

    let first_file = fs::File::open(first)
        .map_err(|error| format!("Failed to open {}: {}", first.display(), error))?;
    let second_file = fs::File::open(second)
        .map_err(|error| format!("Failed to open {}: {}", second.display(), error))?;

    let mut first_reader = BufReader::new(first_file);
    let mut second_reader = BufReader::new(second_file);

    let mut first_buffer = [0u8; 8192];
    let mut second_buffer = [0u8; 8192];

    loop {
        let first_read = first_reader
            .read(&mut first_buffer)
            .map_err(|error| format!("Failed to read {}: {}", first.display(), error))?;
        let second_read = second_reader
            .read(&mut second_buffer)
            .map_err(|error| format!("Failed to read {}: {}", second.display(), error))?;

        if first_read != second_read {
            return Ok(false);
        }

        if first_read == 0 {
            return Ok(true);
        }

        if first_buffer[..first_read] != second_buffer[..second_read] {
            return Ok(false);
        }
    }
}

fn ensure_xbox_autoconfig_profiles() -> Result<(), String> {
    for backend in ["udev", "sdl2"] {
        sync_xbox_autoconfig_backend(backend)?;
    }

    Ok(())
}

fn sync_xbox_autoconfig_backend(backend: &str) -> Result<(), String> {
    let backend_url = format!("{}/{}", JOYPAD_AUTOCONFIG_REPO_API_URL, backend);
    let response = ureq::get(&backend_url)
        .set("User-Agent", "Basalt-Emulation-Installer")
        .call()
        .map_err(|error| format!("Failed to fetch joypad profile list: {}", error))?;

    let payload = response
        .into_string()
        .map_err(|error| format!("Failed to read joypad profile list payload: {}", error))?;
    let listing: Value = serde_json::from_str(&payload)
        .map_err(|error| format!("Failed to parse joypad profile list: {}", error))?;

    let entries = listing
        .as_array()
        .ok_or_else(|| "Joypad profile listing has unexpected format".to_string())?;

    let backend_dir = retroarch_autoconfig_root_dir()?.join(backend);
    fs::create_dir_all(&backend_dir)
        .map_err(|error| format!("Failed to create autoconfig directory: {}", error))?;

    for entry in entries {
        let Some(name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !is_xbox_profile_name(name) {
            continue;
        }

        let Some(download_url) = entry.get("download_url").and_then(Value::as_str) else {
            continue;
        };

        let destination = backend_dir.join(name);
        if destination.exists() {
            continue;
        }

        if let Err(error) = download_file(download_url, &destination) {
            eprintln!(
                "Warning: Failed to download controller profile {}: {}",
                name, error
            );
        }
    }

    Ok(())
}

fn is_xbox_profile_name(name: &str) -> bool {
    let normalized = name.to_lowercase();
    normalized.contains("xbox") || normalized.contains("x-box") || normalized.contains("microsoft")
}
