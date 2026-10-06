//! Resolves game artwork to image URLs that QML `Image` items load and decode themselves.
//!
//! Core decides which file belongs to a game ([`ArtworkRequest`]). Here, overrides and cached
//! files are looked up on one background thread, and only genuinely missing artwork is handed
//! to a small pool of download workers. Every batch carries a generation number so results
//! from before a refresh are ignored.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use basalt_core::{ArtworkKind, ArtworkRequest};
use crossbeam_channel::{unbounded, Sender};

/// Bundled with the QML module (see build.rs).
pub const MATTMC_ARTWORK_URL: &str = "qrc:/qt/qml/org/basalt/app/assets/MattMC.svg";
const DOWNLOAD_WORKERS: usize = 4;

/// Called with (generation, artwork key, image URL) from a background thread.
pub type ResolvedCallback = Arc<dyn Fn(u64, String, String) + Send + Sync>;

pub struct ArtworkResolver {
    download_tx: Sender<(u64, ArtworkRequest)>,
    generation: Arc<AtomicU64>,
    on_resolved: ResolvedCallback,
}

impl ArtworkResolver {
    pub fn new(on_resolved: ResolvedCallback) -> Self {
        let (download_tx, download_rx) = unbounded::<(u64, ArtworkRequest)>();
        let generation = Arc::new(AtomicU64::new(0));

        for _ in 0..DOWNLOAD_WORKERS {
            let download_rx = download_rx.clone();
            let generation = Arc::clone(&generation);
            let on_resolved = Arc::clone(&on_resolved);
            thread::spawn(move || {
                while let Ok((batch, request)) = download_rx.recv() {
                    // Skip work queued before a refresh.
                    if batch != generation.load(Ordering::Relaxed) {
                        continue;
                    }
                    if let Some(path) = request.fetch() {
                        on_resolved(batch, request.key, file_url(&path));
                    }
                }
            });
        }

        Self {
            download_tx,
            generation,
            on_resolved,
        }
    }

    /// Starts resolving artwork for `requests`, superseding any earlier batch. Returns the new
    /// generation; results tagged with older generations should be discarded.
    pub fn resolve_all(&self, requests: Vec<ArtworkRequest>) -> u64 {
        let batch = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let download_tx = self.download_tx.clone();
        let generation = Arc::clone(&self.generation);
        let on_resolved = Arc::clone(&self.on_resolved);

        thread::spawn(move || {
            for request in requests {
                if batch != generation.load(Ordering::Relaxed) {
                    return;
                }

                if let Some(path) = request.local_override_path() {
                    on_resolved(batch, request.key, file_url(&path));
                } else if request.kind == ArtworkKind::Mattmc {
                    on_resolved(batch, request.key, MATTMC_ARTWORK_URL.to_string());
                } else if let Some(path) = request.cached_path() {
                    on_resolved(batch, request.key, file_url(&path));
                } else {
                    let _ = download_tx.send((batch, request));
                }
            }
        });

        batch
    }
}

/// `file://` URL for an absolute local path. Characters that would otherwise start a URL query
/// or fragment are escaped.
pub fn file_url(path: &Path) -> String {
    let path = path.to_string_lossy();
    let mut escaped = String::with_capacity(path.len());
    for character in path.chars() {
        match character {
            '%' => escaped.push_str("%25"),
            '#' => escaped.push_str("%23"),
            '?' => escaped.push_str("%3F"),
            _ => escaped.push(character),
        }
    }

    format!("file://{}", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_escape_special_characters() {
        assert_eq!(
            file_url(Path::new("/home/me/Pokemon Radical Red.jpeg")),
            "file:///home/me/Pokemon Radical Red.jpeg"
        );
        assert_eq!(
            file_url(Path::new("/art/Sonic #2? 100%.png")),
            "file:///art/Sonic %232%3F 100%25.png"
        );
    }
}
