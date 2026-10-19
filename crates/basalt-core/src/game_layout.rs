//! The STANDARDS.md data layout: which category every path in an installation belongs to, and
//! file-level helpers (collect, fingerprint, mirror) that the install, backup and sync code share.
//!
//! All paths are relative to a root (an installation, a backup snapshot or a sync folder), so a
//! game behaves the same in every library.
use crate::catalog::GameDefinition;
use crate::error::{CoreError, CoreResult};
use crate::progress::Progress;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

/// Basalt's own per-installation files (sync state, running marker, operation lock).
pub(crate) const BASALT_DIR: &str = ".basalt";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataCategory {
    App,
    Save,
    SharedConfig,
    Config,
    Cache,
}

impl DataCategory {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "app" => Some(Self::App),
            "save" => Some(Self::Save),
            "shared_config" => Some(Self::SharedConfig),
            "config" => Some(Self::Config),
            "cache" => Some(Self::Cache),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Save => "save",
            Self::SharedConfig => "shared_config",
            Self::Config => "config",
            Self::Cache => "cache",
        }
    }
}

/// Category sets used by backup and sync. Including `config` includes `shared_config`, which
/// is nested inside it by default.
pub(crate) fn expand_categories(categories: &[DataCategory]) -> Vec<DataCategory> {
    let mut expanded: Vec<DataCategory> = categories.to_vec();
    if expanded.contains(&DataCategory::Config) && !expanded.contains(&DataCategory::SharedConfig) {
        expanded.push(DataCategory::SharedConfig);
    }
    expanded.sort();
    expanded.dedup();
    expanded
}

#[derive(Debug, Clone)]
pub struct GameLayout {
    entries: Vec<(PathBuf, DataCategory)>,
}

impl GameLayout {
    /// The effective layout of a schema 2 entry: standard defaults, each category replaced by
    /// the catalog's override when present.
    pub fn for_game(game: &GameDefinition) -> Self {
        let layout = game.layout.clone().unwrap_or_default();
        let pick = |custom: Option<Vec<String>>, default: &str| {
            custom.unwrap_or_else(|| vec![default.to_string()])
        };
        let mut entries = Vec::new();
        for (category, paths) in [
            (DataCategory::App, pick(layout.app, "app")),
            (DataCategory::Save, pick(layout.save, "data/save")),
            (
                DataCategory::SharedConfig,
                pick(layout.shared_config, "data/config/shared"),
            ),
            (DataCategory::Config, pick(layout.config, "data/config")),
            (DataCategory::Cache, pick(layout.cache, "data/cache")),
        ] {
            for path in paths {
                entries.push((PathBuf::from(path), category));
            }
        }
        Self { entries }
    }

    /// The category of `relative`: that of the most specific layout path containing it, or
    /// `None` for uncategorized files (preserved on update, never synced or backed up).
    pub fn category_of(&self, relative: &Path) -> Option<DataCategory> {
        self.entries
            .iter()
            .filter(|(path, _)| relative.starts_with(path))
            .max_by_key(|(path, _)| path.components().count())
            .map(|(_, category)| *category)
    }

    pub fn paths(&self, category: DataCategory) -> impl Iterator<Item = &Path> {
        self.entries
            .iter()
            .filter(move |(_, c)| *c == category)
            .map(|(path, _)| path.as_path())
    }

    /// The first path of a category, which the `BASALT_*_DIR` variables point at.
    pub fn primary(&self, category: DataCategory) -> Option<&Path> {
        self.paths(category).next()
    }

    pub fn is_layout_root(&self, relative: &Path) -> bool {
        self.entries.iter().any(|(path, _)| path == relative)
    }

    /// True when some layout path of `category` lies strictly inside `relative`.
    fn has_nested(&self, relative: &Path, category: DataCategory) -> bool {
        self.entries
            .iter()
            .any(|(path, c)| *c == category && path != relative && path.starts_with(relative))
    }

    /// Every file and symlink under `root` whose category is in `categories`, relative to `root`.
    /// Directories are traversed but symlinked directories are never followed.
    pub fn collect_files(
        &self,
        root: &Path,
        categories: &[DataCategory],
    ) -> CoreResult<BTreeSet<PathBuf>> {
        let mut files = BTreeSet::new();
        for (path, category) in &self.entries {
            if categories.contains(category) {
                walk(root, path, &mut |relative| {
                    if self
                        .category_of(relative)
                        .is_some_and(|c| categories.contains(&c))
                    {
                        files.insert(relative.to_path_buf());
                    }
                })?;
            }
        }
        Ok(files)
    }

    /// Files under `root` that are not application files: user data and anything uncategorized.
    /// Basalt's manifest and staging directories are excluded; `.basalt/` is included.
    pub fn non_app_entries(&self, root: &Path) -> CoreResult<Vec<PathBuf>> {
        let mut kept = Vec::new();
        self.non_app_in(root, Path::new(""), &mut kept)?;
        Ok(kept)
    }

    fn non_app_in(&self, root: &Path, relative: &Path, kept: &mut Vec<PathBuf>) -> CoreResult<()> {
        let directory = root.join(relative);
        for entry in fs::read_dir(&directory)
            .map_err(|e| CoreError::io("Read installation", &directory, e))?
        {
            let entry = entry.map_err(|e| CoreError::io("Read installation", &directory, e))?;
            let child = relative.join(entry.file_name());
            if relative.as_os_str().is_empty()
                && entry.file_name() == crate::catalog_launch::MANIFEST
            {
                continue;
            }
            let is_dir = entry
                .file_type()
                .map_err(|e| CoreError::io("Inspect installation", entry.path(), e))?
                .is_dir();
            match self.category_of(&child) {
                // Application paths are replaced by the release, except data nested inside them.
                Some(DataCategory::App) => {
                    if is_dir && self.has_non_app_nested(&child) {
                        self.non_app_in(root, &child, kept)?;
                    }
                }
                _ if is_dir && self.has_nested(&child, DataCategory::App) => {
                    self.non_app_in(root, &child, kept)?;
                }
                _ => kept.push(child),
            }
        }
        Ok(())
    }

    fn has_non_app_nested(&self, relative: &Path) -> bool {
        self.entries
            .iter()
            .any(|(path, c)| *c != DataCategory::App && path != relative && path.starts_with(relative))
    }
}

fn walk(root: &Path, relative: &Path, visit: &mut dyn FnMut(&Path)) -> CoreResult<()> {
    let path = root.join(relative);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return Ok(());
    };
    if metadata.is_dir() {
        for entry in fs::read_dir(&path).map_err(|e| CoreError::io("Read game data", &path, e))? {
            let entry = entry.map_err(|e| CoreError::io("Read game data", &path, e))?;
            walk(root, &relative.join(entry.file_name()), visit)?;
        }
    } else if metadata.is_file() || metadata.file_type().is_symlink() {
        visit(relative);
    }
    Ok(())
}

/// Stable content fingerprint of `files` under `root` (paths, symlink targets and file bytes),
/// so two copies of a data set compare equal regardless of timestamps. FNV-1a, 64-bit.
pub(crate) fn fingerprint(
    root: &Path,
    files: &BTreeSet<PathBuf>,
    progress: &Progress,
    message: &str,
) -> CoreResult<String> {
    use std::io::Read;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let total = files.len() as u64;
    for (index, relative) in files.iter().enumerate() {
        progress.check_cancelled()?;
        progress.report_items(message, index as u64, total);
        feed(relative.as_os_str().as_bytes());
        feed(&[0]);
        let path = root.join(relative);
        let metadata =
            fs::symlink_metadata(&path).map_err(|e| CoreError::io("Inspect game data", &path, e))?;
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(|e| CoreError::io("Read link", &path, e))?;
            feed(b"L");
            feed(target.as_os_str().as_bytes());
        } else {
            feed(b"F");
            let mut file =
                fs::File::open(&path).map_err(|e| CoreError::io("Read game data", &path, e))?;
            let mut buffer = vec![0u8; 64 * 1024];
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|e| CoreError::io("Read game data", &path, e))?;
                if read == 0 {
                    break;
                }
                feed(&buffer[..read]);
            }
        }
        feed(&[0xff]);
    }
    Ok(format!("{hash:016x}"))
}

/// Copies one file or symlink, replacing whatever is at `target` (via a temporary sibling and
/// rename, so the destination is never half-written).
pub(crate) fn copy_entry(source: &Path, target: &Path) -> CoreResult<()> {
    let parent = target
        .parent()
        .ok_or_else(|| CoreError::new("Copy target has no parent"))?;
    ensure_directory(parent)?;
    let name = target
        .file_name()
        .ok_or_else(|| CoreError::new("Copy target has no file name"))?;
    let temporary = parent.join(format!(
        ".{}.basalt-{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let _ = fs::remove_file(&temporary);
    let metadata =
        fs::symlink_metadata(source).map_err(|e| CoreError::io("Inspect game data", source, e))?;
    if metadata.file_type().is_symlink() {
        let link = fs::read_link(source).map_err(|e| CoreError::io("Read link", source, e))?;
        symlink(link, &temporary).map_err(|e| CoreError::io("Copy link", &temporary, e))?;
    } else {
        fs::copy(source, &temporary).map_err(|e| CoreError::io("Copy game data", source, e))?;
    }
    if let Ok(existing) = fs::symlink_metadata(target)
        && existing.is_dir()
    {
        fs::remove_dir_all(target).map_err(|e| CoreError::io("Replace directory", target, e))?;
    }
    fs::rename(&temporary, target).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        CoreError::io("Write game data", target, e)
    })
}

/// Creates `directory` and its parents, replacing a plain file or symlink that is in the way
/// (a file that became a directory in the incoming data).
fn ensure_directory(directory: &Path) -> CoreResult<()> {
    if let Ok(metadata) = fs::symlink_metadata(directory) {
        if metadata.is_dir() {
            return Ok(());
        }
        fs::remove_file(directory).map_err(|e| CoreError::io("Replace file", directory, e))?;
    } else if let Some(parent) = directory.parent() {
        ensure_directory(parent)?;
    }
    fs::create_dir(directory).map_err(|e| CoreError::io("Create directory", directory, e))
}

/// Makes the managed files under `target` match `source`: copies every source file and deletes
/// target files in `managed` that the source lacks, then prunes directories that became empty
/// (stopping at layout roots so the game's top-level folders remain).
pub(crate) fn mirror(
    layout: &GameLayout,
    source: &Path,
    files: &BTreeSet<PathBuf>,
    target: &Path,
    managed: &BTreeSet<PathBuf>,
    progress: &Progress,
    message: &str,
) -> CoreResult<usize> {
    let stale: Vec<&PathBuf> = managed.difference(files).collect();
    let total = (files.len() + stale.len()) as u64;
    let mut done = 0;
    for relative in &stale {
        progress.report_items(message, done, total);
        let path = target.join(relative);
        fs::remove_file(&path).map_err(|e| CoreError::io("Remove game data", &path, e))?;
        prune_empty_parents(layout, target, relative);
        done += 1;
    }
    for relative in files {
        progress.report_items(message, done, total);
        copy_entry(&source.join(relative), &target.join(relative))?;
        done += 1;
    }
    Ok(files.len())
}

/// Removes now-empty parent directories of `relative`, stopping at a layout root.
pub(crate) fn prune_empty_parents(layout: &GameLayout, root: &Path, relative: &Path) {
    let mut current = relative.parent();
    while let Some(directory) = current {
        if directory.as_os_str().is_empty() || layout.is_layout_root(directory) {
            break;
        }
        if fs::remove_dir(root.join(directory)).is_err() {
            break;
        }
        current = directory.parent();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::catalog::standard_game;
    use serde_json::json;

    pub(crate) fn temp_root(name: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "basalt-layout-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    pub(crate) fn write(path: &Path, value: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }

    #[test]
    fn default_layout_classifies_by_most_specific_path() {
        let layout = GameLayout::for_game(&standard_game(json!({})));
        let category = |path: &str| layout.category_of(Path::new(path));
        assert_eq!(category("app/bin/game"), Some(DataCategory::App));
        assert_eq!(category("data/save/slot1.sav"), Some(DataCategory::Save));
        assert_eq!(category("data/config/video.ini"), Some(DataCategory::Config));
        assert_eq!(
            category("data/config/shared/keys.ini"),
            Some(DataCategory::SharedConfig)
        );
        assert_eq!(category("data/cache/shaders.bin"), Some(DataCategory::Cache));
        assert_eq!(category("notes.txt"), None);
        // Component-wise, not string, prefixes.
        assert_eq!(category("data/saved-elsewhere"), None);
    }

    #[test]
    fn collect_files_respects_nested_categories_and_overrides() {
        let game = standard_game(json!({"layout": {
            "save": ["run/saves", "run/mods"],
            "config": ["run/options.txt"],
            "cache": ["run/saves/logs"]
        }}));
        let layout = GameLayout::for_game(&game);
        let root = temp_root("collect");
        write(&root.join("run/saves/world/level.dat"), "w");
        write(&root.join("run/saves/logs/latest.log"), "log");
        write(&root.join("run/mods/a.jar"), "m");
        write(&root.join("run/options.txt"), "o");
        write(&root.join("run/other.txt"), "x");
        let saves = layout.collect_files(&root, &[DataCategory::Save]).unwrap();
        let names: Vec<_> = saves.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["run/mods/a.jar", "run/saves/world/level.dat"]);
        let config = layout.collect_files(&root, &[DataCategory::Config]).unwrap();
        assert_eq!(config.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn non_app_entries_keep_data_inside_app_directories() {
        let game = standard_game(json!({"layout": {"app": ["bin", "lib"], "save": ["lib/user"]}}));
        let layout = GameLayout::for_game(&game);
        let root = temp_root("non-app");
        write(&root.join("bin/game"), "g");
        write(&root.join("lib/core.so"), "c");
        write(&root.join("lib/user/profile"), "p");
        write(&root.join("data/save/s"), "s");
        write(&root.join(crate::catalog_launch::MANIFEST), "{}");
        let mut kept: Vec<_> = layout
            .non_app_entries(&root)
            .unwrap()
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        kept.sort();
        assert_eq!(kept, ["data", "lib/user"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fingerprint_depends_on_content_not_timestamps() {
        let layout = GameLayout::for_game(&standard_game(json!({})));
        let a = temp_root("fp-a");
        let b = temp_root("fp-b");
        write(&a.join("data/save/s"), "same");
        write(&b.join("data/save/s"), "same");
        let fp = |root: &Path| {
            let files = layout.collect_files(root, &[DataCategory::Save]).unwrap();
            fingerprint(root, &files, &Progress::none(), "").unwrap()
        };
        assert_eq!(fp(&a), fp(&b));
        write(&b.join("data/save/s"), "different");
        assert_ne!(fp(&a), fp(&b));
        let _ = fs::remove_dir_all(a);
        let _ = fs::remove_dir_all(b);
    }

    #[test]
    fn mirror_replaces_removes_and_prunes_empty_directories() {
        let layout = GameLayout::for_game(&standard_game(json!({})));
        let source = temp_root("mirror-src");
        let target = temp_root("mirror-dst");
        write(&source.join("data/save/a"), "new a");
        write(&target.join("data/save/a"), "old a");
        write(&target.join("data/save/world/gone"), "stale");
        write(&target.join("data/config/video.ini"), "unmanaged");
        let files = layout.collect_files(&source, &[DataCategory::Save]).unwrap();
        let managed = layout.collect_files(&target, &[DataCategory::Save]).unwrap();
        mirror(&layout, &source, &files, &target, &managed, &Progress::none(), "").unwrap();
        assert_eq!(fs::read_to_string(target.join("data/save/a")).unwrap(), "new a");
        assert!(!target.join("data/save/world").exists());
        assert!(target.join("data/save").is_dir());
        assert!(target.join("data/config/video.ini").exists());
        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(target);
    }
}
