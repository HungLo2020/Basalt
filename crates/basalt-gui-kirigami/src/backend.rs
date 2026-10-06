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
        #[qproperty(QString, running_games_json)]
        #[qproperty(bool, loading)]
        // Bumped when artwork URLs or installed-core state change, so QML bindings re-evaluate.
        #[qproperty(i32, artwork_revision)]
        #[qproperty(i32, core_status_revision)]
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
        #[qproperty(QString, remote_roms_root)]
        #[qproperty(QString, remote_saves_root)]
        #[qproperty(bool, fullscreen)]
        #[qproperty(bool, maximized)]
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
        fn sync_mattmc(self: Pin<&mut Backend>, up: bool);
        /// `target` is "library" or "install": which screen shows the result.
        #[qinvokable]
        fn update_mattmc(self: Pin<&mut Backend>, target: &QString);
        #[qinvokable]
        fn install_mattmc(self: Pin<&mut Backend>);
        #[qinvokable]
        fn install_core(self: Pin<&mut Backend>, system: &QString);
        #[qinvokable]
        fn sync_roms(self: Pin<&mut Backend>, system: &QString, up: bool);
        #[qinvokable]
        fn sync_saves(self: Pin<&mut Backend>, system: &QString, up: bool);
        #[qinvokable]
        fn cancel_job(self: Pin<&mut Backend>);

        #[qinvokable]
        fn save_remote_paths(self: Pin<&mut Backend>, roms_root: &QString, saves_root: &QString);
        #[qinvokable]
        fn set_display_mode(self: Pin<&mut Backend>, fullscreen: bool, maximized: bool);

        /// Image URL for an artwork key, or "" while unresolved.
        #[qinvokable]
        fn artwork_url(self: &Backend, key: &QString) -> QString;
        #[qinvokable]
        fn is_core_installed(self: &Backend, system: &QString) -> bool;

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
    running_games_json: QString,
    loading: bool,
    artwork_revision: i32,
    core_status_revision: i32,
    library_status: QString,
    install_status: QString,
    settings_status: QString,
    job_active: bool,
    job_message: QString,
    job_fraction: f64,
    job_cancellable: bool,
    job_cancelling: bool,
    remote_roms_root: QString,
    remote_saves_root: QString,
    fullscreen: bool,
    maximized: bool,
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
            running_games_json: QString::from("[]"),
            loading: true,
            artwork_revision: 0,
            core_status_revision: 0,
            library_status: QString::from("Loading games..."),
            install_status: QString::default(),
            settings_status: QString::default(),
            job_active: false,
            job_message: QString::default(),
            job_fraction: -1.0,
            job_cancellable: false,
            job_cancelling: false,
            remote_roms_root: QString::default(),
            remote_saves_root: QString::default(),
            fullscreen: false,
            maximized: false,
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

        let paths = basalt_core::load_emulation_remote_paths().unwrap_or_else(|error| {
            warnings.push(format!("Settings load warning: {}", error));
            basalt_core::default_emulation_remote_paths()
        });
        self.as_mut().set_remote_roms_root(qs(&paths.roms_root_dir));
        self.as_mut()
            .set_remote_saves_root(qs(&paths.saves_root_dir));

        let display = basalt_core::load_launcher_display_settings().unwrap_or_else(|error| {
            warnings.push(format!("Display setting load warning: {}", error));
            basalt_core::LauncherDisplaySettings {
                fullscreen_enabled: false,
                maximized_enabled: false,
            }
        });
        self.as_mut().set_fullscreen(display.fullscreen_enabled);
        self.as_mut().set_maximized(display.maximized_enabled);
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

        let requests: Vec<ArtworkRequest> = games
            .iter()
            .filter_map(ArtworkRequest::for_game)
            .chain(std::iter::once(ArtworkRequest::mattmc()))
            .collect();

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

        let requests: Vec<ArtworkRequest> = self
            .rust()
            .games
            .iter()
            .filter_map(ArtworkRequest::for_game)
            .chain(std::iter::once(ArtworkRequest::mattmc()))
            .collect();
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

    // ----- MattMC and emulation jobs ---------------------------------------------------------

    fn sync_mattmc(mut self: Pin<&mut Self>, up: bool) {
        let label = if up { "SyncUp" } else { "SyncDown" };
        self.as_mut().start_job(
            StatusTarget::Library,
            format!("{} started for MattMC", label),
            move |_| {
                if up {
                    basalt_core::sync_mattmc_up()
                } else {
                    basalt_core::sync_mattmc_down()
                }
            },
            move |mut backend, result| {
                let message = match result {
                    Ok(()) => format!("{} completed for MattMC", label),
                    Err(error) => failure_message(label, &error),
                };
                backend.as_mut().set_library_status(qs(message));
            },
        );
    }

    fn update_mattmc(mut self: Pin<&mut Self>, target: &QString) {
        let target = StatusTarget::parse(&target.to_string());
        self.as_mut().start_job(
            target,
            "MattMC update started".to_string(),
            |_| basalt_core::update_mattmc(),
            move |mut backend, result| {
                let message = match result {
                    Ok(()) => {
                        backend.as_mut().reload_library();
                        "Update completed for MattMC".to_string()
                    }
                    Err(error) => format!("MattMC update failed: {}", error),
                };
                backend.as_mut().set_status(target, message);
            },
        );
    }

    fn install_mattmc(mut self: Pin<&mut Self>) {
        self.as_mut().start_job(
            StatusTarget::Install,
            "MattMC install started".to_string(),
            |progress| basalt_core::install_mattmc(&progress),
            |mut backend, result| {
                let message = match result {
                    Ok(report) => {
                        backend.as_mut().reload_library();
                        format!(
                            "Installed MattMC release '{}' into {}. {}",
                            report.release_tag,
                            report.install_dir.display(),
                            report.discovery_message()
                        )
                    }
                    Err(error) => failure_message("Install", &error),
                };
                backend.as_mut().set_install_status(qs(message));
            },
        );
    }

    fn install_core(mut self: Pin<&mut Self>, system: &QString) {
        let system = system.to_string();
        let label = system.to_uppercase();
        self.as_mut().start_job(
            StatusTarget::Install,
            format!("Installing {} emulator core...", label),
            move |progress| basalt_core::install_emulation_core_for_system(&system, &progress),
            move |mut backend, result| {
                let message = match result {
                    Ok(()) => format!("Installed {} emulator core", label),
                    Err(error) => failure_message(&format!("{} core install", label), &error),
                };
                backend.as_mut().set_install_status(qs(message));
            },
        );
    }

    fn sync_roms(mut self: Pin<&mut Self>, system: &QString, up: bool) {
        let system = system.to_string();
        let label = format!(
            "Sync Roms {} ({})",
            if up { "Up" } else { "Down" },
            system.to_uppercase()
        );

        if up {
            self.as_mut().start_job(
                StatusTarget::Install,
                format!("{} started", label),
                move |progress| basalt_core::sync_emulation_roms_up_for_system(&system, &progress),
                move |mut backend, result| {
                    let message = match result {
                        Ok(report) => format!(
                            "{} completed: copied {}, unchanged {}, deleted {}",
                            label, report.copied, report.unchanged, report.deleted
                        ),
                        Err(error) => failure_message(&label, &error),
                    };
                    backend.as_mut().set_install_status(qs(message));
                },
            );
        } else {
            self.as_mut().start_job(
                StatusTarget::Install,
                format!("{} started", label),
                move |progress| {
                    basalt_core::sync_emulation_roms_down_and_discover_for_system(&system, &progress)
                },
                move |mut backend, result| {
                    let message = match result {
                        Ok((sync, discovered)) => {
                            backend.as_mut().reload_library();
                            format!(
                                "{} completed: copied {}, unchanged {}, deleted {} | Emulator discover: found {}, added {}, updated {}, existing {}",
                                label,
                                sync.copied,
                                sync.unchanged,
                                sync.deleted,
                                discovered.found,
                                discovered.added,
                                discovered.updated,
                                discovered.already_exists
                            )
                        }
                        Err(error) => failure_message(&label, &error),
                    };
                    backend.as_mut().set_install_status(qs(message));
                },
            );
        }
    }

    fn sync_saves(mut self: Pin<&mut Self>, system: &QString, up: bool) {
        let system = system.to_string();
        let label = format!(
            "Sync Saves {} ({})",
            if up { "Up" } else { "Down" },
            system.to_uppercase()
        );
        self.as_mut().start_job(
            StatusTarget::Install,
            format!("{} started", label),
            move |progress| {
                if up {
                    basalt_core::sync_emulation_saves_up_for_system(&system, &progress)
                } else {
                    basalt_core::sync_emulation_saves_down_for_system(&system, &progress)
                }
            },
            move |mut backend, result| {
                let message = match result {
                    Ok(report) => format!(
                        "{} completed: copied {}, unchanged {}, deleted {}",
                        label, report.copied, report.unchanged, report.deleted
                    ),
                    Err(error) => failure_message(&label, &error),
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
                let revision = *backend.core_status_revision() + 1;
                backend.as_mut().set_core_status_revision(revision);
            });
        });
    }

    fn set_status(mut self: Pin<&mut Self>, target: StatusTarget, message: String) {
        match target {
            StatusTarget::Library => self.as_mut().set_library_status(qs(message)),
            StatusTarget::Install => self.as_mut().set_install_status(qs(message)),
        }
    }

    // ----- settings --------------------------------------------------------------------------

    fn save_remote_paths(mut self: Pin<&mut Self>, roms_root: &QString, saves_root: &QString) {
        let message = match basalt_core::save_emulation_remote_paths(
            &roms_root.to_string(),
            &saves_root.to_string(),
        ) {
            Ok(saved) => {
                self.as_mut().set_remote_roms_root(qs(&saved.roms_root_dir));
                self.as_mut()
                    .set_remote_saves_root(qs(&saved.saves_root_dir));
                "Saved remote ROM/Saves default paths".to_string()
            }
            Err(error) => format!("Save failed: {}", error),
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

    // ----- queries ---------------------------------------------------------------------------

    fn artwork_url(&self, key: &QString) -> QString {
        self.rust()
            .artwork_urls
            .get(&key.to_string())
            .map(qs)
            .unwrap_or_default()
    }

    fn is_core_installed(&self, system: &QString) -> bool {
        basalt_core::is_emulation_core_installed_for_system(&system.to_string()).unwrap_or(false)
    }
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
    let mattmc = match report.mattmc {
        Some(DiscoverResult::Added) => "MattMC added".to_string(),
        Some(DiscoverResult::AlreadyExists) => "MattMC already exists".to_string(),
        Some(DiscoverResult::NotFound) => "MattMC not found".to_string(),
        None => "MattMC skipped".to_string(),
    };
    let steam = match &report.steam {
        Some(steam) => format!(
            "Steam: found {}, added {}, existing {}",
            steam.found, steam.added, steam.already_exists
        ),
        None => "Steam skipped".to_string(),
    };
    let emulators = match &report.emulators {
        Some(emulators) => format!(
            "Emulators: found {}, added {}, updated {}, existing {}",
            emulators.found, emulators.added, emulators.updated, emulators.already_exists
        ),
        None => "Emulators skipped".to_string(),
    };

    format!("Discover complete | {} | {} | {}", mattmc, steam, emulators)
}
