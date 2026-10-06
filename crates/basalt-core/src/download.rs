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
        .header("User-Agent", user_agent)
        .call()
        .map_err(|error| CoreError::new(format!("Failed to download {}: {}", url, error)))?;
    let total = response.body().content_length();

    let mut reader = response.into_body().into_reader();
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

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::progress::CancelToken;

    /// Serves one HTTP response of `size` bytes from localhost, in `chunk`-sized writes with
    /// `delay` between them (so a test can cancel mid-download). Returns the URL.
    fn serve_once(size: usize, chunk: usize, delay: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file.bin", listener.local_addr().unwrap());

        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            // Skip the request headers.
            while reader.read_line(&mut line).is_ok() && line != "\r\n" && !line.is_empty() {
                line.clear();
            }

            let mut stream = stream;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                size
            );
            if stream.write_all(header.as_bytes()).is_err() {
                return;
            }
            let block = vec![7u8; chunk];
            let mut sent = 0;
            while sent < size {
                let n = chunk.min(size - sent);
                // The client hanging up (after a cancel) ends the response.
                if stream.write_all(&block[..n]).is_err() {
                    return;
                }
                sent += n;
                thread::sleep(delay);
            }
        });

        url
    }

    #[test]
    fn downloads_everything_and_reports_byte_progress() {
        let size = 300 * 1024;
        let url = serve_once(size, 64 * 1024, Duration::ZERO);
        let updates = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&updates);
        let progress = Progress::new(
            move |update| sink.lock().unwrap().push((update.completed, update.total)),
            CancelToken::new(),
        );

        let mut body = Vec::new();
        let written =
            download_to_writer(&url, "basalt-test", &mut body, &progress, "Test").unwrap();

        assert_eq!(written, size as u64);
        assert_eq!(body.len(), size);
        let updates = updates.lock().unwrap();
        assert_eq!(updates.first(), Some(&(0, None)));
        assert_eq!(updates.last(), Some(&(size as u64, Some(size as u64))));
    }

    #[test]
    fn cancelling_mid_download_stops_with_cancelled() {
        let size = 4 * 1024 * 1024;
        // ~64 chunks with 10 ms gaps: the whole body would take over half a second.
        let url = serve_once(size, 64 * 1024, Duration::from_millis(10));
        let cancel = CancelToken::new();
        let trigger = cancel.clone();
        let progress = Progress::new(
            move |update| {
                if update.completed >= REPORT_INTERVAL_BYTES {
                    trigger.cancel();
                }
            },
            cancel,
        );

        let mut body = Vec::new();
        let result = download_to_writer(&url, "basalt-test", &mut body, &progress, "Test");

        assert!(matches!(result, Err(CoreError::Cancelled)), "{:?}", result);
        assert!(
            body.len() < size,
            "download should stop early, got {} of {} bytes",
            body.len(),
            size
        );
    }

    #[test]
    fn cancelled_before_start_makes_no_request() {
        let cancel = CancelToken::new();
        cancel.cancel();
        let progress = Progress::new(|_| {}, cancel);

        // Nothing listens on this port; reaching the network would be a different error.
        let result = download_to_writer(
            "http://127.0.0.1:9/unused",
            "basalt-test",
            &mut Vec::new(),
            &progress,
            "Test",
        );
        assert!(matches!(result, Err(CoreError::Cancelled)), "{:?}", result);
    }
}
