//! Per-file content indexes: what a set of game files contains (hash, size) so that sync,
//! backup and restore compare and copy only what differs.
//!
//! Locally the index doubles as a stat cache: a file whose size and modification time match its
//! cached record keeps its hash without being read again. Hashes are 128-bit xxh3; they detect
//! changes, they are not a security boundary.
use crate::error::{CoreError, CoreResult};
use crate::progress::Progress;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use xxhash_rust::xxh3::Xxh3;

/// Parallel file operations; mostly to overlap per-file latency on network shares.
pub(crate) const WORKERS: usize = 6;
const BUFFER: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileRecord {
    pub hash: String,
    pub size: u64,
    /// Modification time in nanoseconds; only meaningful on the machine that recorded it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_ns: Option<i128>,
    /// Symlink target, for links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl FileRecord {
    /// The same content, without machine-specific fields.
    pub fn portable(&self) -> Self {
        Self {
            modified_ns: None,
            ..self.clone()
        }
    }
}

/// Relative path ('/'-separated) → record.
pub(crate) type FileIndex = BTreeMap<String, FileRecord>;

pub(crate) fn key(relative: &Path) -> CoreResult<String> {
    relative.to_str().map(str::to_string).ok_or_else(|| {
        CoreError::new(format!(
            "Game file name is not valid UTF-8: {}",
            relative.display()
        ))
    })
}

/// Fingerprint of a whole set: equal exactly when paths and contents are equal.
pub(crate) fn fingerprint(index: &FileIndex) -> String {
    let mut hasher = Xxh3::new();
    for (path, record) in index {
        hasher.update(path.as_bytes());
        hasher.update(&[0]);
        hasher.update(record.hash.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:032x}", hasher.digest128())
}

fn link_hash(target: &Path) -> String {
    let mut hasher = Xxh3::new();
    hasher.update(b"link\0");
    hasher.update(target.as_os_str().as_bytes());
    format!("{:032x}", hasher.digest128())
}

fn modified_ns(metadata: &fs::Metadata) -> Option<i128> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as i128)
}

/// Reads `path` and returns its content hash and size.
pub(crate) fn hash_file(path: &Path) -> CoreResult<(String, u64)> {
    let mut file = fs::File::open(path).map_err(|e| CoreError::io("Read game data", path, e))?;
    let mut hasher = Xxh3::new();
    let mut buffer = vec![0u8; BUFFER];
    let mut size = 0;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| CoreError::io("Read game data", path, e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((format!("{:032x}", hasher.digest128()), size))
}

/// Indexes `files` under `root`. With a `cache`, files whose size and modification time are
/// unchanged reuse the cached hash; everything else is read and hashed (in parallel).
pub(crate) fn scan(
    root: &Path,
    files: &BTreeSet<PathBuf>,
    cache: Option<&FileIndex>,
    progress: &Progress,
    message: &str,
) -> CoreResult<FileIndex> {
    let mut index = FileIndex::new();
    let mut to_hash: Vec<(String, PathBuf, Option<i128>)> = Vec::new();
    for relative in files {
        progress.check_cancelled()?;
        let name = key(relative)?;
        let path = root.join(relative);
        let metadata =
            fs::symlink_metadata(&path).map_err(|e| CoreError::io("Inspect game data", &path, e))?;
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(|e| CoreError::io("Read link", &path, e))?;
            index.insert(
                name,
                FileRecord {
                    hash: link_hash(&target),
                    size: 0,
                    modified_ns: None,
                    link: Some(target.to_string_lossy().into_owned()),
                },
            );
            continue;
        }
        let modified = modified_ns(&metadata);
        let cached = cache.and_then(|cache| cache.get(&name)).filter(|record| {
            record.link.is_none()
                && record.size == metadata.len()
                && modified.is_some()
                && record.modified_ns == modified
        });
        match cached {
            Some(record) => {
                index.insert(name, record.clone());
            }
            None => to_hash.push((name, path, modified)),
        }
    }
    let hashed = parallel(&to_hash, progress, message, |(_, path, _)| hash_file(path))?;
    for ((name, _, modified), (hash, size)) in to_hash.into_iter().zip(hashed) {
        index.insert(
            name,
            FileRecord {
                hash,
                size,
                modified_ns: modified,
                link: None,
            },
        );
    }
    Ok(index)
}

/// Runs `work` over `items` on [`WORKERS`] threads, reporting progress and stopping at the first
/// error (or cancellation). Results keep the order of `items`.
pub(crate) fn parallel<T, R>(
    items: &[T],
    progress: &Progress,
    message: &str,
    work: impl Fn(&T) -> CoreResult<R> + Sync,
) -> CoreResult<Vec<R>>
where
    T: Sync,
    R: Send,
{
    let total = items.len() as u64;
    if total == 0 {
        return Ok(Vec::new());
    }
    let next = AtomicUsize::new(0);
    let done = AtomicU64::new(0);
    let failed: Mutex<Option<CoreError>> = Mutex::new(None);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    progress.report_items(message, 0, total);
    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(items.len()) {
            scope.spawn(|| loop {
                if failed.lock().map(|f| f.is_some()).unwrap_or(true) {
                    return;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(index) else {
                    return;
                };
                let outcome = progress.check_cancelled().and_then(|()| work(item));
                match outcome {
                    Ok(value) => {
                        if let Ok(mut results) = results.lock() {
                            results[index] = Some(value);
                        }
                        let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                        progress.report_items(message, count, total);
                    }
                    Err(error) => {
                        if let Ok(mut failed) = failed.lock() {
                            failed.get_or_insert(error);
                        }
                        return;
                    }
                }
            });
        }
    });
    if let Some(error) = failed.into_inner().ok().flatten() {
        return Err(error);
    }
    results
        .into_inner()
        .map_err(|_| CoreError::new("A file worker panicked"))?
        .into_iter()
        .map(|result| result.ok_or_else(|| CoreError::new("A file worker stopped early")))
        .collect()
}

/// Copies one file or symlink to `target`, replacing whatever is there, and returns the hash of
/// what was copied (read once, while copying). The data goes to a temporary sibling first, so
/// the destination is never half-written.
pub(crate) fn copy_hashed(source: &Path, target: &Path) -> CoreResult<String> {
    let parent = target
        .parent()
        .ok_or_else(|| CoreError::new("Copy target has no parent"))?;
    crate::game_layout::ensure_directory(parent)?;
    let name = target
        .file_name()
        .ok_or_else(|| CoreError::new("Copy target has no file name"))?;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let temporary = parent.join(format!(
        ".{}.basalt-{}-{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let metadata =
        fs::symlink_metadata(source).map_err(|e| CoreError::io("Inspect game data", source, e))?;
    let hash = if metadata.file_type().is_symlink() {
        let link = fs::read_link(source).map_err(|e| CoreError::io("Read link", source, e))?;
        symlink(&link, &temporary).map_err(|e| CoreError::io("Copy link", &temporary, e))?;
        link_hash(&link)
    } else {
        let result = (|| {
            let mut input =
                fs::File::open(source).map_err(|e| CoreError::io("Read game data", source, e))?;
            let mut output = fs::File::create(&temporary)
                .map_err(|e| CoreError::io("Write game data", &temporary, e))?;
            let mut hasher = Xxh3::new();
            let mut buffer = vec![0u8; BUFFER];
            loop {
                let read = input
                    .read(&mut buffer)
                    .map_err(|e| CoreError::io("Read game data", source, e))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                output
                    .write_all(&buffer[..read])
                    .map_err(|e| CoreError::io("Write game data", &temporary, e))?;
            }
            output
                .flush()
                .map_err(|e| CoreError::io("Write game data", &temporary, e))?;
            // Keep the source's timestamp, so a later scan of the copy can reuse its hash.
            if let Ok(modified) = metadata.modified() {
                let _ = output.set_modified(modified);
            }
            let _ = output.set_permissions(metadata.permissions());
            Ok(format!("{:032x}", hasher.digest128()))
        })();
        match result {
            Ok(hash) => hash,
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(error);
            }
        }
    };
    if let Ok(existing) = fs::symlink_metadata(target)
        && existing.is_dir()
    {
        fs::remove_dir_all(target).map_err(|e| CoreError::io("Replace directory", target, e))?;
    }
    fs::rename(&temporary, target).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        CoreError::io("Write game data", target, e)
    })?;
    Ok(hash)
}

/// The local record for a file just written with known content `hash`.
pub(crate) fn record_after_copy(path: &Path, hash: &str, link: Option<&str>) -> Option<FileRecord> {
    let metadata = fs::symlink_metadata(path).ok()?;
    Some(FileRecord {
        hash: hash.to_string(),
        size: if link.is_some() { 0 } else { metadata.len() },
        modified_ns: if link.is_some() { None } else { modified_ns(&metadata) },
        link: link.map(str::to_string),
    })
}

pub(crate) fn load(path: &Path) -> FileIndex {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub(crate) fn save(path: &Path, index: &FileIndex) -> CoreResult<()> {
    let bytes = serde_json::to_vec(index).map_err(|e| CoreError::new(e.to_string()))?;
    crate::storage::write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_layout::tests::{temp_root, write};

    fn files(root: &Path) -> BTreeSet<PathBuf> {
        let mut set = BTreeSet::new();
        for entry in fs::read_dir(root).unwrap().flatten() {
            set.insert(PathBuf::from(entry.file_name()));
        }
        set
    }

    #[test]
    fn scan_reuses_cached_hashes_only_for_unchanged_files() {
        let root = temp_root("index-cache");
        write(&root.join("a"), "first");
        write(&root.join("b"), "other");
        let first = scan(&root, &files(&root), None, &Progress::none(), "").unwrap();
        // Same size, restored timestamp: indistinguishable by stat, so the cache is trusted...
        let modified = fs::metadata(root.join("a")).unwrap().modified().unwrap();
        write(&root.join("a"), "FIRST");
        fs::File::options()
            .write(true)
            .open(root.join("a"))
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let cached = scan(&root, &files(&root), Some(&first), &Progress::none(), "").unwrap();
        assert_eq!(cached["a"].hash, first["a"].hash);
        // ...while a full verify (no cache) sees the new content.
        let verified = scan(&root, &files(&root), None, &Progress::none(), "").unwrap();
        assert_ne!(verified["a"].hash, first["a"].hash);
        assert_eq!(verified["b"], first["b"]);
        assert_ne!(fingerprint(&verified), fingerprint(&first));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn copies_report_the_hash_of_what_was_written_and_keep_timestamps() {
        let root = temp_root("index-copy");
        write(&root.join("src/file"), "content");
        let hash = copy_hashed(&root.join("src/file"), &root.join("dst/nested/file")).unwrap();
        assert_eq!(hash, hash_file(&root.join("src/file")).unwrap().0);
        assert_eq!(fs::read_to_string(root.join("dst/nested/file")).unwrap(), "content");
        assert_eq!(
            fs::metadata(root.join("src/file")).unwrap().modified().unwrap(),
            fs::metadata(root.join("dst/nested/file")).unwrap().modified().unwrap()
        );
        let leftovers = fs::read_dir(root.join("dst/nested")).unwrap().count();
        assert_eq!(leftovers, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parallel_keeps_order_and_stops_on_errors() {
        let items: Vec<u32> = (0..50).collect();
        let doubled = parallel(&items, &Progress::none(), "", |n| Ok(n * 2)).unwrap();
        assert_eq!(doubled, items.iter().map(|n| n * 2).collect::<Vec<_>>());
        let failed = parallel(&items, &Progress::none(), "", |n| {
            if *n == 17 { Err(CoreError::new("boom")) } else { Ok(*n) }
        });
        assert_eq!(failed.unwrap_err().to_string(), "boom");
    }
}
