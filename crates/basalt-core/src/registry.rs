use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::error::{CoreError, CoreResult};
use super::runners::RunnerKind;
use super::storage;
use super::GameEntry;

const REGISTRY_FILE_NAME: &str = "games.tsv";
const BLACKLIST_FILE_NAME: &str = "blacklist.txt";
const DEFAULT_BLACKLIST_TEMPLATE: &str =
    "# Basalt blacklist\n# One game name per line.\n# Lines starting with # are ignored.\n";
const BUNDLED_BLACKLIST: &str = include_str!("../../../resources/blacklist.txt");

fn get_registry_path() -> CoreResult<PathBuf> {
    Ok(storage::data_dir()?.join(REGISTRY_FILE_NAME))
}

fn get_blacklist_path() -> CoreResult<PathBuf> {
    Ok(storage::data_dir()?.join(BLACKLIST_FILE_NAME))
}

fn ensure_blacklist_file() -> CoreResult<()> {
    let blacklist_path = get_blacklist_path()?;

    if blacklist_path.exists() {
        return Ok(());
    }

    storage::write_atomic(&blacklist_path, BUNDLED_BLACKLIST.as_bytes())
}

pub(super) fn load_blacklisted_names() -> CoreResult<HashSet<String>> {
    ensure_blacklist_file()?;

    let blacklist_path = get_blacklist_path()?;
    let mut names = read_blacklisted_names_from_file(&blacklist_path)?;
    if !names.is_empty() {
        return Ok(names);
    }

    let existing_contents = fs::read_to_string(&blacklist_path).unwrap_or_default();
    if existing_contents.trim() == DEFAULT_BLACKLIST_TEMPLATE.trim()
        && has_active_blacklist_entries(BUNDLED_BLACKLIST)
    {
        storage::write_atomic(&blacklist_path, BUNDLED_BLACKLIST.as_bytes())?;
        names = read_blacklisted_names_from_file(&blacklist_path)?;
    }

    Ok(names)
}

fn read_blacklisted_names_from_file(path: &Path) -> CoreResult<HashSet<String>> {
    let file = fs::File::open(path).map_err(|err| CoreError::io("Failed to open", path, err))?;

    let reader = BufReader::new(file);
    let mut names = HashSet::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|err| CoreError::io("Failed to read", path, err))?;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        names.insert(trimmed.to_lowercase());
    }

    Ok(names)
}

fn has_active_blacklist_entries(contents: &str) -> bool {
    contents
        .lines()
        .map(|line| line.trim())
        .any(|trimmed| !trimmed.is_empty() && !trimmed.starts_with('#'))
}

pub(super) fn load_entries() -> CoreResult<Vec<GameEntry>> {
    let registry_path = get_registry_path()?;
    if !registry_path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&registry_path)
        .map_err(|err| CoreError::io("Failed to open", &registry_path, err))?;

    parse_entries_from_reader(BufReader::new(file))
        .map_err(|err| CoreError::io("Failed to read", &registry_path, err))
}

/// Callers doing read-modify-write must hold [`storage::with_data_lock`].
pub(super) fn save_entries(entries: &[GameEntry]) -> CoreResult<()> {
    storage::write_atomic(&get_registry_path()?, serialize_entries(entries).as_bytes())
}

fn parse_entries_from_reader<R: BufRead>(reader: R) -> std::io::Result<Vec<GameEntry>> {
    let mut entries = Vec::new();

    for line_result in reader.lines() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.splitn(3, '\t');
        let name = parts.next().unwrap_or_default().to_string();
        let second = parts.next().unwrap_or_default().to_string();
        let third = parts.next().unwrap_or_default().to_string();

        if !name.is_empty() {
            if !third.is_empty() {
                if let Some(runner_kind) = RunnerKind::parse(&second) {
                    entries.push(GameEntry {
                        name,
                        runner_kind,
                        launch_target: third,
                    });
                }
            } else if !second.is_empty() {
                entries.push(GameEntry {
                    name,
                    runner_kind: RunnerKind::Bash,
                    launch_target: second,
                });
            }
        }
    }

    Ok(entries)
}

fn serialize_entries(entries: &[GameEntry]) -> String {
    let mut serialized = String::new();
    for entry in entries {
        serialized.push_str(&format!(
            "{}\t{}\t{}\n",
            entry.name,
            entry.runner_kind.as_str(),
            entry.launch_target
        ));
    }
    serialized
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn registry_parser_supports_current_and_legacy_rows() {
        let contents = "\
MattMC\tbash\t/home/me/Games/MattMC/run-mattmc.sh
Legacy Bash\t/home/me/game.sh
Steam Game\tsteam\t12345
Bad Runner\tmissing\tignored
\tsteam\tno-name

";

        let entries = parse_entries_from_reader(Cursor::new(contents)).unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "MattMC");
        assert_eq!(entries[0].runner_kind, RunnerKind::Bash);
        assert_eq!(
            entries[0].launch_target,
            "/home/me/Games/MattMC/run-mattmc.sh"
        );
        assert_eq!(entries[1].name, "Legacy Bash");
        assert_eq!(entries[1].runner_kind, RunnerKind::Bash);
        assert_eq!(entries[1].launch_target, "/home/me/game.sh");
        assert_eq!(entries[2].runner_kind, RunnerKind::Steam);
        assert_eq!(entries[2].launch_target, "12345");
    }

    #[test]
    fn registry_serializer_writes_runner_kind_column() {
        let entries = vec![GameEntry {
            name: "Steam Game".to_string(),
            runner_kind: RunnerKind::Steam,
            launch_target: "12345".to_string(),
        }];

        assert_eq!(serialize_entries(&entries), "Steam Game\tsteam\t12345\n");
    }
}
