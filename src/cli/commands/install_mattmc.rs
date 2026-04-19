use std::env;
use std::fs;
use std::io::copy;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use zip::ZipArchive;

use crate::core;
use crate::platform;

const MATTMC_RELEASES_API_LATEST_URL: &str =
    "https://api.github.com/repos/HungLo2020/MattMC/releases/latest";

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt install-mattmc".to_string());
    }

    let home = platform::home_dir()?;
    let games_dir = home.join("Games");
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
    let response = ureq::get(MATTMC_RELEASES_API_LATEST_URL)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "Basalt-MattMC-Installer")
        .call()
        .map_err(|err| {
            format!(
                "Failed to fetch latest MattMC release metadata: {}",
                err
            )
        })?;

    let payload = response
        .into_string()
        .map_err(|err| format!("Failed to read latest MattMC release metadata response: {}", err))?;

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
    if let Some(parent_directory) = destination_path.parent() {
        fs::create_dir_all(parent_directory)
            .map_err(|err| format!("Failed to prepare archive destination directory: {}", err))?;
    }

    let response = ureq::get(archive_url)
        .set("User-Agent", "Basalt-MattMC-Installer")
        .call()
        .map_err(|err| format!("Failed to download MattMC release archive: {}", err))?;

    let mut reader = response.into_reader();
    let mut destination_file =
        fs::File::create(destination_path).map_err(|err| {
            format!(
                "Failed to create MattMC archive destination file {}: {}",
                destination_path.display(),
                err
            )
        })?;

    copy(&mut reader, &mut destination_file).map_err(|err| {
        format!(
            "Failed to write MattMC archive to {}: {}",
            destination_path.display(),
            err
        )
    })?;

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

    extract_zip_archive(archive_path, &extraction_root)?;

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

    copy_directory_contents(&source_root, target_dir)?;

    if let Err(error) = fs::remove_dir_all(&extraction_root) {
        eprintln!(
            "Warning: Failed to remove temporary extraction directory at {}: {}",
            extraction_root.display(),
            error
        );
    }

    Ok(())
}

fn extract_zip_archive(archive_path: &Path, extraction_root: &Path) -> Result<(), String> {
    let zip_file = fs::File::open(archive_path)
        .map_err(|err| format!("Failed to open ZIP archive {}: {}", archive_path.display(), err))?;

    let mut zip_archive = ZipArchive::new(zip_file)
        .map_err(|err| format!("Failed to read ZIP archive {}: {}", archive_path.display(), err))?;

    for index in 0..zip_archive.len() {
        let mut entry = zip_archive
            .by_index(index)
            .map_err(|err| format!("Failed to read ZIP entry {}: {}", index, err))?;

        let Some(relative_path) = entry.enclosed_name().map(|value| value.to_path_buf()) else {
            continue;
        };

        let destination_path = extraction_root.join(relative_path);

        if entry.is_dir() {
            fs::create_dir_all(&destination_path).map_err(|err| {
                format!(
                    "Failed to create extraction directory {}: {}",
                    destination_path.display(),
                    err
                )
            })?;
            continue;
        }

        if let Some(parent_directory) = destination_path.parent() {
            fs::create_dir_all(parent_directory).map_err(|err| {
                format!(
                    "Failed to create extraction parent directory {}: {}",
                    parent_directory.display(),
                    err
                )
            })?;
        }

        let mut destination_file = fs::File::create(&destination_path).map_err(|err| {
            format!(
                "Failed to create extracted file {}: {}",
                destination_path.display(),
                err
            )
        })?;

        copy(&mut entry, &mut destination_file).map_err(|err| {
            format!(
                "Failed to write extracted file {}: {}",
                destination_path.display(),
                err
            )
        })?;
    }

    Ok(())
}

fn copy_directory_contents(source_root: &Path, target_root: &Path) -> Result<(), String> {
    fs::create_dir_all(target_root).map_err(|err| {
        format!(
            "Failed to create target directory {}: {}",
            target_root.display(),
            err
        )
    })?;

    let entries = fs::read_dir(source_root).map_err(|err| {
        format!(
            "Failed to read extracted source directory {}: {}",
            source_root.display(),
            err
        )
    })?;

    for entry_result in entries {
        let entry = entry_result
            .map_err(|err| format!("Failed to read extracted source entry: {}", err))?;

        let source_path = entry.path();
        let destination_path = target_root.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_contents(&source_path, &destination_path)?;
            continue;
        }

        if let Some(parent_directory) = destination_path.parent() {
            fs::create_dir_all(parent_directory).map_err(|err| {
                format!(
                    "Failed to create destination parent directory {}: {}",
                    parent_directory.display(),
                    err
                )
            })?;
        }

        fs::copy(&source_path, &destination_path).map_err(|err| {
            format!(
                "Failed to copy {} to {}: {}",
                source_path.display(),
                destination_path.display(),
                err
            )
        })?;
    }

    Ok(())
}