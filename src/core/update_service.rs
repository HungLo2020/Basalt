use std::fs;
use std::io::copy;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::platform;

const BASALT_RELEASES_API_LATEST_URL: &str =
    "https://api.github.com/repos/HungLo2020/Basalt/releases/latest";
const BUILD_INFO_ASSET_NAME: &str = "basalt-build-info.json";

#[derive(Clone, Debug)]
pub struct BasaltBuildInfo {
    pub version: String,
    pub commit: String,
    pub build_time: String,
}

#[derive(Clone, Debug)]
pub struct UpdateCheckResult {
    pub current: BasaltBuildInfo,
    pub latest: BasaltBuildInfo,
    pub release_name: String,
    pub release_page_url: String,
    pub asset_name: String,
    pub asset_url: String,
    pub update_available: bool,
}

#[derive(Clone, Debug)]
pub struct DownloadedUpdate {
    pub installer_path: PathBuf,
}

pub fn current_build_info() -> BasaltBuildInfo {
    BasaltBuildInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: env!("BASALT_BUILD_COMMIT").to_string(),
        build_time: env!("BASALT_BUILD_TIME").to_string(),
    }
}

pub fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let release_json = fetch_json(BASALT_RELEASES_API_LATEST_URL)?;
    let asset = select_update_asset(&release_json)?;
    let latest = latest_build_info(&release_json)?;
    let current = current_build_info();
    let update_available = is_newer_build(&current, &latest);

    Ok(UpdateCheckResult {
        current,
        latest,
        release_name: release_json
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| release_json.get("tag_name").and_then(Value::as_str))
            .unwrap_or("Latest Basalt release")
            .to_string(),
        release_page_url: release_json
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or("https://github.com/HungLo2020/Basalt/releases/latest")
            .to_string(),
        asset_name: asset.name,
        asset_url: asset.url,
        update_available,
    })
}

pub fn download_update(update: &UpdateCheckResult) -> Result<DownloadedUpdate, String> {
    if !update.update_available {
        return Err("No Basalt update is available.".to_string());
    }

    let destination_path = temp_update_path(&update.asset_name)?;
    download_file(&update.asset_url, &destination_path)?;
    Ok(DownloadedUpdate {
        installer_path: destination_path,
    })
}

pub fn install_update_and_restart(downloaded_update: &DownloadedUpdate) -> Result<(), String> {
    platform::install_basalt_update_and_restart(&downloaded_update.installer_path)
}

pub fn can_install_updates() -> bool {
    platform::can_install_basalt_updates()
}

fn fetch_json(url: &str) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(Duration::from_secs(12))
        .build();

    let response = agent
        .get(url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "Basalt-Updater")
        .call()
        .map_err(|err| format!("Update check unavailable: {}", err))?;

    let payload = response
        .into_string()
        .map_err(|err| format!("Failed to read update response: {}", err))?;

    serde_json::from_str(&payload)
        .map_err(|err| format!("Failed to parse update response: {}", err))
}

fn latest_build_info(release_json: &Value) -> Result<BasaltBuildInfo, String> {
    if let Some(build_info_url) = find_asset_url(release_json, BUILD_INFO_ASSET_NAME) {
        if let Ok(build_info_json) = fetch_json(&build_info_url) {
            let version = build_info_json
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let commit = build_info_json
                .get("commit")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let build_time = json_string_or_u64(&build_info_json, "built_at_unix")
                .or_else(|| json_string_or_u64(&build_info_json, "built_at"))
                .unwrap_or_default();

            if !version.is_empty() || !commit.is_empty() {
                return Ok(BasaltBuildInfo {
                    version: version.to_string(),
                    commit: commit.to_string(),
                    build_time,
                });
            }
        }
    }

    let tag_name = release_json
        .get("tag_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let body = release_json
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("");
    let version = version_from_tag(tag_name)
        .or_else(|| version_from_assets(release_json))
        .unwrap_or_else(|| tag_name.to_string());

    Ok(BasaltBuildInfo {
        version,
        commit: commit_from_release_body(body).unwrap_or_default(),
        build_time: release_json
            .get("published_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

fn select_update_asset(release_json: &Value) -> Result<ReleaseAsset, String> {
    let Some(assets) = release_json.get("assets").and_then(Value::as_array) else {
        return Err("Latest Basalt release metadata is missing assets.".to_string());
    };

    let preferred_suffix = platform::basalt_update_asset_suffix();
    let preferred_marker = platform::basalt_update_asset_marker();
    let mut fallback: Option<ReleaseAsset> = None;

    for asset in assets {
        let name = asset.get("name").and_then(Value::as_str).unwrap_or("");
        let url = asset
            .get("browser_download_url")
            .and_then(Value::as_str)
            .unwrap_or("");
        let name_lower = name.to_ascii_lowercase();
        let url_lower = url.to_ascii_lowercase();

        if name.is_empty() || url.is_empty() || !name_lower.ends_with(preferred_suffix) {
            continue;
        }

        let candidate = ReleaseAsset {
            name: name.to_string(),
            url: url.to_string(),
        };

        if name_lower.contains(preferred_marker) || url_lower.contains(preferred_marker) {
            return Ok(candidate);
        }

        if fallback.is_none() {
            fallback = Some(candidate);
        }
    }

    fallback.ok_or_else(|| {
        format!(
            "Latest Basalt release has no {} asset for this platform.",
            preferred_suffix
        )
    })
}

fn find_asset_url(release_json: &Value, target_name: &str) -> Option<String> {
    release_json
        .get("assets")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|asset| {
            let name = asset.get("name").and_then(Value::as_str)?;
            if name.eq_ignore_ascii_case(target_name) {
                asset
                    .get("browser_download_url")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            } else {
                None
            }
        })
}

fn is_newer_build(current: &BasaltBuildInfo, latest: &BasaltBuildInfo) -> bool {
    let current_commit = normalized_commit(&current.commit);
    let latest_commit = normalized_commit(&latest.commit);

    // Release artifacts should be treated as the source of truth. If the release
    // carries a different commit than this executable, offer the update even
    // when package versions are unchanged.
    if !current_commit.is_empty() && !latest_commit.is_empty() {
        return !commits_match(current_commit, latest_commit);
    }

    if let (Some(current_build_time), Some(latest_build_time)) = (
        current.build_time.parse::<u64>().ok(),
        latest.build_time.parse::<u64>().ok(),
    ) {
        return latest_build_time > current_build_time;
    }

    !latest.build_time.trim().is_empty()
        && normalized_build_time(&latest.build_time) != normalized_build_time(&current.build_time)
}

fn json_string_or_u64(json: &Value, key: &str) -> Option<String> {
    let value = json.get(key)?;
    if let Some(string_value) = value.as_str() {
        let trimmed = string_value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    } else {
        value
            .as_u64()
            .map(|numeric_value| numeric_value.to_string())
    }
}

fn normalized_commit(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("unknown") {
        ""
    } else {
        trimmed
    }
}

fn commits_match(current: &str, latest: &str) -> bool {
    current.starts_with(latest) || latest.starts_with(current)
}

fn normalized_build_time(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("unknown") {
        ""
    } else {
        trimmed
    }
}

fn parse_version_parts(value: &str) -> Option<Vec<u64>> {
    let normalized = value.trim().trim_start_matches('v').trim_start_matches('V');
    if normalized.is_empty() || !normalized.chars().next()?.is_ascii_digit() {
        return None;
    }

    let mut parts = Vec::new();
    for part in normalized.split('.') {
        let numeric = part
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>();
        if numeric.is_empty() {
            return None;
        }
        parts.push(numeric.parse::<u64>().ok()?);
    }

    Some(parts)
}

fn version_from_tag(tag_name: &str) -> Option<String> {
    let trimmed = tag_name.trim();
    if parse_version_parts(trimmed).is_some() {
        Some(trimmed.trim_start_matches(is_version_prefix).to_string())
    } else {
        None
    }
}

fn version_from_assets(release_json: &Value) -> Option<String> {
    let assets = release_json.get("assets").and_then(Value::as_array)?;
    for asset in assets {
        let name = asset.get("name").and_then(Value::as_str).unwrap_or("");
        for token in name.split(['-', '_']) {
            if parse_version_parts(token).is_some() {
                return Some(token.trim_start_matches(is_version_prefix).to_string());
            }
        }
    }

    None
}

fn is_version_prefix(character: char) -> bool {
    character == 'v' || character == 'V'
}

fn commit_from_release_body(body: &str) -> Option<String> {
    let marker_index = body.find("Commit:")?;
    let after_marker = body[marker_index + "Commit:".len()..].trim_start();
    let commit = after_marker
        .split(|character: char| character.is_whitespace() || character == '.')
        .next()?
        .trim();

    if commit.is_empty() {
        None
    } else {
        Some(commit.to_string())
    }
}

fn temp_update_path(asset_name: &str) -> Result<PathBuf, String> {
    let sanitized_asset_name = asset_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("Failed to read system clock: {}", err))?
        .as_secs();

    Ok(std::env::temp_dir().join(format!(
        "basalt-update-{}-{}-{}",
        std::process::id(),
        timestamp,
        sanitized_asset_name
    )))
}

fn download_file(url: &str, destination_path: &PathBuf) -> Result<(), String> {
    if let Some(parent_directory) = destination_path.parent() {
        fs::create_dir_all(parent_directory)
            .map_err(|err| format!("Failed to prepare update download directory: {}", err))?;
    }

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(Duration::from_secs(60))
        .build();
    let response = agent
        .get(url)
        .set("User-Agent", "Basalt-Updater")
        .call()
        .map_err(|err| format!("Failed to download Basalt update: {}", err))?;

    let mut reader = response.into_reader();
    let mut destination_file = fs::File::create(destination_path).map_err(|err| {
        format!(
            "Failed to create update file {}: {}",
            destination_path.display(),
            err
        )
    })?;

    copy(&mut reader, &mut destination_file).map_err(|err| {
        format!(
            "Failed to write update file {}: {}",
            destination_path.display(),
            err
        )
    })?;

    Ok(())
}

struct ReleaseAsset {
    name: String,
    url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_build_uses_commit_difference_before_numeric_build_time() {
        let current = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: "abc1234".to_string(),
            build_time: "200".to_string(),
        };
        let latest_same_commit = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: "abc1234".to_string(),
            build_time: "300".to_string(),
        };
        let latest_new_commit_older_build_time = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: "def5678".to_string(),
            build_time: "100".to_string(),
        };

        assert!(!is_newer_build(&current, &latest_same_commit));
        assert!(is_newer_build(
            &current,
            &latest_new_commit_older_build_time
        ));
    }

    #[test]
    fn newer_build_uses_commit_difference_when_build_times_are_not_numeric() {
        let current = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: "abc1234".to_string(),
            build_time: "older".to_string(),
        };
        let latest = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: "def5678".to_string(),
            build_time: "newer".to_string(),
        };

        assert!(is_newer_build(&current, &latest));
    }

    #[test]
    fn newer_build_uses_numeric_build_time_when_commit_data_is_missing() {
        let current = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: String::new(),
            build_time: "100".to_string(),
        };
        let latest = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: String::new(),
            build_time: "200".to_string(),
        };

        assert!(is_newer_build(&current, &latest));
    }

    #[test]
    fn newer_build_ignores_version_when_release_is_not_newer() {
        let current = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: String::new(),
            build_time: "200".to_string(),
        };
        let latest = BasaltBuildInfo {
            version: "0.2.0".to_string(),
            commit: String::new(),
            build_time: "100".to_string(),
        };

        assert!(!is_newer_build(&current, &latest));
    }

    #[test]
    fn newer_build_offers_update_for_unknown_current_metadata() {
        let current = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: "unknown".to_string(),
            build_time: "unknown".to_string(),
        };
        let latest = BasaltBuildInfo {
            version: "0.1.0".to_string(),
            commit: "def5678".to_string(),
            build_time: "200".to_string(),
        };

        assert!(is_newer_build(&current, &latest));
    }
}
