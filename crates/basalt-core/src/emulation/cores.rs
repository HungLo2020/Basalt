use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use super::paths;
use super::runtime::RuntimeCommand;
use super::systems::EmulatorSystemSpec;
use crate::download::download_to_writer;
use crate::error::{CoreError, CoreResult};
use crate::progress::Progress;

/// The installed core for `core_spec`, without downloading it. Used when launching games.
pub(super) fn installed_core_path(core_spec: &EmulatorSystemSpec) -> CoreResult<PathBuf> {
    let core_path = paths::retroarch_cores_dir()?.join(core_spec.core_file);
    if core_path.is_file() {
        Ok(core_path)
    } else {
        Err(CoreError::EmulatorCoreMissing {
            system: core_spec.system_key.to_string(),
            name: core_spec.short_name.to_string(),
        })
    }
}

/// The installed core for `core_spec`, downloading it when missing. Only for explicit install
/// actions.
pub(super) fn ensure_core_installed(
    core_spec: &EmulatorSystemSpec,
    _runtime_command: &RuntimeCommand,
    progress: &Progress,
) -> CoreResult<PathBuf> {
    let cores_dir = paths::retroarch_cores_dir()?;
    let core_path = cores_dir.join(core_spec.core_file);
    if core_path.exists() {
        return Ok(core_path);
    }

    // Downloaded into memory, so cancelling before extraction leaves nothing behind.
    let mut archive_bytes = Vec::new();
    download_to_writer(
        core_spec.archive_url,
        "Basalt-Emulation-Installer",
        &mut archive_bytes,
        progress,
        &format!("Downloading {} core", core_spec.short_name),
    )?;
    progress.check_cancelled()?;

    progress.report_step(format!("Installing {} core", core_spec.short_name));
    extract_zip(&archive_bytes, &cores_dir).map_err(|error| {
        CoreError::new(format!(
            "Failed to extract {}: {}",
            core_spec.archive_url, error
        ))
    })?;

    if core_path.exists() {
        Ok(core_path)
    } else {
        Err(CoreError::new(format!(
            "Core installation did not produce expected file: {}",
            core_path.display()
        )))
    }
}

/// Extracts every entry of an in-memory ZIP into `destination`. Entry paths that would escape
/// `destination` are rejected by `ZipArchive::extract`.
fn extract_zip(archive_bytes: &[u8], destination: &Path) -> CoreResult<()> {
    fs::create_dir_all(destination).map_err(|error| CoreError::new(error.to_string()))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(archive_bytes))
        .map_err(|error| CoreError::new(error.to_string()))?;
    archive
        .extract(destination)
        .map_err(|error| CoreError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn extract_zip_writes_archive_entries() {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            writer
                .start_file::<_, ()>("test_libretro.so", zip::write::FileOptions::default())
                .unwrap();
            writer.write_all(b"core bytes").unwrap();
            writer.finish().unwrap();
        }

        let destination =
            std::env::temp_dir().join(format!("basalt-core-extract-test-{}", std::process::id()));
        extract_zip(buffer.get_ref(), &destination).unwrap();

        assert_eq!(
            fs::read(destination.join("test_libretro.so")).unwrap(),
            b"core bytes"
        );
        let _ = fs::remove_dir_all(destination);
    }
}
