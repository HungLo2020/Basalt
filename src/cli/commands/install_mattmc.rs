use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::core;

const MATTMC_RELEASES_API_LATEST_URL: &str =
    "https://api.github.com/repos/HungLo2020/MattMC/releases/latest";

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt install-mattmc".to_string());
    }

    let home = env::var("HOME").map_err(|_| "HOME environment variable is not set".to_string())?;
    let games_dir = Path::new(&home).join("Games");
    let target_dir = games_dir.join("MattMC");

    fs::create_dir_all(&games_dir)
        .map_err(|err| format!("Failed to create Games directory: {}", err))?;

    fs::create_dir_all(&target_dir)
        .map_err(|err| format!("Failed to create MattMC directory: {}", err))?;

    let (latest_tag, archive_url) = fetch_latest_release_tag_and_client_zip_url()?;

    let unix_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("Failed to read system clock: {}", err))?
        .as_secs();
    let temp_archive_path = env::temp_dir().join(format!(
        "basalt-mattmc-release-{}-{}.zip",
        std::process::id(),
        unix_timestamp
    ));

    download_archive(&archive_url, &temp_archive_path)?;
    extract_zip_into_target(&temp_archive_path, &target_dir)?;

    let discover_report = core::discover_with_runners(&[core::DiscoverRunner::Mattmc])?;
    let discover_result_message = match discover_report.mattmc {
        Some(core::DiscoverResult::Added) => "Discovered MattMC and added it.".to_string(),
        Some(core::DiscoverResult::AlreadyExists) => {
            "MattMC game entry already exists; no changes made.".to_string()
        }
        Some(core::DiscoverResult::NotFound) | None => {
            return Err("MattMC install completed, but discovery did not find ~/Games/MattMC/run-mattmc.sh".to_string())
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

fn fetch_latest_release_tag_and_client_zip_url() -> Result<(String, String), String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: Basalt-MattMC-Installer",
            MATTMC_RELEASES_API_LATEST_URL,
        ])
        .output()
        .map_err(|err| format!("Failed to run curl for latest MattMC release metadata: {}", err))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to fetch latest MattMC release metadata: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let payload = String::from_utf8(output.stdout)
        .map_err(|_| "Latest release metadata output was not valid UTF-8".to_string())?;

    let release_json: Value = serde_json::from_str(&payload)
        .map_err(|err| format!("Failed to parse latest MattMC release metadata JSON: {}", err))?;

    let latest_tag = release_json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "Latest MattMC release metadata is missing tag_name".to_string())?
        .to_string();

    let assets = release_json
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| "Latest MattMC release metadata is missing assets array".to_string())?;

    let mut zip_asset_names = Vec::new();
    let mut selected_client_url: Option<String> = None;

    for asset in assets {
        let name = asset.get("name").and_then(Value::as_str).unwrap_or("");
        let browser_download_url = asset
            .get("browser_download_url")
            .and_then(Value::as_str)
            .unwrap_or("");

        let name_lower = name.to_lowercase();
        let url_lower = browser_download_url.to_lowercase();

        if name_lower.ends_with(".zip") {
            zip_asset_names.push(name.to_string());
        }

        if is_mattmc_client_zip_asset(&name_lower, &url_lower) {
            selected_client_url = Some(browser_download_url.to_string());
            break;
        }
    }

    let archive_url = selected_client_url.ok_or_else(|| {
        if zip_asset_names.is_empty() {
            "Latest MattMC release has no ZIP assets. Upload MattMC-Client*.zip as a release asset."
                .to_string()
        } else {
            format!(
                "Latest MattMC release is missing a MattMC-Client ZIP asset. Found ZIP assets: {}",
                zip_asset_names.join(", ")
            )
        }
    })?;

    Ok((latest_tag, archive_url))
}

fn is_mattmc_client_zip_asset(name_lower: &str, url_lower: &str) -> bool {
    name_lower.ends_with(".zip")
        && url_lower.ends_with(".zip")
        && (name_lower.starts_with("mattmc-client") || name_lower.contains("mattmc-client"))
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

fn extract_zip_into_target(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let extraction_root = env::temp_dir().join(format!(
        "basalt-mattmc-extract-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("Failed to read system clock: {}", err))?
            .as_secs()
    ));

    fs::create_dir_all(&extraction_root)
        .map_err(|err| format!("Failed to create temporary extraction directory: {}", err))?;

    let output = Command::new("unzip")
        .args(["-oq"])
        .arg(archive_path)
        .arg("-d")
        .arg(&extraction_root)
        .output()
        .map_err(|err| format!("Failed to run unzip for MattMC extraction: {}", err))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to extract MattMC release ZIP: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let entries = fs::read_dir(&extraction_root)
        .map_err(|err| format!("Failed to inspect extracted MattMC ZIP directory: {}", err))?;

    let mut children = Vec::new();
    for entry_result in entries {
        let entry = entry_result
            .map_err(|err| format!("Failed to read extracted MattMC ZIP entry: {}", err))?;
        children.push(entry.path());
    }

    let source_root = if children.len() == 1 && children[0].is_dir() {
        children[0].clone()
    } else {
        extraction_root.clone()
    };

    let copy_output = Command::new("cp")
        .args(["-a"])
        .arg(format!("{}/.", source_root.display()))
        .arg(target_dir)
        .output()
        .map_err(|err| format!("Failed to copy extracted MattMC files into target directory: {}", err))?;

    if !copy_output.status.success() {
        return Err(format!(
            "Failed to copy MattMC files into target directory: {}",
            String::from_utf8_lossy(&copy_output.stderr).trim()
        ));
    }

    if let Err(error) = fs::remove_dir_all(&extraction_root) {
        eprintln!(
            "Warning: Failed to remove temporary extraction directory at {}: {}",
            extraction_root.display(),
            error
        );
    }

    Ok(())
}