use std::env;
use std::fs;
use std::io::copy;
use std::path::{Path, PathBuf};
use std::process::Command;

const RETROARCH_FLATPAK_APP_ID: &str = "org.libretro.RetroArch";

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

pub fn install_runtime_and_cores() -> Result<EmulationInstallReport, String> {
    ensure_emulator_directories()?;
    let runtime_command = ensure_runtime_command()?;

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
    let append_config_contents = format!(
        "savefile_directory = \"{}\"\nsavestate_directory = \"{}\"\n",
        save_directory_string, save_directory_string
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
