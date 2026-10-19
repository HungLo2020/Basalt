//! Built-in data actions for STANDARDS.md games: backups, restore, sync and cache clearing,
//! plus the launch/script environment and the guards that keep them from running while the
//! game is open or another operation holds the installation.
use crate::catalog::{GameDefinition, STANDARD_VERSION};
use crate::error::{CoreError, CoreResult};
use crate::file_index::{self, FileIndex, FileRecord, parallel};
use crate::game_layout::{self, BASALT_DIR, DataCategory, GameLayout};
use crate::progress::Progress;
use crate::{catalog, platform, registry, storage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNNING_MARKER: &str = "running.pid";
const OPERATION_LOCK: &str = "operation.lock";
const SYNC_STATE: &str = "sync-state.json";
const FILE_INDEX: &str = "file-index.json";
const REMOTE_MARKER: &str = ".basalt-sync.json";
/// Version of the sync state and the sync folder's file list.
const SYNC_FORMAT: u32 = 2;
const OBJECTS: &str = "objects";
const SNAPSHOTS: &str = "snapshots";
const LEGACY_SNAPSHOT_FILE: &str = "snapshot.json";
const LEGACY_FILES: &str = "files";
pub(crate) const DEFAULT_BACKUP_KEEP: u32 = 10;

/// A catalog game's installation on this machine.
pub(crate) struct Installation {
    pub definition: GameDefinition,
    pub root: PathBuf,
}

impl Installation {
    /// Finds the registered installation of a catalog game, wherever its library is.
    pub fn locate(game_id: &str) -> CoreResult<Self> {
        let definition = catalog::find_game(game_id)?;
        let entry = registry::load_entries()?
            .into_iter()
            .find(|e| e.name == definition.name)
            .ok_or_else(|| CoreError::GameNotFound(definition.name.clone()))?;
        if definition.follows_standard() && entry.runner_kind != crate::RunnerKind::Catalog {
            return Err(CoreError::InvalidInput(format!(
                "{} is registered from an older installation ({}). Reinstall it from the Install \
                 page (or `basalt install {}`) to use Basalt's actions; your files are kept.",
                definition.name, entry.launch_target, definition.id
            )));
        }
        let launch = PathBuf::from(&entry.launch_target);
        let mut root = launch
            .parent()
            .ok_or_else(|| CoreError::new("Launch target has no parent"))?
            .to_path_buf();
        // Catalog targets are the manifest at the root; legacy bash targets are the executable.
        let depth = match entry.runner_kind {
            crate::RunnerKind::Catalog => 0,
            crate::RunnerKind::Bash => Path::new(&definition.launch.executable)
                .components()
                .count()
                .saturating_sub(1),
            crate::RunnerKind::Steam => {
                return Err(CoreError::InvalidInput(
                    "Game does not have a local catalog installation".into(),
                ));
            }
        };
        for _ in 0..depth {
            root = root
                .parent()
                .ok_or_else(|| CoreError::new("Invalid installation root"))?
                .to_path_buf();
        }
        let root = root
            .canonicalize()
            .map_err(|e| CoreError::new(format!("Cannot resolve game root: {e}")))?;
        Ok(Self { definition, root })
    }

    pub fn layout(&self) -> GameLayout {
        GameLayout::for_game(&self.definition)
    }

    fn basalt_dir(&self) -> PathBuf {
        self.root.join(BASALT_DIR)
    }
}

pub(crate) fn running_marker(root: &Path) -> PathBuf {
    root.join(BASALT_DIR).join(RUNNING_MARKER)
}

/// True while Basalt has the game open (in any Basalt process).
pub(crate) fn is_running(root: &Path) -> bool {
    fs::read_to_string(running_marker(root))
        .ok()
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .is_some_and(platform::process_alive)
}

/// Exclusive hold on an installation for one data operation; released on drop.
pub(crate) struct OperationGuard {
    _lock: fs::File,
}

impl OperationGuard {
    /// Fails if another operation holds the installation, or (when `refuse_running`) if the
    /// game is open.
    pub fn acquire(installation: &Installation, refuse_running: bool) -> CoreResult<Self> {
        let name = &installation.definition.name;
        if refuse_running && is_running(&installation.root) {
            return Err(CoreError::InvalidInput(format!(
                "Close {name} before running this action"
            )));
        }
        let directory = installation.basalt_dir();
        fs::create_dir_all(&directory)
            .map_err(|e| CoreError::io("Create Basalt state directory", &directory, e))?;
        let path = directory.join(OPERATION_LOCK);
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| CoreError::io("Open operation lock", &path, e))?;
        match lock.try_lock() {
            Ok(()) => Ok(Self { _lock: lock }),
            Err(fs::TryLockError::WouldBlock) => Err(CoreError::InvalidInput(format!(
                "Another Basalt operation is already running for {name}"
            ))),
            Err(fs::TryLockError::Error(e)) => Err(CoreError::io("Lock installation", &path, e)),
        }
    }
}

/// True if a data operation currently holds the installation.
pub(crate) fn operation_in_progress(root: &Path) -> bool {
    let path = root.join(BASALT_DIR).join(OPERATION_LOCK);
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .is_ok_and(|file| matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock)))
}

// ----- environment ---------------------------------------------------------------------------

/// The `BASALT_*` variables for a launch (`action` = None) or a script action.
pub(crate) fn environment(
    definition: &GameDefinition,
    root: &Path,
    action: Option<&str>,
) -> CoreResult<Vec<(String, String)>> {
    let text = |path: &Path| path.to_string_lossy().into_owned();
    let mut variables = vec![
        ("BASALT_GAME_ID".to_string(), definition.id.clone()),
        ("BASALT_GAME_DIR".to_string(), text(root)),
        (
            "BASALT_STANDARD_VERSION".to_string(),
            STANDARD_VERSION.to_string(),
        ),
    ];
    if definition.follows_standard() {
        let layout = GameLayout::for_game(definition);
        for (name, category) in [
            ("BASALT_APP_DIR", DataCategory::App),
            ("BASALT_SAVE_DIR", DataCategory::Save),
            ("BASALT_CONFIG_DIR", DataCategory::Config),
            ("BASALT_SHARED_CONFIG_DIR", DataCategory::SharedConfig),
            ("BASALT_CACHE_DIR", DataCategory::Cache),
        ] {
            if let Some(path) = layout.primary(category) {
                variables.push((name.to_string(), text(&root.join(path))));
            }
        }
    }
    if let Some(action) = action {
        variables.push(("BASALT_ACTION".to_string(), action.to_string()));
        variables.push((
            "BASALT_BACKUP_DIR".to_string(),
            text(&backup_store(&definition.id)?),
        ));
    }
    Ok(variables)
}

// ----- local file index ----------------------------------------------------------------------

fn index_path(installation: &Installation) -> PathBuf {
    installation.basalt_dir().join(FILE_INDEX)
}

/// Indexes this installation's files in `categories`, reusing cached hashes for files whose size
/// and modification time are unchanged (all of them are re-read with `verify`), and refreshes
/// the cache.
pub(crate) fn scan_installation(
    installation: &Installation,
    categories: &[DataCategory],
    verify: bool,
    progress: &Progress,
) -> CoreResult<FileIndex> {
    let layout = installation.layout();
    let files = layout.collect_files(&installation.root, categories)?;
    let cache = file_index::load(&index_path(installation));
    let index = file_index::scan(
        &installation.root,
        &files,
        (!verify).then_some(&cache),
        progress,
        "Checking game data",
    )?;
    let mut updated = cache;
    updated.retain(|path, _| {
        !layout
            .category_of(Path::new(path))
            .is_some_and(|category| categories.contains(&category))
    });
    updated.extend(index.iter().map(|(path, record)| (path.clone(), record.clone())));
    save_index(installation, &updated);
    Ok(index)
}

fn save_index(installation: &Installation, index: &FileIndex) {
    if let Err(error) = file_index::save(&index_path(installation), index) {
        crate::warnings::warn(format!("Could not save the file index: {error}"));
    }
}

/// Records files just written into the installation, so the next scan need not re-read them.
fn remember_copies(installation: &Installation, written: &[(String, FileRecord)]) {
    let path = index_path(installation);
    let mut cache = file_index::load(&path);
    for (name, record) in written {
        let target = installation.root.join(name);
        match file_index::record_after_copy(&target, &record.hash, record.link.as_deref()) {
            Some(fresh) => cache.insert(name.clone(), fresh),
            None => cache.remove(name),
        };
    }
    save_index(installation, &cache);
}

/// What to change in a destination so it matches a source index.
struct Plan {
    copies: Vec<String>,
    deletes: Vec<String>,
}

impl Plan {
    fn between(source: &FileIndex, destination: &FileIndex) -> Self {
        Self {
            copies: source
                .iter()
                .filter(|(path, record)| {
                    destination
                        .get(*path)
                        .is_none_or(|existing| existing.hash != record.hash)
                })
                .map(|(path, _)| path.clone())
                .collect(),
            deletes: destination
                .keys()
                .filter(|path| !source.contains_key(*path))
                .cloned()
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.copies.is_empty() && self.deletes.is_empty()
    }

    /// True if applying it would replace or remove anything that exists in the destination.
    fn changes_existing(&self, destination: &FileIndex) -> bool {
        !self.deletes.is_empty() || self.copies.iter().any(|path| destination.contains_key(path))
    }
}

/// Applies a plan: deletes, then copies `source_of(path)` to `target/path` in parallel, checking
/// each copy against the hash it was planned with. Returns the copied records.
fn apply(
    plan: &Plan,
    layout: &GameLayout,
    target: &Path,
    expected: &FileIndex,
    source_of: impl Fn(&str) -> PathBuf + Sync,
    progress: &Progress,
    message: &str,
) -> CoreResult<Vec<(String, FileRecord)>> {
    for path in &plan.deletes {
        let file = target.join(path);
        match fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(CoreError::io("Remove game data", &file, e));
            }
            _ => {}
        }
        game_layout::prune_empty_parents(layout, target, Path::new(path));
    }
    parallel(&plan.copies, progress, message, |path| {
        let record = &expected[path];
        let hash = file_index::copy_hashed(&source_of(path), &target.join(path))?;
        if hash != record.hash {
            return Err(CoreError::new(format!(
                "{path} changed while it was being copied; run the action again"
            )));
        }
        Ok((path.clone(), record.clone()))
    })
}

// ----- backups -------------------------------------------------------------------------------
//
// Store layout (per game): `objects/<2 hex>/<hash>` holds each distinct file content once, and
// `snapshots/<id>.json` lists a snapshot's files by hash. A backup therefore only copies content
// the store does not already have. Snapshots from Basalt 0.5.0 (`<id>/snapshot.json` plus a
// `files/` copy) are still listed, restored and pruned.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub id: String,
    pub created_unix: u64,
    /// Why it was taken: "manual", "before-update", "before-sync-down", "before-restore",
    /// or "remote-before-sync-up".
    pub reason: String,
    pub categories: Vec<String>,
    pub files: usize,
}

#[derive(Serialize, Deserialize)]
struct SnapshotFile {
    #[serde(flatten)]
    info: BackupSnapshot,
    entries: FileIndex,
}

fn backup_store(game_id: &str) -> CoreResult<PathBuf> {
    Ok(storage::data_dir()?.join("backups").join(game_id))
}

fn object_path(store: &Path, hash: &str) -> PathBuf {
    store.join(OBJECTS).join(&hash[..2.min(hash.len())]).join(hash)
}

fn snapshot_path(store: &Path, id: &str) -> PathBuf {
    store.join(SNAPSHOTS).join(format!("{id}.json"))
}

/// Snapshots of a game, newest first.
pub fn list_backups(game_id: &str) -> CoreResult<Vec<BackupSnapshot>> {
    catalog::find_game(game_id)?;
    let store = backup_store(game_id)?;
    let mut snapshots: Vec<BackupSnapshot> = Vec::new();
    if let Ok(entries) = fs::read_dir(store.join(SNAPSHOTS)) {
        snapshots.extend(entries.flatten().filter_map(|entry| {
            let bytes = fs::read(entry.path()).ok()?;
            serde_json::from_slice(&bytes).ok()
        }));
    }
    if let Ok(entries) = fs::read_dir(&store) {
        snapshots.extend(
            entries
                .flatten()
                .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
                .filter_map(|entry| {
                    let bytes = fs::read(entry.path().join(LEGACY_SNAPSHOT_FILE)).ok()?;
                    serde_json::from_slice(&bytes).ok()
                }),
        );
    }
    snapshots.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(snapshots)
}

/// Saves a snapshot of `entries`. Content missing from the store is copied from the first of
/// `sources` (root, index) that has a file with that hash. Then prunes to `keep`.
pub(crate) fn create_snapshot(
    game_id: &str,
    entries: &FileIndex,
    sources: &[(&Path, &FileIndex)],
    categories: &[DataCategory],
    reason: &str,
    keep: u32,
    progress: &Progress,
) -> CoreResult<BackupSnapshot> {
    let store = backup_store(game_id)?;
    // Where each content hash can be read from, preferring earlier sources.
    let mut locations: BTreeMap<&str, PathBuf> = BTreeMap::new();
    for (root, index) in sources.iter().rev() {
        for (path, record) in index.iter().filter(|(_, record)| record.link.is_none()) {
            locations.insert(&record.hash, root.join(path));
        }
    }
    let mut needed: BTreeMap<&str, PathBuf> = BTreeMap::new();
    for record in entries.values().filter(|record| record.link.is_none()) {
        if needed.contains_key(record.hash.as_str()) || object_path(&store, &record.hash).is_file()
        {
            continue;
        }
        let source = locations.get(record.hash.as_str()).cloned();
        let source = source.ok_or_else(|| CoreError::new("Backup source file is missing"))?;
        needed.insert(&record.hash, source);
    }
    let needed: Vec<(&str, PathBuf)> = needed.into_iter().collect();
    parallel(&needed, progress, "Backing up game data", |(hash, source)| {
        let copied = file_index::copy_hashed(source, &object_path(&store, hash))?;
        if copied != *hash {
            let _ = fs::remove_file(object_path(&store, hash));
            return Err(CoreError::new(format!(
                "{} changed while it was being backed up; run the action again",
                source.display()
            )));
        }
        Ok(())
    })?;

    let now_ms = unix_now_ms();
    let stamp = timestamp_id(now_ms);
    let mut id = format!("{stamp}-{reason}");
    let mut counter = 2;
    while snapshot_path(&store, &id).exists() || store.join(&id).exists() {
        id = format!("{stamp}-{reason}-{counter}");
        counter += 1;
    }
    let snapshot = SnapshotFile {
        info: BackupSnapshot {
            id: id.clone(),
            created_unix: now_ms / 1000,
            reason: reason.to_string(),
            categories: categories.iter().map(|c| c.as_str().to_string()).collect(),
            files: entries.len(),
        },
        entries: entries
            .iter()
            .map(|(path, record)| (path.clone(), record.portable()))
            .collect(),
    };
    let bytes = serde_json::to_vec(&snapshot).map_err(|e| CoreError::new(e.to_string()))?;
    storage::write_atomic(&snapshot_path(&store, &id), &bytes)?;
    prune_backups(game_id, keep);
    Ok(snapshot.info)
}

/// Keeps the newest `keep` snapshots and deletes stored content no snapshot refers to.
fn prune_backups(game_id: &str, keep: u32) {
    let (Ok(snapshots), Ok(store)) = (list_backups(game_id), backup_store(game_id)) else {
        return;
    };
    for old in snapshots.iter().skip(keep.max(1) as usize) {
        let new_format = snapshot_path(&store, &old.id);
        let result = if new_format.is_file() {
            fs::remove_file(&new_format)
        } else {
            fs::remove_dir_all(store.join(&old.id))
        };
        if let Err(error) = result {
            crate::warnings::warn(format!("Could not remove old backup {}: {error}", old.id));
        }
    }
    let mut referenced = BTreeSet::new();
    if let Ok(entries) = fs::read_dir(store.join(SNAPSHOTS)) {
        for entry in entries.flatten() {
            let Some(snapshot) = fs::read(entry.path())
                .ok()
                .and_then(|bytes| serde_json::from_slice::<SnapshotFile>(&bytes).ok())
            else {
                // Unreadable snapshot: keep every object rather than risk deleting its content.
                return;
            };
            referenced.extend(snapshot.entries.into_values().map(|record| record.hash));
        }
    }
    let Ok(buckets) = fs::read_dir(store.join(OBJECTS)) else {
        return;
    };
    for bucket in buckets.flatten() {
        for object in fs::read_dir(bucket.path()).into_iter().flatten().flatten() {
            if !referenced.contains(object.file_name().to_string_lossy().as_ref()) {
                let _ = fs::remove_file(object.path());
            }
        }
        let _ = fs::remove_dir(bucket.path());
    }
}

/// Snapshot of this installation's backed-up categories.
pub(crate) fn backup_installation(
    installation: &Installation,
    categories: &[DataCategory],
    keep: u32,
    reason: &str,
    progress: &Progress,
) -> CoreResult<BackupSnapshot> {
    let categories = game_layout::expand_categories(categories);
    let index = scan_installation(installation, &categories, false, progress)?;
    create_snapshot(
        &installation.definition.id,
        &index,
        &[(&installation.root, &index)],
        &categories,
        reason,
        keep,
        progress,
    )
}

/// Replaces the snapshot's categories with its contents, after backing up the current state.
/// Only files that differ are copied.
pub(crate) fn restore_backup(
    installation: &Installation,
    snapshot_id: &str,
    safety_categories: &[DataCategory],
    keep: u32,
    progress: &Progress,
) -> CoreResult<String> {
    let game_id = &installation.definition.id;
    let snapshot = list_backups(game_id)?
        .into_iter()
        .find(|snapshot| snapshot.id == snapshot_id)
        .ok_or_else(|| {
            CoreError::InvalidInput(format!(
                "No backup '{snapshot_id}' for {} (see `basalt backups {game_id}`)",
                installation.definition.name
            ))
        })?;
    let categories: Vec<DataCategory> = snapshot
        .categories
        .iter()
        .filter_map(|c| DataCategory::parse(c))
        .collect();
    let store = backup_store(game_id)?;
    let new_format = snapshot_path(&store, &snapshot.id);
    let (entries, legacy_root) = if new_format.is_file() {
        let bytes = fs::read(&new_format).map_err(|e| CoreError::io("Read backup", &new_format, e))?;
        let file: SnapshotFile = serde_json::from_slice(&bytes).map_err(|e| CoreError::Json {
            path: new_format.clone(),
            source: e,
        })?;
        (file.entries, None)
    } else {
        let root = store.join(&snapshot.id).join(LEGACY_FILES);
        let files = all_files(&root)?;
        let index = file_index::scan(&root, &files, None, progress, "Reading backup")?;
        (index, Some(root))
    };
    backup_installation(installation, safety_categories, keep, "before-restore", progress)?;
    progress.check_cancelled()?;
    let local = scan_installation(installation, &categories, false, progress)?;
    let plan = Plan::between(&entries, &local);
    let layout = installation.layout();
    // Copying cannot be cancelled midway; the backup taken first is the way back.
    let copying = progress.uncancellable();
    let links: Vec<String> = plan
        .copies
        .iter()
        .filter(|path| entries[*path].link.is_some())
        .cloned()
        .collect();
    let content = Plan {
        copies: plan
            .copies
            .iter()
            .filter(|path| entries[*path].link.is_none())
            .cloned()
            .collect(),
        deletes: plan.deletes.clone(),
    };
    let mut written = apply(
        &content,
        &layout,
        &installation.root,
        &entries,
        |path| match &legacy_root {
            Some(root) => root.join(path),
            None => object_path(&store, &entries[path].hash),
        },
        &copying,
        "Restoring game data",
    )?;
    for path in links {
        let record = &entries[&path];
        let target = installation.root.join(&path);
        let link = record.link.as_deref().unwrap_or_default();
        if let Some(parent) = target.parent() {
            game_layout::ensure_directory(parent)?;
        }
        let _ = fs::remove_file(&target);
        std::os::unix::fs::symlink(link, &target)
            .map_err(|e| CoreError::io("Restore link", &target, e))?;
        written.push((path, record.clone()));
    }
    remember_copies(installation, &written);
    Ok(format!(
        "Restored {} backup {} ({} files changed, {} removed)",
        installation.definition.name,
        snapshot.id,
        plan.copies.len(),
        plan.deletes.len()
    ))
}

fn all_files(root: &Path) -> CoreResult<BTreeSet<PathBuf>> {
    fn visit(root: &Path, relative: &Path, files: &mut BTreeSet<PathBuf>) -> CoreResult<()> {
        let path = root.join(relative);
        let metadata =
            fs::symlink_metadata(&path).map_err(|e| CoreError::io("Read backup", &path, e))?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(|e| CoreError::io("Read backup", &path, e))? {
                let entry = entry.map_err(|e| CoreError::io("Read backup", &path, e))?;
                visit(root, &relative.join(entry.file_name()), files)?;
            }
        } else {
            files.insert(relative.to_path_buf());
        }
        Ok(())
    }
    let mut files = BTreeSet::new();
    if root.is_dir() {
        visit(root, Path::new(""), &mut files)?;
        files.remove(Path::new(""));
    }
    Ok(files)
}

// ----- sync ----------------------------------------------------------------------------------
//
// The sync folder holds `<GameId>/` with the synced files plus `.basalt-sync.json`, a list of
// those files and their hashes written by the last sync. A sync compares that list with this
// machine's cached index, so when nothing changed it reads one small file from the share, and
// otherwise it copies only the files that differ. The list is marked incomplete while files are
// being changed; an incomplete or missing list (or `verify`) makes Basalt read the folder itself.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SyncMode {
    Auto,
    Up,
    Down,
}

#[derive(Serialize, Deserialize)]
struct SyncState {
    remote: PathBuf,
    fingerprint: String,
    /// 2 since the per-file index; older fingerprints are not comparable.
    #[serde(default)]
    format: u32,
}

#[derive(Serialize, Deserialize)]
struct RemoteManifest {
    #[serde(default)]
    format: u32,
    game_id: String,
    updated_unix: u64,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    files: FileIndex,
}

/// One side of a sync: its fingerprint and whether it holds any files.
#[derive(Clone, Copy)]
struct Side<'a> {
    fingerprint: &'a str,
    empty: bool,
}

/// What a sync should do to bring both sides together, given each side and the fingerprint
/// both had after this machine's last sync (`base`).
fn auto_direction(
    local: Side<'_>,
    remote: Option<Side<'_>>,
    base: Option<&str>,
) -> Result<Option<SyncMode>, ()> {
    let Some(remote) = remote else {
        return Ok(Some(SyncMode::Up));
    };
    if local.fingerprint == remote.fingerprint {
        return Ok(None);
    }
    match base {
        Some(base) if base == remote.fingerprint => Ok(Some(SyncMode::Up)),
        Some(base) if base == local.fingerprint => Ok(Some(SyncMode::Down)),
        Some(_) => Err(()),
        // Never synced here: an empty side (e.g. a fresh install) takes the other side's data.
        None if local.empty => Ok(Some(SyncMode::Down)),
        None if remote.empty => Ok(Some(SyncMode::Up)),
        None => Err(()),
    }
}

pub(crate) struct SyncSettings<'a> {
    pub categories: &'a [DataCategory],
    pub backup_categories: &'a [DataCategory],
    pub keep: u32,
    /// Re-read every file on both sides instead of trusting the cache and the folder's list.
    pub verify: bool,
}

fn write_manifest(remote: &Path, game_id: &str, files: &FileIndex, complete: bool) -> CoreResult<()> {
    let manifest = RemoteManifest {
        format: SYNC_FORMAT,
        game_id: game_id.to_string(),
        updated_unix: unix_now(),
        complete,
        files: files
            .iter()
            .map(|(path, record)| (path.clone(), record.portable()))
            .collect(),
    };
    let bytes = serde_json::to_vec(&manifest).map_err(|e| CoreError::new(e.to_string()))?;
    storage::write_atomic(&remote.join(REMOTE_MARKER), &bytes)
}

pub(crate) fn sync(
    installation: &Installation,
    mode: SyncMode,
    settings: &SyncSettings<'_>,
    progress: &Progress,
) -> CoreResult<String> {
    let name = &installation.definition.name;
    let game_id = &installation.definition.id;
    let remote_root = crate::settings::load_sync_remote()?.path;
    // Never create the sync folder itself: if a share is unmounted, its mount point must not be
    // mistaken for an empty sync folder. Per-game folders inside it are created as needed.
    if !remote_root.is_dir() {
        return Err(CoreError::InvalidInput(format!(
            "The sync folder {} is not available. Is it mounted? (Change it in Settings → Game \
             Sync or with `basalt sync-remote <folder>`.)",
            remote_root.display()
        )));
    }
    let remote = remote_root.join(game_id);
    let categories = game_layout::expand_categories(settings.categories);
    let layout = installation.layout();
    let in_categories = |path: &str| {
        layout
            .category_of(Path::new(path))
            .is_some_and(|category| categories.contains(&category))
    };

    let local = scan_installation(installation, &categories, settings.verify, progress)?;
    let remote_exists = remote.is_dir();
    let manifest = fs::read(remote.join(REMOTE_MARKER))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RemoteManifest>(&bytes).ok())
        .filter(|m| m.format == SYNC_FORMAT && m.complete && !settings.verify);
    // The folder's list is trusted unless missing, incomplete, or verify was asked for.
    let (remote_index, list_is_current) = match (remote_exists, manifest) {
        (false, _) => (None, true),
        (true, Some(manifest)) => {
            let mut files = manifest.files;
            files.retain(|path, _| in_categories(path));
            (Some(files), true)
        }
        (true, None) => {
            let files = layout.collect_files(&remote, &categories)?;
            let index = file_index::scan(&remote, &files, None, progress, "Checking synced game data")?;
            (Some(index), false)
        }
    };
    let local_fp = file_index::fingerprint(&local);
    let remote_fp = remote_index.as_ref().map(file_index::fingerprint);

    let state_path = installation.basalt_dir().join(SYNC_STATE);
    let base = fs::read(&state_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SyncState>(&bytes).ok())
        .filter(|state| state.remote == remote_root && state.format == SYNC_FORMAT)
        .map(|state| state.fingerprint);
    let save_state = |fingerprint: &str| {
        let state = SyncState {
            remote: remote_root.clone(),
            fingerprint: fingerprint.to_string(),
            format: SYNC_FORMAT,
        };
        let bytes = serde_json::to_vec_pretty(&state).map_err(|e| CoreError::new(e.to_string()))?;
        storage::write_atomic(&state_path, &bytes)
    };
    let in_sync = |remote_index: &FileIndex| -> CoreResult<String> {
        if !list_is_current {
            write_manifest(&remote, game_id, remote_index, true)?;
        }
        save_state(&local_fp)?;
        Ok(format!("{name} is already in sync"))
    };

    let direction = match mode {
        SyncMode::Up => Some(SyncMode::Up),
        SyncMode::Down if !remote_exists => {
            return Err(CoreError::InvalidInput(format!(
                "There is no synced copy of {name} in {} yet",
                remote_root.display()
            )));
        }
        SyncMode::Down => Some(SyncMode::Down),
        SyncMode::Auto => auto_direction(
            Side {
                fingerprint: &local_fp,
                empty: local.is_empty(),
            },
            remote_fp.as_deref().map(|fingerprint| Side {
                fingerprint,
                empty: remote_index.as_ref().is_none_or(FileIndex::is_empty),
            }),
            base.as_deref(),
        )
        .map_err(|()| CoreError::SyncConflict(name.clone()))?,
    };
    progress.check_cancelled()?;
    let empty = FileIndex::new();
    let remote_index_ref = remote_index.as_ref().unwrap_or(&empty);
    match direction {
        None => in_sync(remote_index_ref),
        Some(SyncMode::Up) => {
            let plan = Plan::between(&local, remote_index_ref);
            if plan.is_empty() && remote_exists {
                return in_sync(remote_index_ref);
            }
            if plan.changes_existing(remote_index_ref) {
                create_snapshot(
                    game_id,
                    remote_index_ref,
                    &[(&installation.root, &local), (&remote, remote_index_ref)],
                    &categories,
                    "remote-before-sync-up",
                    settings.keep,
                    progress,
                )?;
            }
            fs::create_dir_all(&remote).map_err(|e| CoreError::io("Create sync copy", &remote, e))?;
            // Mark the list incomplete until every file is in place.
            write_manifest(&remote, game_id, remote_index_ref, false)?;
            apply(
                &plan,
                &layout,
                &remote,
                &local,
                |path| installation.root.join(path),
                &progress.uncancellable(),
                "Uploading game data",
            )?;
            write_manifest(&remote, game_id, &local, true)?;
            save_state(&local_fp)?;
            Ok(format!(
                "Synced {name} up ({} files copied, {} removed)",
                plan.copies.len(),
                plan.deletes.len()
            ))
        }
        Some(SyncMode::Down) | Some(SyncMode::Auto) => {
            let plan = Plan::between(remote_index_ref, &local);
            if plan.is_empty() {
                return in_sync(remote_index_ref);
            }
            if plan.changes_existing(&local) {
                backup_installation(
                    installation,
                    settings.backup_categories,
                    settings.keep,
                    "before-sync-down",
                    progress,
                )?;
            }
            let written = apply(
                &plan,
                &layout,
                &installation.root,
                remote_index_ref,
                |path| remote.join(path),
                &progress.uncancellable(),
                "Downloading game data",
            )?;
            remember_copies(installation, &written);
            if !list_is_current {
                write_manifest(&remote, game_id, remote_index_ref, true)?;
            }
            save_state(remote_fp.as_deref().unwrap_or(&local_fp))?;
            Ok(format!(
                "Synced {name} down ({} files copied, {} removed)",
                plan.copies.len(),
                plan.deletes.len()
            ))
        }
    }
}

// ----- cache ---------------------------------------------------------------------------------

pub(crate) fn clear_cache(installation: &Installation) -> CoreResult<String> {
    let layout = installation.layout();
    let files = layout.collect_files(&installation.root, &[DataCategory::Cache])?;
    for relative in &files {
        let path = installation.root.join(relative);
        fs::remove_file(&path).map_err(|e| CoreError::io("Clear cache", &path, e))?;
        game_layout::prune_empty_parents(&layout, &installation.root, relative);
    }
    Ok(format!(
        "Cleared {} cached files for {}",
        files.len(),
        installation.definition.name
    ))
}

// ----- time ----------------------------------------------------------------------------------

fn unix_now() -> u64 {
    unix_now_ms() / 1000
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `YYYYMMDDTHHMMSS.mmmZ` in UTC, so snapshot IDs sort chronologically.
fn timestamp_id(unix_ms: u64) -> String {
    let unix = unix_ms / 1000;
    let days = (unix / 86_400) as i64;
    let seconds = unix % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}.{:03}Z",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        unix_ms % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_ids_are_utc_and_sortable() {
        assert_eq!(timestamp_id(0), "19700101T000000.000Z");
        assert_eq!(timestamp_id(1_785_542_400_000), "20260801T000000.000Z");
        assert_eq!(timestamp_id((951_782_400 + 3_661) * 1000 + 42), "20000229T010101.042Z");
    }

    #[test]
    fn auto_sync_direction_follows_the_side_that_changed() {
        let side = |fingerprint| Side {
            fingerprint,
            empty: false,
        };
        let empty = |fingerprint| Side {
            fingerprint,
            empty: true,
        };
        assert_eq!(auto_direction(side("a"), None, None), Ok(Some(SyncMode::Up)));
        assert_eq!(auto_direction(side("a"), Some(side("a")), None), Ok(None));
        assert_eq!(
            auto_direction(side("b"), Some(side("a")), Some("a")),
            Ok(Some(SyncMode::Up))
        );
        assert_eq!(
            auto_direction(side("a"), Some(side("b")), Some("a")),
            Ok(Some(SyncMode::Down))
        );
        // Both changed, or never synced here and both have different data: ask.
        assert_eq!(auto_direction(side("b"), Some(side("c")), Some("a")), Err(()));
        assert_eq!(auto_direction(side("b"), Some(side("c")), None), Err(()));
        // A fresh install takes the synced data; an empty sync copy takes this machine's.
        assert_eq!(
            auto_direction(empty("e"), Some(side("c")), None),
            Ok(Some(SyncMode::Down))
        );
        assert_eq!(
            auto_direction(side("b"), Some(empty("e")), None),
            Ok(Some(SyncMode::Up))
        );
        // Once synced, an emptied side is a real change (e.g. deleted saves), not a fresh start.
        assert_eq!(
            auto_direction(empty("e"), Some(side("a")), Some("a")),
            Ok(Some(SyncMode::Up))
        );
    }
}
