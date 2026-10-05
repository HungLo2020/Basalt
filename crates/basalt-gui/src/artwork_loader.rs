//! Background artwork loading for library tiles.
//!
//! Core decides *which* image file belongs to a game ([`ArtworkRequest`]); this module decides
//! *when* to fetch it, how many downloads to run at once, and turns files into RGBA buffers on
//! worker threads so the UI thread only uploads textures.

use std::collections::{HashMap, HashSet};
use std::thread;

use basalt_core::{ArtworkKind, ArtworkRequest, GameEntry};
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};

use super::image_prep::{self, PreparedArtwork};

const MATTMC_SVG_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../resources/assets/icons/apps/MattMC.svg"
));
const MAX_STEAM_DOWNLOAD_QUEUE: usize = 48;
const MAX_EMULATOR_DOWNLOAD_QUEUE: usize = 48;
const MAX_RESULT_QUEUE: usize = 96;

pub(super) struct ArtworkLoader {
    ready: HashSet<String>,
    missing: HashSet<String>,
    requested_by_kind: HashMap<String, ArtworkKind>,
    metadata_prefetch_queue: Vec<ArtworkRequest>,
    max_pending_steam_downloads: usize,
    max_pending_emulator_downloads: usize,
    steam_download_tx: Sender<ArtworkRequest>,
    emulator_download_tx: Sender<ArtworkRequest>,
    result_rx: Receiver<ArtworkLoadResult>,
}

pub(super) enum ArtworkRequestResult {
    Ready {
        key: String,
        payload: PreparedArtwork,
    },
    Pending {
        key: String,
    },
    Missing {
        key: String,
    },
}

pub(super) enum ArtworkLoadResult {
    Ready {
        key: String,
        payload: PreparedArtwork,
    },
    Missing {
        key: String,
    },
}

impl ArtworkLoader {
    pub(super) fn new() -> Self {
        let host_thread_count = detect_host_thread_count();
        let (steam_worker_count, emulator_worker_count) =
            compute_artwork_worker_counts(host_thread_count);
        let max_pending_steam_downloads = (steam_worker_count * 3).clamp(4, 24);
        let max_pending_emulator_downloads = (emulator_worker_count * 3).clamp(6, 64);

        let (steam_download_tx, steam_download_rx) =
            bounded::<ArtworkRequest>(MAX_STEAM_DOWNLOAD_QUEUE);
        let (emulator_download_tx, emulator_download_rx) =
            bounded::<ArtworkRequest>(MAX_EMULATOR_DOWNLOAD_QUEUE);
        let (result_tx, result_rx) = bounded::<ArtworkLoadResult>(MAX_RESULT_QUEUE);

        spawn_workers(steam_worker_count, &steam_download_rx, &result_tx);
        spawn_workers(emulator_worker_count, &emulator_download_rx, &result_tx);

        Self {
            ready: HashSet::new(),
            missing: HashSet::new(),
            requested_by_kind: HashMap::new(),
            metadata_prefetch_queue: Vec::new(),
            max_pending_steam_downloads,
            max_pending_emulator_downloads,
            steam_download_tx,
            emulator_download_tx,
            result_rx,
        }
    }

    pub(super) fn poll_load_results(&mut self, max_results: usize) -> Vec<ArtworkLoadResult> {
        let mut results = Vec::new();

        while results.len() < max_results {
            let result = match self.result_rx.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            };

            match &result {
                ArtworkLoadResult::Ready { key, .. } => {
                    self.requested_by_kind.remove(key);
                    self.ready.insert(key.clone());
                }
                ArtworkLoadResult::Missing { key } => {
                    self.requested_by_kind.remove(key);
                    self.missing.insert(key.clone());
                }
            }

            results.push(result);
        }

        self.pump_metadata_prefetch_queue();
        results
    }

    pub(super) fn prepare_for_games(&mut self, games: &[GameEntry]) -> HashSet<String> {
        let mut visible_keys = HashSet::new();
        let mut metadata_prefetch_requests = Vec::new();

        for request in games.iter().filter_map(ArtworkRequest::for_game) {
            visible_keys.insert(request.key.clone());

            if is_downloadable(request.kind) {
                metadata_prefetch_requests.push(request);
            }
        }

        self.requested_by_kind
            .retain(|key, _| visible_keys.contains(key));
        self.missing.retain(|key| visible_keys.contains(key));
        self.ready.retain(|key| visible_keys.contains(key));

        self.metadata_prefetch_queue = metadata_prefetch_requests;
        self.pump_metadata_prefetch_queue();
        visible_keys
    }

    pub(super) fn refresh_metadata_for_games(&mut self, games: &[GameEntry]) -> HashSet<String> {
        let _ = basalt_core::clear_artwork_cache();

        self.missing.clear();
        self.ready.clear();
        self.requested_by_kind.clear();
        self.metadata_prefetch_queue.clear();

        self.prepare_for_games(games)
    }

    pub(super) fn request_for_game(&mut self, game: &GameEntry) -> Option<ArtworkRequestResult> {
        let request = ArtworkRequest::for_game(game)?;
        Some(self.request_artwork(request))
    }

    pub(super) fn request_mattmc_artwork(&mut self) -> ArtworkRequestResult {
        self.request_artwork(ArtworkRequest::mattmc())
    }

    fn request_artwork(&mut self, request: ArtworkRequest) -> ArtworkRequestResult {
        if self.ready.contains(&request.key) {
            return ArtworkRequestResult::Pending { key: request.key };
        }

        if let Some(local_artwork_path) = request.local_override_path() {
            if let Some(payload) =
                image_prep::prepare_artwork_payload_from_path(&local_artwork_path)
            {
                self.missing.remove(&request.key);
                self.ready.insert(request.key.clone());
                return ArtworkRequestResult::Ready {
                    key: request.key,
                    payload,
                };
            }
        }

        if request.kind == ArtworkKind::Mattmc {
            if let Some(payload) =
                image_prep::prepare_artwork_payload_from_svg_bytes(MATTMC_SVG_BYTES)
            {
                self.ready.insert(request.key.clone());
                return ArtworkRequestResult::Ready {
                    key: request.key,
                    payload,
                };
            }
        }

        let key = request.key.clone();
        if self.request_download(request) {
            ArtworkRequestResult::Pending { key }
        } else {
            ArtworkRequestResult::Missing { key }
        }
    }

    /// Queues a background fetch. Returns false only when the queue for this kind is full, so the
    /// caller can retry on a later frame.
    fn request_download(&mut self, request: ArtworkRequest) -> bool {
        if !is_downloadable(request.kind) || request.local_override_path().is_some() {
            return true;
        }

        if self.missing.contains(&request.key) || self.requested_by_kind.contains_key(&request.key)
        {
            return true;
        }

        let pending_count_for_kind = self
            .requested_by_kind
            .values()
            .filter(|kind| **kind == request.kind)
            .count();
        let (max_pending_for_kind, download_tx) = match request.kind {
            ArtworkKind::Steam => (self.max_pending_steam_downloads, &self.steam_download_tx),
            _ => (
                self.max_pending_emulator_downloads,
                &self.emulator_download_tx,
            ),
        };

        // Cached art is cheap to load, so it may bypass the in-flight download limit.
        if pending_count_for_kind >= max_pending_for_kind && request.cached_path().is_none() {
            return false;
        }

        let key = request.key.clone();
        let kind = request.kind;
        match download_tx.try_send(request) {
            Ok(_) => {
                self.requested_by_kind.insert(key, kind);
                true
            }
            Err(TrySendError::Full(_)) => false,
            Err(TrySendError::Disconnected(_)) => {
                self.missing.insert(key);
                true
            }
        }
    }

    fn pump_metadata_prefetch_queue(&mut self) {
        if self.metadata_prefetch_queue.is_empty() {
            return;
        }

        let queued_requests = std::mem::take(&mut self.metadata_prefetch_queue);
        let mut remaining_queue = Vec::new();
        for request in queued_requests {
            if self.missing.contains(&request.key)
                || self.ready.contains(&request.key)
                || self.requested_by_kind.contains_key(&request.key)
            {
                continue;
            }

            if !self.request_download(request.clone()) {
                remaining_queue.push(request);
            }
        }

        self.metadata_prefetch_queue = remaining_queue;
    }
}

fn is_downloadable(kind: ArtworkKind) -> bool {
    matches!(kind, ArtworkKind::Steam | ArtworkKind::Emulator)
}

fn spawn_workers(
    count: usize,
    download_rx: &Receiver<ArtworkRequest>,
    result_tx: &Sender<ArtworkLoadResult>,
) {
    for _ in 0..count {
        let worker_download_rx = download_rx.clone();
        let worker_result_tx = result_tx.clone();

        thread::spawn(move || {
            while let Ok(request) = worker_download_rx.recv() {
                if worker_result_tx.send(load_artwork(request)).is_err() {
                    break;
                }
            }
        });
    }
}

/// Runs on a worker thread: resolve the file (possibly downloading it), then decode and resize.
fn load_artwork(request: ArtworkRequest) -> ArtworkLoadResult {
    match request
        .fetch()
        .and_then(|path| image_prep::prepare_artwork_payload_from_path(&path))
    {
        Some(payload) => ArtworkLoadResult::Ready {
            key: request.key,
            payload,
        },
        None => ArtworkLoadResult::Missing { key: request.key },
    }
}

fn detect_host_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4)
}

fn compute_artwork_worker_counts(host_threads: usize) -> (usize, usize) {
    let clamped_threads = host_threads.max(2);
    let emulator_workers = (clamped_threads / 2).clamp(2, 12);

    let steam_workers = if clamped_threads >= 16 {
        4
    } else if clamped_threads >= 8 {
        3
    } else {
        2
    };

    (steam_workers, emulator_workers)
}
