//! Shared helpers for the CLI's end-to-end tests.

// These tests exist to spawn the CLI, which the platform-boundary lint otherwise forbids.
#![allow(clippy::disallowed_methods, clippy::disallowed_types, dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_HOME: AtomicU32 = AtomicU32::new(0);

/// A throwaway home directory, removed when dropped.
pub struct TestHome {
    pub path: PathBuf,
}

impl TestHome {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "basalt-cli-test-{}-{}-{}",
            name,
            std::process::id(),
            NEXT_HOME.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_basalt"));
        command
            .args(args)
            .env("HOME", &self.path)
            .env_remove("XDG_DATA_HOME")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_CACHE_HOME")
            .stdin(Stdio::null());
        command
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    pub fn spawn(&self, args: &[&str]) -> Child {
        self.command(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    pub fn data_dir(&self) -> PathBuf {
        self.path.join(".local/share/basalt")
    }

    pub fn config_dir(&self) -> PathBuf {
        self.path.join(".config/basalt")
    }

    /// An executable launch script the CLI will accept as a game target.
    pub fn script(&self, name: &str) -> String {
        let scripts = self.path.join("scripts");
        fs::create_dir_all(&scripts).unwrap();
        let path = scripts.join(format!("{}.sh", name));
        fs::write(&path, "#!/bin/bash\nexit 0\n").unwrap();
        path.to_string_lossy().into_owned()
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "basalt failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}
