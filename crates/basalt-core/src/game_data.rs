//! Built-in data actions for STANDARDS.md games: backups, restore, sync and cache clearing,
//! plus the launch/script environment and the guards that keep them from running while the
//! game is open or another operation holds the installation.
use crate::catalog::{GameDefinition, STANDARD_VERSION};
use crate::error::{CoreError, CoreResult};
use crate::game_layout::{self, BASALT_DIR, DataCategory, GameLayout};
use crate::progress::Progress;
use crate::{catalog, platform, registry, storage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNNING_MARKER: &str = "running.pid";
const OPERATION_LOCK: &str = "operation.lock";
const SYNC_STATE: &str = "sync-state.json";
const REMOTE_MARKER: &str = ".basalt-sync.json";
const SNAPSHOT_FILE: &str = "snapshot.json";
const SNAPSHOT_FILES: &str = "files";
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

// ----- backups -------------------------------------------------------------------------------

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

fn backup_store(game_id: &str) -> CoreResult<PathBuf> {
    Ok(storage::data_dir()?.join("backups").join(game_id))
}

/// Snapshots of a game, newest first.
pub fn list_backups(game_id: &str) -> CoreResult<Vec<BackupSnapshot>> {
    catalog::find_game(game_id)?;
    let store = backup_store(game_id)?;
    let Ok(entries) = fs::read_dir(&store) else {
        return Ok(Vec::new());
    };
    let mut snapshots: Vec<BackupSnapshot> = entries
        .flatten()
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|entry| {
            let bytes = fs::read(entry.path().join(SNAPSHOT_FILE)).ok()?;
            serde_json::from_slice(&bytes).ok()
        })
        .collect();
    snapshots.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(snapshots)
}

/// Copies `files` (relative to `source`) into a new snapshot, then prunes to `keep`.
pub(crate) fn create_snapshot(
    game_id: &str,
    source: &Path,
    files: &BTreeSet<PathBuf>,
    categories: &[DataCategory],
    reason: &str,
    keep: u32,
    progress: &Progress,
) -> CoreResult<BackupSnapshot> {
    let store = backup_store(game_id)?;
    fs::create_dir_all(&store).map_err(|e| CoreError::io("Create backup store", &store, e))?;
    let now_ms = unix_now_ms();
    let now = now_ms / 1000;
    let stamp = timestamp_id(now_ms);
    let mut id = format!("{stamp}-{reason}");
    let mut counter = 2;
    while store.join(&id).exists() {
        id = format!("{stamp}-{reason}-{counter}");
        counter += 1;
    }
    let incoming = store.join(format!(".incoming-{id}"));
    let _ = fs::remove_dir_all(&incoming);
    let result = (|| {
        let total = files.len() as u64;
        for (index, relative) in files.iter().enumerate() {
            progress.check_cancelled()?;
            progress.report_items("Backing up game data", index as u64, total);
            game_layout::copy_entry(
                &source.join(relative),
                &incoming.join(SNAPSHOT_FILES).join(relative),
            )?;
        }
        fs::create_dir_all(incoming.join(SNAPSHOT_FILES))
            .map_err(|e| CoreError::io("Create backup", &incoming, e))?;
        let snapshot = BackupSnapshot {
            id: id.clone(),
            created_unix: now,
            reason: reason.to_string(),
            categories: categories.iter().map(|c| c.as_str().to_string()).collect(),
            files: files.len(),
        };
        let bytes =
            serde_json::to_vec_pretty(&snapshot).map_err(|e| CoreError::new(e.to_string()))?;
        storage::write_atomic(&incoming.join(SNAPSHOT_FILE), &bytes)?;
        fs::rename(&incoming, store.join(&id))
            .map_err(|e| CoreError::io("Save backup", &incoming, e))?;
        Ok(snapshot)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&incoming);
    }
    let snapshot = result?;
    prune_backups(game_id, keep);
    Ok(snapshot)
}

fn prune_backups(game_id: &str, keep: u32) {
    let (Ok(snapshots), Ok(store)) = (list_backups(game_id), backup_store(game_id)) else {
        return;
    };
    for old in snapshots.iter().skip(keep.max(1) as usize) {
        let path = store.join(&old.id);
        if let Err(error) = fs::remove_dir_all(&path) {
            crate::warnings::warn(format!(
                "Could not remove old backup {}: {error}",
                path.display()
            ));
        }
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
    let files = installation
        .layout()
        .collect_files(&installation.root, &categories)?;
    create_snapshot(
        &installation.definition.id,
        &installation.root,
        &files,
        &categories,
        reason,
        keep,
        progress,
    )
}

/// Replaces the snapshot's categories with its contents, after backing up the current state.
pub(crate) fn restore_backup(
    installation: &Installation,
    snapshot_id: &str,
    safety_categories: &[DataCategory],
    keep: u32,
    progress: &Progress,
) -> CoreResult<String> {
    let snapshot = list_backups(&installation.definition.id)?
        .into_iter()
        .find(|snapshot| snapshot.id == snapshot_id)
        .ok_or_else(|| {
            CoreError::InvalidInput(format!(
                "No backup '{snapshot_id}' for {} (see `basalt backups {}`)",
                installation.definition.name, installation.definition.id
            ))
        })?;
    let categories: Vec<DataCategory> = snapshot
        .categories
        .iter()
        .filter_map(|c| DataCategory::parse(c))
        .collect();
    let layout = installation.layout();
    let source = backup_store(&installation.definition.id)?
        .join(&snapshot.id)
        .join(SNAPSHOT_FILES);
    let files = all_files(&source)?;
    backup_installation(installation, safety_categories, keep, "before-restore", progress)?;
    progress.check_cancelled()?;
    let managed = layout.collect_files(&installation.root, &categories)?;
    game_layout::mirror(
        &layout,
        &source,
        &files,
        &installation.root,
        &managed,
        &progress.uncancellable(),
        "Restoring game data",
    )?;
    Ok(format!(
        "Restored {} backup {}",
        installation.definition.name, snapshot.id
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
}

#[derive(Serialize)]
struct RemoteMarker<'a> {
    game_id: &'a str,
    updated_unix: u64,
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
}

pub(crate) fn sync(
    installation: &Installation,
    mode: SyncMode,
    settings: &SyncSettings<'_>,
    progress: &Progress,
) -> CoreResult<String> {
    let name = &installation.definition.name;
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
    let remote = remote_root.join(&installation.definition.id);
    let categories = game_layout::expand_categories(settings.categories);
    let layout = installation.layout();
    let local_files = layout.collect_files(&installation.root, &categories)?;
    let remote_exists = remote.is_dir();
    let remote_files = if remote_exists {
        layout.collect_files(&remote, &categories)?
    } else {
        BTreeSet::new()
    };
    let local_fp = game_layout::fingerprint(
        &installation.root,
        &local_files,
        progress,
        "Checking local game data",
    )?;
    let remote_fp = if remote_exists {
        Some(game_layout::fingerprint(
            &remote,
            &remote_files,
            progress,
            "Checking synced game data",
        )?)
    } else {
        None
    };
    let state_path = installation.basalt_dir().join(SYNC_STATE);
    let base = fs::read(&state_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SyncState>(&bytes).ok())
        .filter(|state| state.remote == remote_root)
        .map(|state| state.fingerprint);
    let save_state = |fingerprint: &str| {
        let state = SyncState {
            remote: remote_root.clone(),
            fingerprint: fingerprint.to_string(),
        };
        let bytes = serde_json::to_vec_pretty(&state).map_err(|e| CoreError::new(e.to_string()))?;
        storage::write_atomic(&state_path, &bytes)
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
                empty: local_files.is_empty(),
            },
            remote_fp.as_deref().map(|fingerprint| Side {
                fingerprint,
                empty: remote_files.is_empty(),
            }),
            base.as_deref(),
        )
        .map_err(|()| CoreError::SyncConflict(name.clone()))?,
    };
    progress.check_cancelled()?;
    // Copying cannot be cancelled midway; the backup taken first is the way back.
    let copying = progress.uncancellable();
    match direction {
        None => {
            save_state(&local_fp)?;
            Ok(format!("{name} is already in sync"))
        }
        Some(SyncMode::Up) => {
            if !remote_files.is_empty() && remote_fp.as_deref() != Some(local_fp.as_str()) {
                create_snapshot(
                    &installation.definition.id,
                    &remote,
                    &remote_files,
                    &categories,
                    "remote-before-sync-up",
                    settings.keep,
                    progress,
                )?;
            }
            fs::create_dir_all(&remote).map_err(|e| CoreError::io("Create sync copy", &remote, e))?;
            let count = game_layout::mirror(
                &layout,
                &installation.root,
                &local_files,
                &remote,
                &remote_files,
                &copying,
                "Uploading game data",
            )?;
            let marker = RemoteMarker {
                game_id: &installation.definition.id,
                updated_unix: unix_now(),
            };
            let bytes =
                serde_json::to_vec_pretty(&marker).map_err(|e| CoreError::new(e.to_string()))?;
            storage::write_atomic(&remote.join(REMOTE_MARKER), &bytes)?;
            save_state(&local_fp)?;
            Ok(format!("Synced {name} up ({count} files)"))
        }
        Some(SyncMode::Down) | Some(SyncMode::Auto) => {
            backup_installation(
                installation,
                settings.backup_categories,
                settings.keep,
                "before-sync-down",
                progress,
            )?;
            let count = game_layout::mirror(
                &layout,
                &remote,
                &remote_files,
                &installation.root,
                &local_files,
                &copying,
                "Downloading game data",
            )?;
            save_state(remote_fp.as_deref().unwrap_or(&local_fp))?;
            Ok(format!("Synced {name} down ({count} files)"))
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
