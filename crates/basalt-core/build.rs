#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

//! Embeds the git commit and build time, which the self-updater compares against releases.

use std::env;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");

    if let Some(head_ref) = git_output(&["symbolic-ref", "-q", "HEAD"]) {
        println!("cargo:rerun-if-changed=../../.git/{}", head_ref);
    }

    println!(
        "cargo:rustc-env=BASALT_BUILD_COMMIT={}",
        env::var("GITHUB_SHA")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| git_output(&["rev-parse", "HEAD"]))
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!(
        "cargo:rustc-env=BASALT_BUILD_TIME={}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    );
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
