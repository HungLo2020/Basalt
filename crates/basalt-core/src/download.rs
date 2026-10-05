//! HTTP downloads that report byte progress and can be cancelled between chunks.

use std::io::{Read, Write};

use crate::error::{CoreError, CoreResult};
use crate::progress::{Progress, ProgressUnit};

const CHUNK_SIZE: usize = 64 * 1024;
/// Report at most every this many bytes, so callbacks stay cheap.
const REPORT_INTERVAL_BYTES: u64 = 256 * 1024;

/// Streams `url` into `writer`, reporting `message` with byte counts. Returns bytes written.
pub(crate) fn download_to_writer(
    url: &str,
    user_agent: &str,
    writer: &mut impl Write,
    progress: &Progress,
    message: &str,
) -> CoreResult<u64> {
    progress.check_cancelled()?;
    progress.report(message, 0, None, ProgressUnit::Bytes);

    let response = ureq::get(url)
        .set("User-Agent", user_agent)
        .call()
        .map_err(|error| CoreError::new(format!("Failed to download {}: {}", url, error)))?;
    let total = response
        .header("Content-Length")
        .and_then(|value| value.trim().parse::<u64>().ok());

    let mut reader = response.into_reader();
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut completed = 0u64;
    let mut last_reported = 0u64;

    loop {
        progress.check_cancelled()?;

        let read = reader
            .read(&mut buffer)
            .map_err(|error| CoreError::new(format!("Failed to read {}: {}", url, error)))?;
        if read == 0 {
            break;
        }

        writer
            .write_all(&buffer[..read])
            .map_err(|error| CoreError::new(format!("Failed to save {}: {}", url, error)))?;
        completed += read as u64;

        if completed - last_reported >= REPORT_INTERVAL_BYTES {
            progress.report(message, completed, total, ProgressUnit::Bytes);
            last_reported = completed;
        }
    }

    progress.report(
        message,
        completed,
        total.or(Some(completed)),
        ProgressUnit::Bytes,
    );
    Ok(completed)
}
