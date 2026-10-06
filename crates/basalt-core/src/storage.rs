//! Where Basalt keeps its files, and how it writes them safely.
//!
//! - Data (games.tsv, playlists.tsv, blacklist.txt) lives in the platform data dir
//!   (`~/.local/share/basalt` on Linux), settings.json in the config dir
//!   (`~/.config/basalt`), and artwork caches in the cache dir (`~/.cache/basalt`).
//! - Files are replaced atomically (write temp file, fsync, rename) so readers never see a
//!   half-written file.
//! - Read-modify-write operations run under [`with_data_lock`], an advisory file lock that
//!   serializes the CLI, the GUI, and the GUI's own background threads.

use std::cell::Cell;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{CoreError, CoreResult};
use crate::platform;

const LOCK_FILE_NAME: &str = ".basalt.lock";
const LEGACY_DATA_FILES: [&str; 3] = ["games.tsv", "playlists.tsv", "blacklist.txt"];
const LEGACY_CONFIG_FILES: [&str; 1] = ["settings.json"];
const LEGACY_CACHE_DIR_NAME: &str = "cache";

static MIGRATE_LEGACY: Once = Once::new();
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static DATA_LOCK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

pub(crate) fn data_dir() -> CoreResult<PathBuf> {
    ensure_dir(platform::data_dir()?)
}

pub(crate) fn config_dir() -> CoreResult<PathBuf> {
    ensure_dir(platform::config_dir()?)
}

pub(crate) fn cache_dir() -> CoreResult<PathBuf> {
    ensure_dir(platform::cache_dir()?)
}

fn ensure_dir(dir: PathBuf) -> CoreResult<PathBuf> {
    MIGRATE_LEGACY.call_once(migrate_legacy_app_dir);
    fs::create_dir_all(&dir).map_err(|error| CoreError::io("Failed to create", &dir, error))?;
    Ok(dir)
}

/// Moves files from the old `~/.basalt` layout into the platform directories. Runs once per
/// process; each file is only moved when the new location doesn't already have one, so it is
/// safe to run repeatedly and concurrently. Failures are reported but never fatal.
fn migrate_legacy_app_dir() {
    let Ok(legacy_dir) = platform::legacy_app_dir() else {
        return;
    };
    if !legacy_dir.is_dir() {
        return;
    }

    let targets: [(&[&str], CoreResult<PathBuf>); 2] = [
        (&LEGACY_DATA_FILES, platform::data_dir()),
        (&LEGACY_CONFIG_FILES, platform::config_dir()),
    ];
    for (file_names, target_dir) in targets {
        let Ok(target_dir) = target_dir else {
            continue;
        };
        for file_name in file_names {
            let source = legacy_dir.join(file_name);
            if source.is_file()
                && let Err(error) = move_file_if_absent(&source, &target_dir.join(file_name))
            {
                crate::warnings::warn(format!(
                    "Could not move {} to its new location: {}",
                    source.display(),
                    error
                ));
            }
        }
    }

    // Artwork caches are regenerable, so only a cheap same-filesystem rename is attempted.
    let legacy_cache = legacy_dir.join(LEGACY_CACHE_DIR_NAME);
    if let (true, Ok(cache_dir)) = (legacy_cache.is_dir(), platform::cache_dir()) {
        if let Ok(entries) = fs::read_dir(&legacy_cache) {
            for entry in entries.flatten() {
                let destination = cache_dir.join(entry.file_name());
                if !destination.exists() && fs::create_dir_all(&cache_dir).is_ok() {
                    let _ = fs::rename(entry.path(), destination);
                }
            }
        }
        let _ = fs::remove_dir(&legacy_cache);
    }

    // Only succeeds once the legacy directory is empty.
    let _ = fs::remove_dir(&legacy_dir);
}

fn move_file_if_absent(source: &Path, destination: &Path) -> std::io::Result<()> {
    if destination.exists() {
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(source, destination).is_ok() {
        return Ok(());
    }

    // Cross-filesystem: copy atomically, then remove the original.
    let contents = fs::read(source)?;
    write_atomic_io(destination, &contents)?;
    fs::remove_file(source)
}

/// Replaces `path` with `contents` so that readers see either the old or the new file, never a
/// partial one.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> CoreResult<()> {
    write_atomic_io(path, contents).map_err(|error| CoreError::io("Failed to write", path, error))
}

fn write_atomic_io(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp_path = parent.join(format!(
        ".{}.{}-{}.tmp",
        file_name,
        std::process::id(),
        TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let result = (|| {
        let mut file = File::create(&temp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

/// Runs `operation` while holding Basalt's exclusive data lock.
///
/// The lock is an OS advisory lock on a file in the data directory, so it excludes other
/// processes and other threads in this process alike. Nested calls on the same thread are
/// re-entrant. Keep the critical section short: never launch a game or do network I/O under it.
pub(crate) fn with_data_lock<T>(operation: impl FnOnce() -> CoreResult<T>) -> CoreResult<T> {
    if DATA_LOCK_DEPTH.get() > 0 {
        let _depth = DepthGuard::enter();
        return operation();
    }

    let lock_path = data_dir()?.join(LOCK_FILE_NAME);
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|error| CoreError::io("Failed to open lock file", &lock_path, error))?;
    lock_file
        .lock()
        .map_err(|error| CoreError::io("Failed to lock", &lock_path, error))?;

    let result = {
        let _depth = DepthGuard::enter();
        operation()
    };

    // Closing the file releases the lock too; unlocking explicitly just makes it prompt.
    let _ = lock_file.unlock();
    result
}

struct DepthGuard;

impl DepthGuard {
    fn enter() -> Self {
        DATA_LOCK_DEPTH.set(DATA_LOCK_DEPTH.get() + 1);
        Self
    }
}

impl Drop for DepthGuard {
    fn drop(&mut self) {
        DATA_LOCK_DEPTH.set(DATA_LOCK_DEPTH.get() - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn write_atomic_replaces_contents_and_leaves_no_temp_files() {
        let dir = unique_temp_dir("basalt-atomic-test");
        let target = dir.join("games.tsv");

        write_atomic(&target, b"first\n").unwrap();
        write_atomic(&target, b"second\n").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "second\n");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name() != "games.tsv")
            .collect();
        assert!(leftovers.is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn move_file_if_absent_keeps_existing_destination() {
        let dir = unique_temp_dir("basalt-migrate-test");
        let source = dir.join("legacy").join("games.tsv");
        let destination = dir.join("new").join("games.tsv");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "legacy").unwrap();

        move_file_if_absent(&source, &destination).unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), "legacy");
        assert!(!source.exists());

        fs::write(&source, "newer legacy copy").unwrap();
        move_file_if_absent(&source, &destination).unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), "legacy");
        assert!(source.exists());

        let _ = fs::remove_dir_all(dir);
    }
}
