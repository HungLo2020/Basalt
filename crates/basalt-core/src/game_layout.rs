//! The STANDARDS.md data layout: which category every path in an installation belongs to.
//!
//! All paths are relative to a root (an installation or a sync folder), so a game behaves the
//! same in every library. Layout paths outside `app` may use `*` (any characters within one
//! name) and `**` (any number of folders), e.g. `data/save/**/DistantHorizons.sqlite*`.
use crate::catalog::GameDefinition;
use crate::error::{CoreError, CoreResult};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

/// Basalt's own per-installation files (sync state, file index, running marker, lock).
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

/// True if a layout path uses `*` or `**`.
pub(crate) fn has_wildcard(path: &str) -> bool {
    path.contains('*')
}

/// Whether `value` is an acceptable layout path: relative, inside the installation, `**` only
/// as a whole component, and no wildcard in the first component (so a pattern can never reach
/// Basalt's own files at the installation root).
pub(crate) fn valid_pattern(value: &str) -> bool {
    let path = Path::new(value);
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return false;
    };
    !value.is_empty()
        && !first.as_bytes().contains(&b'*')
        && path.components().all(|c| match c {
            Component::Normal(name) => {
                let bytes = name.as_bytes();
                bytes == b"**" || !bytes.windows(2).any(|pair| pair == b"**")
            }
            _ => false,
        })
}

/// `*` glob within a single file or folder name.
fn name_matches(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((b'*', rest)) => (0..=name.len()).any(|skip| name_matches(rest, &name[skip..])),
        Some((byte, rest)) => name.first() == Some(byte) && name_matches(rest, &name[1..]),
    }
}

/// Whether `pattern` matches `path` itself (`exact`) or `path` or one of its ancestors.
fn components_match(pattern: &[&OsStr], path: &[&OsStr], exact: bool) -> bool {
    match pattern.split_first() {
        None => !exact || path.is_empty(),
        Some((first, rest)) if first.as_bytes() == b"**" => {
            (0..=path.len()).any(|skip| components_match(rest, &path[skip..], exact))
        }
        Some((first, rest)) => path.split_first().is_some_and(|(name, remaining)| {
            name_matches(first.as_bytes(), name.as_bytes())
                && components_match(rest, remaining, exact)
        }),
    }
}

fn parts(path: &Path) -> Vec<&OsStr> {
    path.components().map(|c| c.as_os_str()).collect()
}

#[derive(Debug, Clone)]
struct Entry {
    path: PathBuf,
    category: DataCategory,
    /// Literal (non-`**`) components, for "most specific wins".
    specificity: usize,
    wildcard: bool,
}

impl Entry {
    fn contains(&self, relative: &Path) -> bool {
        if self.wildcard {
            components_match(&parts(&self.path), &parts(relative), false)
        } else {
            relative.starts_with(&self.path)
        }
    }

    fn is(&self, relative: &Path) -> bool {
        if self.wildcard {
            components_match(&parts(&self.path), &parts(relative), true)
        } else {
            relative == self.path
        }
    }

    /// The folder to walk to find everything the entry can match.
    fn walk_root(&self) -> PathBuf {
        self.path
            .components()
            .take_while(|c| !c.as_os_str().as_bytes().contains(&b'*'))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct GameLayout {
    entries: Vec<Entry>,
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
                let path = PathBuf::from(path);
                entries.push(Entry {
                    specificity: path
                        .components()
                        .filter(|c| c.as_os_str().as_bytes() != b"**")
                        .count(),
                    wildcard: has_wildcard(&path.to_string_lossy()),
                    path,
                    category,
                });
            }
        }
        Self { entries }
    }

    /// The category of `relative`: that of the most specific layout path containing it, or
    /// `None` for uncategorized files (preserved on update, never synced or backed up).
    /// Between equally specific paths, a literal one beats a pattern.
    pub fn category_of(&self, relative: &Path) -> Option<DataCategory> {
        self.entries
            .iter()
            .filter(|entry| entry.contains(relative))
            .max_by_key(|entry| (entry.specificity, !entry.wildcard))
            .map(|entry| entry.category)
    }

    /// The first literal path of a category, which the `BASALT_*_DIR` variables point at and
    /// which installs create.
    pub fn primary(&self, category: DataCategory) -> Option<&Path> {
        self.entries
            .iter()
            .find(|entry| entry.category == category && !entry.wildcard)
            .map(|entry| entry.path.as_path())
    }

    pub fn is_layout_root(&self, relative: &Path) -> bool {
        self.entries.iter().any(|entry| entry.is(relative))
    }

    /// True when some literal layout path of `category` lies strictly inside `relative`.
    fn has_nested(&self, relative: &Path, category: DataCategory) -> bool {
        self.entries.iter().any(|entry| {
            entry.category == category
                && entry.path != relative
                && entry.walk_root().starts_with(relative)
        })
    }

    /// Every file and symlink under `root` whose category is in `categories`, relative to `root`.
    /// Directories are traversed but symlinked directories are never followed.
    pub fn collect_files(
        &self,
        root: &Path,
        categories: &[DataCategory],
    ) -> CoreResult<BTreeSet<PathBuf>> {
        let mut walk_roots: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|entry| categories.contains(&entry.category))
            .map(Entry::walk_root)
            .collect();
        walk_roots.sort();
        // Walking a folder covers everything nested in it.
        let mut distinct: Vec<PathBuf> = Vec::new();
        for path in walk_roots {
            if !distinct.iter().any(|kept| path.starts_with(kept)) {
                distinct.push(path);
            }
        }
        let mut files = BTreeSet::new();
        for path in distinct {
            walk(root, &path, &mut |relative| {
                if self
                    .category_of(relative)
                    .is_some_and(|c| categories.contains(&c))
                {
                    files.insert(relative.to_path_buf());
                }
            })?;
        }
        Ok(files)
    }

    /// Files under `root` that are not application files: user data and anything uncategorized.
    /// Basalt's manifest is excluded; `.basalt/` is included.
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
        self.entries.iter().any(|entry| {
            entry.category != DataCategory::App
                && entry.path != relative
                && entry.walk_root().starts_with(relative)
        })
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

/// Creates `directory` and its parents, replacing a plain file or symlink that is in the way
/// (a file that became a directory in the incoming data).
pub(crate) fn ensure_directory(directory: &Path) -> CoreResult<()> {
    if let Ok(metadata) = fs::symlink_metadata(directory) {
        if metadata.is_dir() {
            return Ok(());
        }
        fs::remove_file(directory).map_err(|e| CoreError::io("Replace file", directory, e))?;
    } else if let Some(parent) = directory.parent() {
        ensure_directory(parent)?;
    }
    match fs::create_dir(directory) {
        // Another copy worker created it first.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && directory.is_dir() => Ok(()),
        other => other.map_err(|e| CoreError::io("Create directory", directory, e)),
    }
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
    fn wildcards_match_names_and_any_depth() {
        let game = standard_game(json!({"layout": {
            "cache": ["data/cache", "data/save/**/DistantHorizons.sqlite*", "data/save/*/logs"]
        }}));
        let layout = GameLayout::for_game(&game);
        let category = |path: &str| layout.category_of(Path::new(path));
        for cached in [
            "data/save/saves/World/data/DistantHorizons.sqlite",
            "data/save/saves/World/data/DistantHorizons.sqlite-wal",
            "data/save/saves/World/DIM-1/data/DistantHorizons.sqlite",
            "data/save/saves/W/dimensions/minecraft/caves/data/DistantHorizons.sqlite",
            "data/save/DistantHorizons.sqlite",
            "data/save/saves/logs/latest.log",
        ] {
            assert_eq!(category(cached), Some(DataCategory::Cache), "{cached}");
        }
        for saved in [
            "data/save/saves/World/level.dat",
            "data/save/saves/World/data/DistantHorizons.toml",
            "data/save/saves/World/region/r.0.0.mca",
            "data/save/saves/x/y/logs",
        ] {
            assert_eq!(category(saved), Some(DataCategory::Save), "{saved}");
        }
        assert!(layout.is_layout_root(Path::new("data/save/saves/logs")));
        assert!(!layout.is_layout_root(Path::new("data/save/saves")));
        assert_eq!(layout.primary(DataCategory::Cache), Some(Path::new("data/cache")));
    }

    #[test]
    fn pattern_validation() {
        for valid in ["data/save", "data/save/**/x.db*", "data/*/logs", "data/**"] {
            assert!(valid_pattern(valid), "{valid}");
        }
        for invalid in ["", "*", "**/x", "*.db", "data/a**b", "/abs", "data/../x", "./data"] {
            assert!(!valid_pattern(invalid), "{invalid}");
        }
    }

    #[test]
    fn collect_files_respects_nested_categories_and_overrides() {
        let game = standard_game(json!({"layout": {
            "save": ["run/saves", "run/mods"],
            "config": ["run/options.txt"],
            "cache": ["run/saves/logs", "run/saves/**/*.sqlite"]
        }}));
        let layout = GameLayout::for_game(&game);
        let root = temp_root("collect");
        write(&root.join("run/saves/world/level.dat"), "w");
        write(&root.join("run/saves/world/data/lod.sqlite"), "lod");
        write(&root.join("run/saves/logs/latest.log"), "log");
        write(&root.join("run/mods/a.jar"), "m");
        write(&root.join("run/options.txt"), "o");
        write(&root.join("run/other.txt"), "x");
        let saves = layout.collect_files(&root, &[DataCategory::Save]).unwrap();
        let names: Vec<_> = saves.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["run/mods/a.jar", "run/saves/world/level.dat"]);
        let cache = layout.collect_files(&root, &[DataCategory::Cache]).unwrap();
        assert_eq!(cache.len(), 2);
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
}
