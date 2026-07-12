use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use crate::core::settings;

use super::is_save_sync_supported_for_system;
use super::paths;

pub struct RomSyncReport {
    pub copied: usize,
    pub unchanged: usize,
    pub deleted: usize,
}

#[derive(Clone, Copy)]
enum RomSyncDirection {
    Up,
    Down,
}

pub(super) fn sync_roms_up_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync_roms_for_system(system, RomSyncDirection::Up)
}

pub(super) fn sync_roms_down_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync_roms_for_system(system, RomSyncDirection::Down)
}

pub(super) fn sync_saves_up_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync_saves_for_system(system, RomSyncDirection::Up)
}

pub(super) fn sync_saves_down_for_system(system: &str) -> Result<RomSyncReport, String> {
    sync_saves_for_system(system, RomSyncDirection::Down)
}

fn sync_roms_for_system(
    system: &str,
    direction: RomSyncDirection,
) -> Result<RomSyncReport, String> {
    let system_key = paths::normalize_system_key(system)?;
    let remote_paths = settings::load_emulation_remote_paths()?;

    let local_dir = paths::roms_root_dir()?.join(&system_key);
    let remote_dir = Path::new(&remote_paths.roms_root_dir).join(&system_key);

    let (source_dir, destination_dir) = match direction {
        RomSyncDirection::Up => (local_dir, remote_dir),
        RomSyncDirection::Down => (remote_dir, local_dir),
    };

    if !source_dir.exists() || !source_dir.is_dir() {
        return Err(format!(
            "Source ROM directory does not exist: {}",
            source_dir.display()
        ));
    }

    sync_directory_contents(&source_dir, &destination_dir, "ROM", |_: &Path| true)
}

fn sync_saves_for_system(
    system: &str,
    direction: RomSyncDirection,
) -> Result<RomSyncReport, String> {
    let system_key = paths::normalize_system_key(system)?;
    if !is_save_sync_supported_for_system(&system_key) {
        return Err(format!(
            "Save sync is not supported for system: {}",
            system_key
        ));
    }

    let remote_paths = settings::load_emulation_remote_paths()?;

    let local_dir = paths::saves_root_dir()?.join(&system_key);
    let remote_dir = Path::new(&remote_paths.saves_root_dir).join(&system_key);

    fs::create_dir_all(&local_dir).map_err(|error| {
        format!(
            "Failed to create local save directory {}: {}",
            local_dir.display(),
            error
        )
    })?;

    fs::create_dir_all(&remote_dir).map_err(|error| {
        format!(
            "Failed to create remote save directory {}: {}",
            remote_dir.display(),
            error
        )
    })?;

    let (source_dir, destination_dir) = match direction {
        RomSyncDirection::Up => (local_dir, remote_dir),
        RomSyncDirection::Down => (remote_dir, local_dir),
    };

    sync_directory_contents(&source_dir, &destination_dir, "save", is_syncable_save_file)
}

fn sync_directory_contents<F>(
    source_dir: &Path,
    destination_dir: &Path,
    file_label: &str,
    include_file: F,
) -> Result<RomSyncReport, String>
where
    F: Fn(&Path) -> bool,
{
    fs::create_dir_all(destination_dir).map_err(|error| {
        format!(
            "Failed to create destination {} directory {}: {}",
            file_label,
            destination_dir.display(),
            error
        )
    })?;

    let mut source_files = Vec::new();
    collect_files_recursive(source_dir, &mut source_files)?;
    source_files.retain(|path| include_file(path));

    let mut destination_files = Vec::new();
    collect_files_recursive(destination_dir, &mut destination_files)?;
    destination_files.retain(|path| include_file(path));

    let mut source_relative_paths = HashSet::new();

    let mut copied = 0usize;
    let mut unchanged = 0usize;
    let mut deleted = 0usize;

    for source_file in source_files {
        let relative_path = source_file.strip_prefix(source_dir).map_err(|error| {
            format!(
                "Failed to compute relative {} path for {}: {}",
                file_label,
                source_file.display(),
                error
            )
        })?;

        source_relative_paths.insert(relative_path.to_path_buf());

        let destination_file = destination_dir.join(relative_path);
        if destination_file.exists() && destination_file.is_dir() {
            fs::remove_dir_all(&destination_file).map_err(|error| {
                format!(
                    "Failed to remove conflicting destination {} directory {}: {}",
                    file_label,
                    destination_file.display(),
                    error
                )
            })?;
        }

        if let Some(parent_dir) = destination_file.parent() {
            fs::create_dir_all(parent_dir).map_err(|error| {
                format!(
                    "Failed to create destination {} subdirectory {}: {}",
                    file_label,
                    parent_dir.display(),
                    error
                )
            })?;
        }

        let should_copy = if destination_file.exists() && destination_file.is_file() {
            !files_identical(&source_file, &destination_file)?
        } else {
            true
        };

        if should_copy {
            fs::copy(&source_file, &destination_file).map_err(|error| {
                format!(
                    "Failed to copy {} {} -> {}: {}",
                    file_label,
                    source_file.display(),
                    destination_file.display(),
                    error
                )
            })?;
            copied += 1;
        } else {
            unchanged += 1;
        }
    }

    for destination_file in destination_files {
        let relative_path = destination_file
            .strip_prefix(destination_dir)
            .map_err(|error| {
                format!(
                    "Failed to compute destination relative {} path for {}: {}",
                    file_label,
                    destination_file.display(),
                    error
                )
            })?;

        if source_relative_paths.contains(relative_path) {
            continue;
        }

        if destination_file.exists() && destination_file.is_file() {
            fs::remove_file(&destination_file).map_err(|error| {
                format!(
                    "Failed to delete destination {} {}: {}",
                    file_label,
                    destination_file.display(),
                    error
                )
            })?;
            deleted += 1;
        }
    }

    remove_empty_subdirectories(destination_dir)?;

    Ok(RomSyncReport {
        copied,
        unchanged,
        deleted,
    })
}

fn is_syncable_save_file(file_path: &Path) -> bool {
    let Some(file_name) = file_path.file_name().and_then(|value| value.to_str()) else {
        return true;
    };

    !file_name.to_lowercase().contains(".state")
}

fn collect_files_recursive(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Failed to read ROM directory {}: {}", root.display(), error))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Failed to read ROM directory entry: {}", error))?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }

    Ok(())
}

fn remove_empty_subdirectories(root: &Path) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Failed to read ROM directory {}: {}", root.display(), error))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Failed to read ROM directory entry: {}", error))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        remove_empty_subdirectories(&path)?;

        let mut sub_entries = fs::read_dir(&path).map_err(|error| {
            format!(
                "Failed to read ROM subdirectory {}: {}",
                path.display(),
                error
            )
        })?;

        if sub_entries.next().is_none() {
            fs::remove_dir(&path).map_err(|error| {
                format!(
                    "Failed to remove empty ROM subdirectory {}: {}",
                    path.display(),
                    error
                )
            })?;
        }
    }

    Ok(())
}

fn files_identical(first: &Path, second: &Path) -> Result<bool, String> {
    let first_metadata = fs::metadata(first)
        .map_err(|error| format!("Failed to stat {}: {}", first.display(), error))?;
    let second_metadata = fs::metadata(second)
        .map_err(|error| format!("Failed to stat {}: {}", second.display(), error))?;

    if first_metadata.len() != second_metadata.len() {
        return Ok(false);
    }

    let first_file = fs::File::open(first)
        .map_err(|error| format!("Failed to open {}: {}", first.display(), error))?;
    let second_file = fs::File::open(second)
        .map_err(|error| format!("Failed to open {}: {}", second.display(), error))?;

    let mut first_reader = BufReader::new(first_file);
    let mut second_reader = BufReader::new(second_file);

    let mut first_buffer = [0u8; 8192];
    let mut second_buffer = [0u8; 8192];

    loop {
        let first_read = first_reader
            .read(&mut first_buffer)
            .map_err(|error| format!("Failed to read {}: {}", first.display(), error))?;
        let second_read = second_reader
            .read(&mut second_buffer)
            .map_err(|error| format!("Failed to read {}: {}", second.display(), error))?;

        if first_read != second_read {
            return Ok(false);
        }

        if first_read == 0 {
            return Ok(true);
        }

        if first_buffer[..first_read] != second_buffer[..second_read] {
            return Ok(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn save_state_files_are_excluded_from_save_sync() {
        assert!(is_syncable_save_file(Path::new("Pokemon.srm")));
        assert!(is_syncable_save_file(Path::new("nested/Pokemon.save")));
        assert!(!is_syncable_save_file(Path::new("Pokemon.state")));
        assert!(!is_syncable_save_file(Path::new("Pokemon.State1")));
        assert!(!is_syncable_save_file(Path::new("Pokemon.auto.state")));
    }

    #[test]
    fn sync_directory_copies_updates_deletes_and_ignores_filtered_files() {
        let temp_root = unique_temp_dir("basalt-sync-test");
        let source = temp_root.join("source");
        let destination = temp_root.join("destination");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::create_dir_all(destination.join("nested")).unwrap();

        fs::write(source.join("same.srm"), "same").unwrap();
        fs::write(destination.join("same.srm"), "same").unwrap();
        fs::write(source.join("changed.srm"), "new").unwrap();
        fs::write(destination.join("changed.srm"), "old").unwrap();
        fs::write(source.join("nested").join("added.srm"), "added").unwrap();
        fs::write(destination.join("deleted.srm"), "delete me").unwrap();
        fs::write(source.join("ignored.state"), "source ignored").unwrap();
        fs::write(destination.join("ignored.state"), "destination ignored").unwrap();

        let report =
            sync_directory_contents(&source, &destination, "save", is_syncable_save_file).unwrap();

        assert_eq!(report.copied, 2);
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.deleted, 1);
        assert_eq!(
            fs::read_to_string(destination.join("changed.srm")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read_to_string(destination.join("nested").join("added.srm")).unwrap(),
            "added"
        );
        assert!(!destination.join("deleted.srm").exists());
        assert_eq!(
            fs::read_to_string(destination.join("ignored.state")).unwrap(),
            "destination ignored"
        );

        let _ = fs::remove_dir_all(temp_root);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
