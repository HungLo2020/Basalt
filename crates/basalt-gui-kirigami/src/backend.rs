//! The `Backend` QML singleton: the only bridge between the QML pages and basalt-core.
//!
//! Lists are exposed as JSON strings that QML parses into JS arrays; every slow core call runs on
//! a worker thread and posts its result back to the Qt thread with `qt_thread().queue`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        // Library data.
        #[qproperty(QString, games_json)]
        #[qproperty(QString, playlists_json)]
        #[qproperty(QString, install_tiles_json)]
        #[qproperty(QString, game_libraries_json)]
        #[qproperty(QString, running_games_json)]
        #[qproperty(bool, loading)]
        // Bumped when artwork URLs or installed-core state change, so QML bindings re-evaluate.
        #[qproperty(i32, artwork_revision)]
        // Status lines, one per screen.
        #[qproperty(QString, library_status)]
        #[qproperty(QString, install_status)]
        #[qproperty(QString, settings_status)]
        // The single background job slot.
        #[qproperty(bool, job_active)]
        #[qproperty(QString, job_message)]
        #[qproperty(f64, job_fraction)]
        #[qproperty(bool, job_cancellable)]
        #[qproperty(bool, job_cancelling)]
        // Settings.
        #[qproperty(bool, fullscreen)]
        #[qproperty(bool, maximized)]
        #[qproperty(i32, backup_keep)]
        // The configured sync folder, or "" when the default is used.
        #[qproperty(QString, sync_remote)]
        #[qproperty(QString, default_sync_remote)]
        // Catalog id of a game whose last sync found changes on both sides, or "".
        #[qproperty(QString, sync_conflict_game)]
        // Basalt itself is updated by the system package manager; shown in Settings.
        #[qproperty(QString, app_version)]
        #[qproperty(bool, controller_connected)]
        type Backend = super::BackendRust;

        /// Loads data and starts background services. Called once by Main.qml.
        #[qinvokable]
        fn initialize(self: Pin<&mut Backend>);

        #[qinvokable]
        fn refresh_games(self: Pin<&mut Backend>);
        #[qinvokable]
        fn discover(self: Pin<&mut Backend>);
        #[qinvokable]
        fn refresh_metadata(self: Pin<&mut Backend>);

        #[qinvokable]
        fn launch_game(self: Pin<&mut Backend>, name: &QString);
        #[qinvokable]
        fn set_favorite(self: Pin<&mut Backend>, name: &QString, favorite: bool);
        #[qinvokable]
        fn remove_game(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        fn maintain_game(
            self: Pin<&mut Backend>,
            game: &QString,
            action: &QString,
            target: &QString,
        );
        #[qinvokable]
        fn restore_backup(self: Pin<&mut Backend>, game: &QString, snapshot: &QString);
        /// JSON list of a game's backups, newest first.
        #[qinvokable]
        fn backups_json(self: &Backend, game: &QString) -> QString;
        #[qinvokable]
        fn clear_sync_conflict(self: Pin<&mut Backend>);
        #[qinvokable]
        fn install_game(self: Pin<&mut Backend>, game: &QString, library: &QString);
        #[qinvokable]
        fn add_game_library(self: Pin<&mut Backend>, id: &QString, path: &QString);
        #[qinvokable]
        fn refresh_game_libraries(self: Pin<&mut Backend>);
        #[qinvokable]
        fn cancel_job(self: Pin<&mut Backend>);

        #[qinvokable]
        fn set_display_mode(self: Pin<&mut Backend>, fullscreen: bool, maximized: bool);
        /// Sets the sync folder; an empty path clears it.
        #[qinvokable]
        fn save_sync_remote(self: Pin<&mut Backend>, path: &QString);
        #[qinvokable]
        fn save_backup_keep(self: Pin<&mut Backend>, keep: i32);

        /// Image URL for an artwork key, or "" while unresolved.
        #[qinvokable]
        fn artwork_url(self: &Backend, key: &QString) -> QString;

        /// Gamepad navigation: move the library selection by (columns, rows).
        #[qsignal]
        fn controller_navigate(self: Pin<&mut Backend>, dx: i32, dy: i32);
        #[qsignal]
        fn controller_activate(self: Pin<&mut Backend>);
    }

    impl cxx_qt::Threading for Backend {}
}

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::Arc;
use std::thread;

use basalt_core::{
    ArtworkRequest, CancelToken, CoreError, CoreResult, DiscoverReport, DiscoverResult, GameEntry,
    Playlist, Progress,
};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::artwork::ArtworkResolver;
use crate::controller::{self, ControllerCommand};
use crate::view_model;

#[derive(Clone, Copy)]
enum StatusTarget {
    Library,
    Install,
}

impl StatusTarget {
    fn parse(value: &str) -> Self {
        if value == "install" {
            Self::Install
        } else {
            Self::Library
        }
    }
}

pub struct BackendRust {
    games_json: QString,
    playlists_json: QString,
    install_tiles_json: QString,
    game_libraries_json: QString,
    running_games_json: QString,
    loading: bool,
    artwork_revision: i32,
    library_status: QString,
    install_status: QString,
    settings_status: QString,
    job_active: bool,
    job_message: QString,
    job_fraction: f64,
    job_cancellable: bool,
    job_cancelling: bool,
    fullscreen: bool,
    maximized: bool,
    backup_keep: i32,
    sync_remote: QString,
    default_sync_remote: QString,
    sync_conflict_game: QString,
    app_version: QString,
    controller_connected: bool,

    initialized: bool,
    games: Vec<GameEntry>,
    playlists: Vec<Playlist>,
    running: HashSet<String>,
    artwork_urls: HashMap<String, String>,
    artwork_generation: u64,
    artwork: Option<ArtworkResolver>,
    job_cancel: Option<CancelToken>,
}

impl Default for BackendRust {
    fn default() -> Self {
        Self {
            games_json: QString::from("[]"),
            playlists_json: QString::from("[]"),
            install_tiles_json: QString::from(view_model::install_tiles_json().as_str()),
            game_libraries_json: QString::from("[]"),
            running_games_json: QString::from("[]"),
            loading: true,
            artwork_revision: 0,
            library_status: QString::from("Loading games..."),
            install_status: QString::default(),
            settings_status: QString::default(),
            job_active: false,
            job_message: QString::default(),
            job_fraction: -1.0,
            job_cancellable: false,
            job_cancelling: false,
            fullscreen: false,
            maximized: false,
            backup_keep: basalt_core::DEFAULT_BACKUP_KEEP as i32,
            sync_remote: QString::default(),
            default_sync_remote: QString::from(basalt_core::DEFAULT_SYNC_REMOTE),
            sync_conflict_game: QString::default(),
            app_version: QString::from(env!("CARGO_PKG_VERSION")),
            controller_connected: false,

            initialized: false,
            games: Vec::new(),
            playlists: Vec::new(),
            running: HashSet::new(),
            artwork_urls: HashMap::new(),
            artwork_generation: 0,
            artwork: None,
            job_cancel: None,
        }
    }
}

fn qs(value: impl AsRef<str>) -> QString {
    QString::from(value.as_ref())
}

impl qobject::Backend {
    // ----- startup -------------------------------------------------------------------------

    fn initialize(mut self: Pin<&mut Self>) {
        if self.rust().initialized {
            return;
        }
        self.as_mut().rust_mut().initialized = true;

        self.as_mut().load_settings();
        self.as_mut().start_artwork_resolver();
        self.as_mut().start_controller();
        self.as_mut().load_games_in_background();
    }

    fn load_settings(mut self: Pin<&mut Self>) {
        let mut warnings = Vec::new();


        let display = basalt_core::load_launcher_display_settings().unwrap_or_else(|error| {
            warnings.push(format!("Display setting load warning: {}", error));
            basalt_core::LauncherDisplaySettings {
                fullscreen_enabled: false,
                maximized_enabled: false,
            }
        });
        self.as_mut().set_fullscreen(display.fullscreen_enabled);
        self.as_mut().set_maximized(display.maximized_enabled);
        match basalt_core::load_sync_remote() {
            Ok(remote) if remote.is_default => self.as_mut().set_sync_remote(QString::default()),
            Ok(remote) => self
                .as_mut()
                .set_sync_remote(qs(remote.path.to_string_lossy())),
            Err(error) => warnings.push(format!("Sync setting load warning: {}", error)),
        }
        match basalt_core::load_backup_keep() {
            Ok(keep) => self.as_mut().set_backup_keep(keep as i32),
            Err(error) => warnings.push(format!("Backup setting load warning: {error}")),
        }
        self.as_mut().set_settings_status(qs(warnings.join(" | ")));
    }

    fn start_artwork_resolver(mut self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        let resolver = ArtworkResolver::new(Arc::new(move |generation, key, url| {
            let _ = qt_thread.queue(move |mut backend: Pin<&mut qobject::Backend>| {
                if backend.rust().artwork_generation != generation {
                    return;
                }
                backend.as_mut().rust_mut().artwork_urls.insert(key, url);
                let revision = *backend.artwork_revision() + 1;
                backend.as_mut().set_artwork_revision(revision);
            });
        }));
        self.as_mut().rust_mut().artwork = Some(resolver);
    }

    fn start_controller(mut self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        let connected = controller::spawn(move |command| {
            let _ = qt_thread.queue(move |backend: Pin<&mut qobject::Backend>| match command {
                ControllerCommand::Navigate(dx, dy) => backend.controller_navigate(dx, dy),
                ControllerCommand::Activate => backend.controller_activate(),
            });
        });
        self.as_mut().set_controller_connected(connected);
    }

    fn load_games_in_background(self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        thread::spawn(move || {
            let games = basalt_core::list_games();
            let playlists = basalt_core::list_playlists();
            let _ = qt_thread.queue(move |mut backend: Pin<&mut qobject::Backend>| {
                backend.as_mut().set_loading(false);
                match games {
                    Ok(games) => {
                        backend.as_mut().set_library_status(QString::default());
                        backend.as_mut().apply_games(games, playlists);
                    }
                    Err(error) => {
                        backend.as_mut().apply_games(Vec::new(), playlists);
                        backend
                            .as_mut()
                            .set_library_status(qs(format!("Failed to load games: {}", error)));
                    }
                }
                // E.g. files that couldn't be moved from the old ~/.basalt directory.
                backend.as_mut().show_core_warnings(StatusTarget::Library);
            });
        });
    }

    // ----- library ---------------------------------------------------------------------------

    fn apply_games(
        mut self: Pin<&mut Self>,
        games: Vec<GameEntry>,
        playlists: CoreResult<Vec<Playlist>>,
    ) {
        let playlists = match playlists {
            Ok(playlists) => playlists,
            Err(error) => {
                self.as_mut()
                    .set_library_status(qs(format!("Failed to load playlists: {}", error)));
                vec![Playlist {
                    name: basalt_core::FAVORITES_PLAYLIST_NAME.to_string(),
                    game_names: Vec::new(),
                }]
            }
        };

        let requests = artwork_requests(&games);

        self.as_mut()
            .set_games_json(qs(view_model::games_json(&games, &playlists)));
        self.as_mut()
            .set_playlists_json(qs(view_model::playlists_json(&playlists)));
        {
            let mut rust = self.as_mut().rust_mut();
            rust.games = games;
            rust.playlists = playlists;
        }

        self.as_mut().resolve_artwork(requests);
    }

    fn resolve_artwork(mut self: Pin<&mut Self>, requests: Vec<ArtworkRequest>) {
        let Some(generation) = self
            .rust()
            .artwork
            .as_ref()
            .map(|resolver| resolver.resolve_all(requests))
        else {
            return;
        };

        let mut rust = self.as_mut().rust_mut();
        rust.artwork_generation = generation;
        // Keep already-known URLs so tiles don't flicker; they are refreshed as results arrive.
    }

    /// Re-reads games and playlists synchronously; both are small local files.
    fn reload_library(mut self: Pin<&mut Self>) {
        match basalt_core::list_games() {
            Ok(games) => {
                let playlists = basalt_core::list_playlists();
                self.as_mut().apply_games(games, playlists);
            }
            Err(error) => {
                self.as_mut()
                    .apply_games(Vec::new(), basalt_core::list_playlists());
                self.as_mut()
                    .set_library_status(qs(format!("Failed to load games: {}", error)));
            }
        }
    }

    fn reload_playlists(mut self: Pin<&mut Self>) {
        let games = self.rust().games.clone();
        let playlists = basalt_core::list_playlists();
        self.as_mut().apply_playlists_only(games, playlists);
    }

    fn apply_playlists_only(
        mut self: Pin<&mut Self>,
        games: Vec<GameEntry>,
        playlists: CoreResult<Vec<Playlist>>,
    ) {
        match playlists {
            Ok(playlists) => {
                self.as_mut()
                    .set_games_json(qs(view_model::games_json(&games, &playlists)));
                self.as_mut()
                    .set_playlists_json(qs(view_model::playlists_json(&playlists)));
                self.as_mut().rust_mut().playlists = playlists;
            }
            Err(error) => {
                self.as_mut()
                    .set_library_status(qs(format!("Failed to load playlists: {}", error)));
            }
        }
    }

    fn refresh_games(mut self: Pin<&mut Self>) {
        self.as_mut().reload_library();
        self.as_mut().set_library_status(qs("Game list refreshed"));
    }

    fn discover(mut self: Pin<&mut Self>) {
        self.as_mut().start_job(
            StatusTarget::Library,
            "Discovering games...".to_string(),
            |_| basalt_core::discover_with_runners(&basalt_core::ALL_DISCOVER_RUNNERS),
            |mut backend, result| {
                let message = match result {
                    Ok(report) => {
                        backend.as_mut().reload_library();
                        discover_summary(&report)
                    }
                    Err(error) => format!("Discover failed: {}", error),
                };
                backend.as_mut().set_library_status(qs(message));
            },
        );
    }

    fn refresh_metadata(mut self: Pin<&mut Self>) {
        let _ = basalt_core::clear_artwork_cache();
        self.as_mut().rust_mut().artwork_urls.clear();
        let revision = *self.artwork_revision() + 1;
        self.as_mut().set_artwork_revision(revision);

        let requests = artwork_requests(&self.rust().games);
        self.as_mut().resolve_artwork(requests);
        self.as_mut().set_library_status(qs(
            "Metadata refresh started: caches cleared, artwork requeued",
        ));
    }

    fn launch_game(mut self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        if self.rust().running.contains(&name) {
            self.as_mut()
                .set_library_status(qs(format!("{} is already running", name)));
            return;
        }

        self.as_mut().rust_mut().running.insert(name.clone());
        self.as_mut().publish_running_games();
        self.as_mut()
            .set_library_status(qs(format!("Launched {}", name)));

        // basalt_core::launch_game blocks until the game exits, so it gets its own thread. Launches
        // don't use the job slot: playing a game never blocks installs or syncs.
        let qt_thread = self.qt_thread();
        thread::spawn(move || {
            let result = basalt_core::launch_game(&name);
            let _ = qt_thread.queue(move |mut backend: Pin<&mut qobject::Backend>| {
                backend.as_mut().rust_mut().running.remove(&name);
                backend.as_mut().publish_running_games();
                if let Err(error) = result {
                    backend
                        .as_mut()
                        .set_library_status(qs(format!("Launch of {} failed: {}", name, error)));
                }
            });
        });
    }

    fn publish_running_games(mut self: Pin<&mut Self>) {
        let mut running: Vec<&String> = self.rust().running.iter().collect();
        running.sort();
        let json = serde_json::to_string(&running).unwrap_or_else(|_| "[]".to_string());
        self.as_mut().set_running_games_json(qs(json));
    }

    fn set_favorite(mut self: Pin<&mut Self>, name: &QString, favorite: bool) {
        let name = name.to_string();
        let result = if favorite {
            basalt_core::add_game_to_playlist(basalt_core::FAVORITES_PLAYLIST_NAME, &name)
        } else {
            basalt_core::remove_game_from_playlist(basalt_core::FAVORITES_PLAYLIST_NAME, &name)
        };

        let message = match result {
            Ok(()) => {
                self.as_mut().reload_playlists();
                if favorite {
                    format!("Added {} to Favorites", name)
                } else {
                    format!("Removed {} from Favorites", name)
                }
            }
            Err(error) if favorite => format!("Favorite failed: {}", error),
            Err(error) => format!("Unfavorite failed: {}", error),
        };
        self.as_mut().set_library_status(qs(message));
    }

    fn remove_game(mut self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        let message = match basalt_core::remove_game(&name) {
            Ok(()) => {
                self.as_mut().reload_library();
                format!("Removed {}", name)
            }
            Err(error) => format!("Remove failed: {}", error),
        };
        self.as_mut().set_library_status(qs(message));
    }

    // ----- Catalog installation and maintenance jobs ---------------------------------------------------------

    fn maintain_game(mut self: Pin<&mut Self>, game: &QString, action: &QString, target: &QString) {
        let game = game.to_string();
        let action = action.to_string();
        let target = StatusTarget::parse(&target.to_string());
        self.as_mut().set_sync_conflict_game(QString::default());
        self.as_mut().run_action_job(target, game, action, None);
    }

    fn restore_backup(mut self: Pin<&mut Self>, game: &QString, snapshot: &QString) {
        let game = game.to_string();
        let snapshot = snapshot.to_string();
        self.as_mut()
            .run_action_job(StatusTarget::Library, game, "restore".to_string(), Some(snapshot));
    }

    fn run_action_job(
        mut self: Pin<&mut Self>,
        target: StatusTarget,
        game: String,
        action: String,
        snapshot: Option<String>,
    ) {
        self.as_mut().start_job(
            target,
            format!("Running {} for {game}...", action.replace('_', " ")),
            move |progress| {
                let request = basalt_core::ActionRequest {
                    snapshot: snapshot.as_deref(),
                    verify: false,
                };
                let result = basalt_core::run_game_action(&game, &action, request, &progress);
                (game, action, result)
            },
            move |mut backend, (game, action, result)| {
                let message = match result {
                    Ok(message) => {
                        backend.as_mut().reload_library();
                        message
                    }
                    Err(error) => {
                        if error.is_sync_conflict() {
                            backend.as_mut().set_sync_conflict_game(qs(&game));
                        }
                        failure_message(&action.replace('_', " "), &error)
                    }
                };
                backend.as_mut().set_status(target, message);
            },
        );
    }

    fn backups_json(&self, game: &QString) -> QString {
        qs(view_model::backups_json(&game.to_string()))
    }

    fn clear_sync_conflict(mut self: Pin<&mut Self>) {
        self.as_mut().set_sync_conflict_game(QString::default());
    }

    fn refresh_game_libraries(mut self: Pin<&mut Self>) {
        match basalt_core::list_libraries().and_then(|libraries| {
            serde_json::to_string(&libraries)
                .map_err(|e| basalt_core::CoreError::new(e.to_string()))
        }) {
            Ok(json) => self.as_mut().set_game_libraries_json(qs(json)),
            Err(error) => self.as_mut().set_install_status(qs(error.to_string())),
        }
    }

    fn add_game_library(mut self: Pin<&mut Self>, id: &QString, path: &QString) {
        match basalt_core::add_library(
            id.to_string().trim(),
            std::path::Path::new(path.to_string().trim()),
        ) {
            Ok(_) => {
                self.as_mut().refresh_game_libraries();
                self.as_mut().set_install_status(qs("Library added"));
            }
            Err(error) => self.as_mut().set_install_status(qs(error.to_string())),
        }
    }

    fn install_game(mut self: Pin<&mut Self>, game: &QString, library: &QString) {
        let game = game.to_string();
        if self.rust().running.contains(&game) {
            self.as_mut()
                .set_install_status(qs("Close the game before reinstalling it"));
            return;
        }
        let library = library.to_string();
        self.as_mut().start_job(
            StatusTarget::Install,
            format!("Installing {game}"),
            move |progress| basalt_core::install_game(&game, &library, &progress),
            |mut backend, result| {
                let message = match result {
                    Ok(report) => {
                        backend.as_mut().reload_library();
                        format!(
                            "Installed {} release '{}' into {}",
                            report.game_name,
                            report.release_tag,
                            report.install_dir.display()
                        )
                    }
                    Err(error) => failure_message("Install", &error),
                };
                backend.as_mut().set_install_status(qs(message));
            },
        );
    }

    fn cancel_job(mut self: Pin<&mut Self>) {
        if let Some(cancel) = &self.rust().job_cancel {
            cancel.cancel();
        }
        if *self.job_active() {
            self.as_mut().set_job_cancelling(true);
        }
    }

    /// Runs `work` on a worker thread in the single job slot, feeding its [`Progress`] into the
    /// job_* properties, then calls `finish` back on the Qt thread.
    fn start_job<R, W, F>(
        mut self: Pin<&mut Self>,
        target: StatusTarget,
        pending: String,
        work: W,
        finish: F,
    ) where
        R: Send + 'static,
        W: FnOnce(Progress) -> R + Send + 'static,
        F: FnOnce(Pin<&mut qobject::Backend>, R) + Send + 'static,
    {
        if *self.job_active() {
            self.as_mut()
                .set_status(target, "Another operation is already running".to_string());
            return;
        }

        let cancel = CancelToken::new();
        self.as_mut().rust_mut().job_cancel = Some(cancel.clone());
        self.as_mut().set_job_active(true);
        self.as_mut().set_job_message(QString::default());
        self.as_mut().set_job_fraction(-1.0);
        self.as_mut().set_job_cancellable(false);
        self.as_mut().set_job_cancelling(false);
        self.as_mut().set_status(target, pending);

        let progress_thread = self.qt_thread();
        let progress = Progress::new(
            move |update| {
                let message = qs(&update.message);
                let fraction = update.fraction().map(f64::from).unwrap_or(-1.0);
                let _ = progress_thread.queue(move |mut backend: Pin<&mut qobject::Backend>| {
                    if !*backend.job_active() {
                        return;
                    }
                    backend.as_mut().set_job_message(message);
                    backend.as_mut().set_job_fraction(fraction);
                    // Only jobs that report progress also honor cancellation.
                    backend.as_mut().set_job_cancellable(true);
                });
            },
            cancel,
        );

        let qt_thread = self.qt_thread();
        thread::spawn(move || {
            let result = work(progress);
            let _ = qt_thread.queue(move |mut backend: Pin<&mut qobject::Backend>| {
                backend.as_mut().rust_mut().job_cancel = None;
                backend.as_mut().set_job_active(false);
                backend.as_mut().set_job_cancellable(false);
                backend.as_mut().set_job_cancelling(false);
                backend.as_mut().set_job_message(QString::default());
                finish(backend.as_mut(), result);
                backend.as_mut().show_core_warnings(target);
            });
        });
    }

    /// Appends warnings core queued during an operation (see basalt_core::take_warnings) to
    /// that screen's status line.
    fn show_core_warnings(mut self: Pin<&mut Self>, target: StatusTarget) {
        let warnings = basalt_core::take_warnings();
        if warnings.is_empty() {
            return;
        }

        let current = match target {
            StatusTarget::Library => self.library_status().to_string(),
            StatusTarget::Install => self.install_status().to_string(),
        };
        let joined = warnings.join("; ");
        let message = if current.is_empty() {
            format!("Warning: {}", joined)
        } else {
            format!("{} | Warning: {}", current, joined)
        };
        self.as_mut().set_status(target, message);
    }

    fn set_status(mut self: Pin<&mut Self>, target: StatusTarget, message: String) {
        match target {
            StatusTarget::Library => self.as_mut().set_library_status(qs(message)),
            StatusTarget::Install => self.as_mut().set_install_status(qs(message)),
        }
    }

    // ----- settings --------------------------------------------------------------------------

    fn save_backup_keep(mut self: Pin<&mut Self>, keep: i32) {
        let result = u32::try_from(keep)
            .map_err(|_| CoreError::InvalidInput("Backups per game must be at least 1".into()))
            .and_then(basalt_core::save_backup_keep);
        let message = match result {
            Ok(()) => {
                self.as_mut().set_backup_keep(keep);
                format!("Backup limit saved: {keep} per game. Applied when the next backup is created.")
            }
            Err(error) => format!("Save failed: {error}"),
        };
        self.as_mut().set_settings_status(qs(message));
    }

    fn set_display_mode(mut self: Pin<&mut Self>, fullscreen: bool, maximized: bool) {
        let maximized = maximized && !fullscreen;
        let message = match basalt_core::save_launcher_display_settings(fullscreen, maximized) {
            Ok(saved) => {
                self.as_mut().set_fullscreen(saved.fullscreen_enabled);
                self.as_mut().set_maximized(saved.maximized_enabled);
                if saved.fullscreen_enabled {
                    "Enabled launcher fullscreen"
                } else if saved.maximized_enabled {
                    "Enabled launcher maximized window mode"
                } else {
                    "Disabled launcher fullscreen/maximized window mode"
                }
                .to_string()
            }
            // The QML checkboxes re-bind to fullscreen/maximized, so they show the stored values.
            Err(error) => format!("Save failed: {}", error),
        };
        self.as_mut().set_settings_status(qs(message));
    }

    fn save_sync_remote(mut self: Pin<&mut Self>, path: &QString) {
        let path = path.to_string();
        let path = path.trim();
        let remote = (!path.is_empty()).then(|| std::path::PathBuf::from(path));
        let message = match basalt_core::save_sync_remote(remote.as_deref()) {
            Ok(()) => {
                self.as_mut().set_sync_remote(qs(path));
                let (folder, what) = match &remote {
                    None => (
                        std::path::PathBuf::from(basalt_core::DEFAULT_SYNC_REMOTE),
                        format!("Using the default sync folder, {}", basalt_core::DEFAULT_SYNC_REMOTE),
                    ),
                    Some(folder) => (folder.clone(), format!("Sync folder set to {path}")),
                };
                if folder.is_dir() {
                    what
                } else {
                    format!("{what}, but it is not available right now (is it mounted?)")
                }
            }
            Err(error) => format!("Save failed: {}", error),
        };
        self.as_mut().set_settings_status(qs(message));
    }

    // ----- queries ---------------------------------------------------------------------------

    fn artwork_url(&self, key: &QString) -> QString {
        self.rust()
            .artwork_urls
            .get(&key.to_string())
            .map(qs)
            .unwrap_or_default()
    }


}

/// Steam art for Steam games, plus one request per catalog game (installed or not), shared by
/// Library and Install tiles.
fn artwork_requests(games: &[GameEntry]) -> Vec<ArtworkRequest> {
    games
        .iter()
        .filter(|game| game.runner_kind == basalt_core::RunnerKind::Steam)
        .filter_map(ArtworkRequest::for_game)
        .chain(ArtworkRequest::for_catalog_games(games))
        .collect()
}

/// "<what> failed: <error>", or "<what> cancelled" when the user cancelled it.
fn failure_message(what: &str, error: &CoreError) -> String {
    if matches!(error, CoreError::Cancelled) {
        format!("{} cancelled", what)
    } else {
        format!("{} failed: {}", what, error)
    }
}

fn discover_summary(report: &DiscoverReport) -> String {
    let catalog = match report.catalog {
        Some(DiscoverResult::Added) => "Catalog games discovered".to_string(),
        Some(DiscoverResult::AlreadyExists) => "Catalog already registered".to_string(),
        Some(DiscoverResult::NotFound) => "No new catalog games found".to_string(),
        None => "Catalog skipped".to_string(),
    };
    let steam = match &report.steam {
        Some(steam) => format!(
            "Steam: found {}, added {}, existing {}",
            steam.found, steam.added, steam.already_exists
        ),
        None => "Steam skipped".to_string(),
    };
    format!("Discover complete | {} | {}", catalog, steam)
}
