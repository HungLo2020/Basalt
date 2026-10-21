//! Optional, manifest-independent GitHub authentication.
//! Environment tokens take precedence. OAuth credentials live only in the Linux
//! Secret Service (KWallet/GNOME Keyring), never in JSON or game manifests.
use crate::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static REFRESH_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct Credentials {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn client_id() -> CoreResult<String> {
    std::env::var("BASALT_GITHUB_CLIENT_ID").ok().filter(|s| !s.trim().is_empty())
        .ok_or_else(|| CoreError::new("Native GitHub sign-in requires BASALT_GITHUB_CLIENT_ID (register Basalt as a GitHub OAuth App and enable Device Flow)."))
}

fn load() -> Option<Credentials> {
    let output = Command::new("secret-tool").args(["lookup", "application", "basalt", "service", "github"]).output().ok()?;
    if !output.status.success() { return None; }
    serde_json::from_slice(&output.stdout).ok()
}

fn save(creds: &Credentials) -> CoreResult<()> {
    let data = serde_json::to_vec(creds).map_err(|e| CoreError::new(e.to_string()))?;
    let mut child = Command::new("secret-tool").args([
        "store", "--label=Basalt GitHub account", "application", "basalt", "service", "github"
    ]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
        .map_err(|e| CoreError::new(format!("Cannot access Secret Service (secret-tool): {e}")))?;
    child.stdin.take().ok_or_else(|| CoreError::new("Missing keyring input"))?
        .write_all(&data).map_err(|e| CoreError::new(format!("Keyring write failed: {e}")))?;
    if !child.wait().map_err(|e| CoreError::new(e.to_string()))?.success() {
        return Err(CoreError::new("Keyring rejected GitHub credentials; login was not saved"));
    }
    Ok(())
}

fn oauth_request(fields: &[(&str, &str)], url: &str) -> CoreResult<serde_json::Value> {
    let mut response = ureq::post(url).header("Accept", "application/json")
        .send_form(fields.iter().copied()).map_err(|_| CoreError::new("GitHub authentication request failed"))?;
    let payload = response.body_mut().read_to_string()
        .map_err(|_| CoreError::new("Cannot read GitHub authentication response"))?;
    serde_json::from_str(&payload).map_err(|_| CoreError::new("Invalid GitHub authentication response"))
}

fn parse_credentials(value: &serde_json::Value) -> CoreResult<Credentials> {
    let access_token = value["access_token"].as_str()
        .ok_or_else(|| CoreError::new(format!("GitHub authentication failed: {}", value["error"].as_str().unwrap_or("no access token"))))?.to_string();
    let expires_at = value["expires_in"].as_u64().map(|sec| now().saturating_add(sec));
    Ok(Credentials {
        access_token,
        refresh_token: value["refresh_token"].as_str().map(str::to_owned),
        expires_at,
    })
}

fn refresh(creds: &Credentials) -> CoreResult<Credentials> {
    let refresh_token = creds.refresh_token.as_deref()
        .ok_or_else(|| CoreError::new("GitHub login expired; sign in again"))?;
    let id = client_id()?;
    let response = oauth_request(&[
        ("client_id", &id), ("grant_type", "refresh_token"), ("refresh_token", refresh_token)
    ], "https://github.com/login/oauth/access_token")?;
    let new_creds = parse_credentials(&response)?;
    // Refresh tokens rotate: persist the new pair before returning it.
    save(&new_creds)?;
    Ok(new_creds)
}

/// Obtain an optional token; never log or include tokens in an error.
pub(crate) fn token() -> Option<String> {
    for name in ["GH_TOKEN", "GITHUB_TOKEN"] {
        if let Some(value) = std::env::var_os(name) {
            let value = value.to_string_lossy().trim().to_string();
            if !value.is_empty() && !value.contains(['\r', '\n']) { return Some(value); }
        }
    }
    {
        let _lock = REFRESH_LOCK.lock().ok()?;
        if let Some(creds) = load() {
            if creds.expires_at.is_none_or(|expiry| expiry > now().saturating_add(120)) {
                return Some(creds.access_token);
            }
            if let Ok(refreshed) = refresh(&creds) {
                return Some(refreshed.access_token);
            }
            // Do not use an expired token or erase recoverable keyring credentials.
        }
    }
    // Optional compatibility with existing developer setup; not a dependency.
    let output = Command::new("gh").args(["auth", "token", "--hostname", "github.com"]).output().ok()?;
    if !output.status.success() { return None; }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty() && !value.contains(['\r', '\n'])).then_some(value)
}

/// Start native GitHub device authorization. Runs synchronously for CLI use.
/// GUI should run this on a worker thread and display the returned verification code.
pub fn login_device(on_code: impl FnOnce(&str, &str)) -> CoreResult<()> {
    let id = client_id()?;
    let device = oauth_request(&[
        ("client_id", &id), ("scope", "repo offline_access")
    ], "https://github.com/login/device/code")?;
    let code = device["device_code"].as_str().ok_or_else(|| CoreError::new("Missing GitHub device code"))?;
    let user_code = device["user_code"].as_str().ok_or_else(|| CoreError::new("Missing GitHub verification code"))?;
    let verification_uri = device["verification_uri"].as_str().ok_or_else(|| CoreError::new("Missing GitHub verification URL"))?;
    let expiry = now().saturating_add(device["expires_in"].as_u64().unwrap_or(900));
    let mut interval = device["interval"].as_u64().unwrap_or(5).max(5);
    on_code(verification_uri, user_code);
    while now() < expiry {
        std::thread::sleep(std::time::Duration::from_secs(interval));
        let result = oauth_request(&[
            ("client_id", &id), ("device_code", code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ], "https://github.com/login/oauth/access_token")?;
        if result.get("access_token").is_some() {
            return save(&parse_credentials(&result)?);
        }
        match result["error"].as_str().unwrap_or("") {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "expired_token" | "access_denied" => return Err(CoreError::new("GitHub authorization expired or was denied")),
            _ => return Err(CoreError::new(format!("GitHub authorization failed: {}", result["error"].as_str().unwrap_or("unknown error")))),
        }
    }
    Err(CoreError::new("GitHub login expired; try again"))
}

pub fn logout() -> CoreResult<()> {
    let status = Command::new("secret-tool").args(["clear", "application", "basalt", "service", "github"])
        .status().map_err(|e| CoreError::new(format!("Cannot clear GitHub keyring entry: {e}")))?;
    if status.success() { Ok(()) } else { Err(CoreError::new("Could not remove saved GitHub login")) }
}

pub fn auth_status() -> String {
    if std::env::var("GH_TOKEN").is_ok() || std::env::var("GITHUB_TOKEN").is_ok() {
        "GitHub token provided by environment".to_string()
    } else if let Some(saved) = load() {
        if saved.expires_at.is_some_and(|expiry| expiry <= now()) {
            "Saved GitHub login needs refresh".to_string()
        } else { "GitHub login saved in system keyring".to_string() }
    } else { "No native GitHub login saved".to_string() }
}

pub(crate) fn asset_url(repository: &str, id: u64) -> String {
    format!("https://api.github.com/repos/{repository}/releases/assets/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asset_url_uses_api_endpoint() {
        assert_eq!(asset_url("owner/repo", 42), "https://api.github.com/repos/owner/repo/releases/assets/42");
    }
    #[test]
    fn credentials_support_expiring_and_permanent_tokens() {
        let permanent = serde_json::json!({"access_token":"test"});
        assert!(parse_credentials(&permanent).unwrap().expires_at.is_none());
        let expiring = serde_json::json!({"access_token":"test","refresh_token":"renew","expires_in":28800});
        assert!(parse_credentials(&expiring).unwrap().expires_at.unwrap() > now());
    }
}
