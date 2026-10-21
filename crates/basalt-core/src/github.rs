//! Optional GitHub credentials shared by install and update.
//! Never store tokens in catalogs, installed manifests, download URLs, or logs.
use std::process::Command;

/// Prefer explicit credentials. The GitHub CLI is an optional credential source,
/// not a runtime dependency for public repositories or environment-token users.
pub(crate) fn token() -> Option<String> {
    for name in ["GH_TOKEN", "GITHUB_TOKEN"] {
        if let Some(value) = std::env::var_os(name) {
            let value = value.to_string_lossy().trim().to_string();
            if !value.is_empty() && !value.contains(['\r', '\n']) {
                return Some(value);
            }
        }
    }
    let output = Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty() && !value.contains(['\r', '\n'])).then_some(value)
}

/// Construct a GitHub API-only release asset URL. Public releases without an
/// asset ID retain their existing browser URL path.
pub(crate) fn asset_url(repository: &str, id: u64) -> String {
    format!("https://api.github.com/repos/{repository}/releases/assets/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asset_url_uses_api_endpoint() {
        assert_eq!(asset_url("owner/repo", 42),
            "https://api.github.com/repos/owner/repo/releases/assets/42");
    }
}
