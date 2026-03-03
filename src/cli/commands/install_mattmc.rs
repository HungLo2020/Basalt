use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core;

const MATTMC_RELEASES_LATEST_URL: &str = "https://github.com/HungLo2020/MattMC/releases/latest";
const MATTMC_TAG_ARCHIVE_URL_PREFIX: &str = "https://github.com/HungLo2020/MattMC/archive/refs/tags/";

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt install-mattmc".to_string());
    }

    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    let target_dir = Path::new(&home).join("Documents").join("MattMC");

    fs::create_dir_all(&target_dir)
        .map_err(|err| format!("Failed to create MattMC directory: {}", err))?;

    let latest_tag = fetch_latest_release_tag()?;
    let archive_url = format!("{}{}.tar.gz", MATTMC_TAG_ARCHIVE_URL_PREFIX, latest_tag);

    let unix_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("Failed to read system clock: {}", err))?
        .as_secs();
    let temp_archive_path = env::temp_dir().join(format!(
        "basalt-mattmc-release-{}-{}.tar.gz",
        std::process::id(),
        unix_timestamp
    ));

    download_archive(&archive_url, &temp_archive_path)?;
    extract_archive_into_target(&temp_archive_path, &target_dir)?;

    let discover_report = core::discover_with_runners(&[core::DiscoverRunner::Mattmc])?;
    let discover_result_message = match discover_report.mattmc {
        Some(core::DiscoverResult::Added) => "Discovered MattMC and added it.".to_string(),
        Some(core::DiscoverResult::AlreadyExists) => {
            "MattMC game entry already exists; no changes made.".to_string()
        }
        Some(core::DiscoverResult::NotFound) | None => {
            return Err("MattMC install completed, but discovery did not find ~/Documents/MattMC/run-mattmc.sh".to_string())
        }
    };

    if let Err(error) = fs::remove_file(&temp_archive_path) {
        eprintln!(
            "Warning: Failed to remove temporary archive at {}: {}",
            temp_archive_path.display(),
            error
        );
    }

    println!(
        "Installed MattMC release '{}' into {}",
        latest_tag,
        target_dir.display()
    );
    println!("{}", discover_result_message);
    Ok(())
}

fn fetch_latest_release_tag() -> Result<String, String> {
    let output = Command::new("curl")
        .args([
            "-fsSLI",
            "-o",
            "/dev/null",
            "-w",
            "%{url_effective}",
            MATTMC_RELEASES_LATEST_URL,
        ])
        .output()
        .map_err(|err| format!("Failed to run curl for latest MattMC release URL: {}", err))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to resolve latest MattMC release URL: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let effective_url = String::from_utf8(output.stdout)
        .map_err(|_| "Latest release URL output was not valid UTF-8".to_string())?;
    let effective_url = effective_url.trim();

    let (_, encoded_tag) = effective_url
        .rsplit_once("/tag/")
        .ok_or_else(|| format!("Could not parse release tag from URL: {}", effective_url))?;

    if encoded_tag.is_empty() {
        return Err("Latest MattMC release tag is empty".to_string());
    }

    decode_percent_encoding(encoded_tag)
}

fn download_archive(archive_url: &str, destination_path: &Path) -> Result<(), String> {
    let output = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--output"])
        .arg(destination_path)
        .arg(archive_url)
        .output()
        .map_err(|err| format!("Failed to run curl for MattMC archive download: {}", err))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to download MattMC release archive: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

fn extract_archive_into_target(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(target_dir)
        .args(["--strip-components=1", "--overwrite"])
        .output()
        .map_err(|err| format!("Failed to run tar for MattMC extraction: {}", err))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to extract MattMC release archive: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

fn decode_percent_encoding(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(format!(
                    "Invalid percent-encoded release tag '{}': truncated escape sequence",
                    value
                ));
            }

            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).map_err(|_| {
                format!(
                    "Invalid percent-encoded release tag '{}': non-UTF8 escape sequence",
                    value
                )
            })?;

            let byte = u8::from_str_radix(hex, 16).map_err(|_| {
                format!(
                    "Invalid percent-encoded release tag '{}': bad escape '%{}'",
                    value, hex
                )
            })?;

            decoded.push(byte);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(decoded)
        .map_err(|_| format!("Decoded release tag is not valid UTF-8: '{}'", value))
}