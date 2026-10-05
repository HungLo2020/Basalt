use std::fs;
use std::io::{copy, Cursor, Read};
use std::path::{Path, PathBuf};

use super::paths;
use super::runtime::RuntimeCommand;
use crate::emulator_systems::EmulatorSystemSpec;

pub(super) fn ensure_core_installed(
    core_spec: &EmulatorSystemSpec,
    _runtime_command: &RuntimeCommand,
) -> Result<PathBuf, String> {
    let cores_dir = paths::retroarch_cores_dir()?;
    let core_path = cores_dir.join(core_spec.core_file);
    if core_path.exists() {
        return Ok(core_path);
    }

    let archive_bytes = download_bytes(core_spec.archive_url)?;
    extract_zip(&archive_bytes, &cores_dir)
        .map_err(|error| format!("Failed to extract {}: {}", core_spec.archive_url, error))?;

    if core_path.exists() {
        Ok(core_path)
    } else {
        Err(format!(
            "Core installation did not produce expected file: {}",
            core_path.display()
        ))
    }
}

fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .set("User-Agent", "Basalt-Emulation-Installer")
        .call()
        .map_err(|error| format!("Failed to download {}: {}", url, error))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Failed to read {}: {}", url, error))?;
    Ok(bytes)
}

/// Extracts every entry of an in-memory ZIP into `destination`. Entry paths that would escape
/// `destination` are rejected by `ZipArchive::extract`.
fn extract_zip(archive_bytes: &[u8], destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let mut archive =
        zip::ZipArchive::new(Cursor::new(archive_bytes)).map_err(|error| error.to_string())?;
    archive
        .extract(destination)
        .map_err(|error| error.to_string())
}

pub(super) fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    let response = ureq::get(url)
        .set("User-Agent", "Basalt-Emulation-Installer")
        .call()
        .map_err(|error| format!("Failed to download {}: {}", url, error))?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create download directory: {}", error))?;
    }

    let mut reader = response.into_reader();
    let mut file = fs::File::create(destination)
        .map_err(|error| format!("Failed to create file {}: {}", destination.display(), error))?;
    copy(&mut reader, &mut file)
        .map_err(|error| format!("Failed to save {}: {}", destination.display(), error))?;

    Ok(())
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
