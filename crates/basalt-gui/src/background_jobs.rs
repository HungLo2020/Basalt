use std::sync::mpsc::{self, TryRecvError};
use std::thread;

use std::sync::Arc;

use basalt_core::{self as core, CancelToken, CoreResult, DiscoverResult, Progress};
use eframe::egui;

use super::app::BasaltApp;

pub(super) enum GuiBackgroundJobResult {
    Discover(CoreResult<core::DiscoverReport>),
    InstallMattmc(CoreResult<core::MattmcInstallReport>),
    InstallEmulatorCore {
        system: String,
        result: CoreResult<()>,
    },
    SyncEmulatorRomsUp {
        system: String,
        result: CoreResult<core::EmulationRomSyncReport>,
    },
    SyncEmulatorRomsDown {
        system: String,
        result: CoreResult<(core::EmulationRomSyncReport, core::EmulatorDiscoverReport)>,
    },
    SyncEmulatorSavesUp {
        system: String,
        result: CoreResult<core::EmulationRomSyncReport>,
    },
    SyncEmulatorSavesDown {
        system: String,
        result: CoreResult<core::EmulationRomSyncReport>,
    },
    SyncMattmcUp(CoreResult<()>),
    SyncMattmcDown(CoreResult<()>),
    UpdateMattmc {
        status_target: GuiBackgroundStatusTarget,
        result: CoreResult<()>,
    },
}

#[derive(Clone, Copy)]
pub(super) enum GuiBackgroundStatusTarget {
    Library,
    Install,
}

impl BasaltApp {
    pub(super) fn has_background_job(&self) -> bool {
        self.background_job.rx.is_some()
    }

    /// Runs `build_result` on a worker thread. It receives a [`Progress`] that feeds the progress
    /// bar and Cancel button; jobs that can't report progress simply ignore it.
    pub(super) fn start_background_job<F>(
        &mut self,
        status_target: GuiBackgroundStatusTarget,
        pending_message: String,
        build_result: F,
    ) where
        F: FnOnce(Progress) -> GuiBackgroundJobResult + Send + 'static,
    {
        if self.background_job.rx.is_some() {
            self.set_background_status(status_target, "Another operation is already running");
            return;
        }

        let (tx, rx) = mpsc::channel::<GuiBackgroundJobResult>();
        self.background_job.rx = Some(rx);
        self.set_background_status(status_target, &pending_message);

        let cancel = CancelToken::new();
        self.background_job.cancel = Some(cancel.clone());
        if let Ok(mut slot) = self.background_job.progress.lock() {
            *slot = None;
        }

        let progress_slot = Arc::clone(&self.background_job.progress);
        let egui_ctx = self.egui_ctx.clone();
        let progress = Progress::new(
            move |update| {
                if let Ok(mut slot) = progress_slot.lock() {
                    *slot = Some(update.clone());
                }
                egui_ctx.request_repaint();
            },
            cancel,
        );

        let egui_ctx = self.egui_ctx.clone();
        thread::spawn(move || {
            let _ = tx.send(build_result(progress));
            egui_ctx.request_repaint();
        });
    }

    /// Progress bar and Cancel button for the running job, shown under a screen's status text.
    pub(super) fn render_background_job_progress(&self, ui: &mut egui::Ui) {
        if !self.has_background_job() {
            return;
        }
        let Some(update) = self
            .background_job
            .progress
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
        else {
            return;
        };

        ui.add_space(6.0);
        ui.label(&update.message);
        let bar = match update.fraction() {
            Some(fraction) => egui::ProgressBar::new(fraction).show_percentage(),
            None => egui::ProgressBar::new(0.0).animate(true),
        };
        ui.add(bar);

        if let Some(cancel) = &self.background_job.cancel {
            if cancel.is_cancelled() {
                ui.label("Cancelling...");
            } else if ui.button("Cancel").clicked() {
                cancel.cancel();
            }
        }
    }

    pub(super) fn poll_background_job(&mut self) {
        let poll_result = self
            .background_job
            .rx
            .as_ref()
            .map(|receiver| receiver.try_recv());

        let Some(received) = poll_result else {
            return;
        };

        match received {
            Ok(result) => {
                self.finish_background_job();
                self.apply_background_job_result(result);
            }
            Err(TryRecvError::Disconnected) => {
                self.finish_background_job();
                self.library.status_message =
                    "Operation failed: background task disconnected".to_string();
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn finish_background_job(&mut self) {
        self.background_job.rx = None;
        self.background_job.cancel = None;
        if let Ok(mut slot) = self.background_job.progress.lock() {
            *slot = None;
        }
    }

    fn apply_background_job_result(&mut self, result: GuiBackgroundJobResult) {
        match result {
            GuiBackgroundJobResult::Discover(result) => self.apply_discover_result(result),
            GuiBackgroundJobResult::InstallMattmc(result) => match result {
                Ok(report) => {
                    self.install.status_message = format!(
                        "Installed MattMC release '{}' into {}. {}",
                        report.release_tag,
                        report.install_dir.display(),
                        report.discovery_message()
                    );
                    self.refresh_games();
                }
                Err(err) => {
                    self.install.status_message = format!("Install failed: {}", err);
                }
            },
            GuiBackgroundJobResult::InstallEmulatorCore { system, result } => match result {
                Ok(_) => {
                    self.install.status_message =
                        format!("Installed {} emulator core", system.to_uppercase());
                }
                Err(err) => {
                    self.install.status_message =
                        format!("{} core install failed: {}", system.to_uppercase(), err);
                }
            },
            GuiBackgroundJobResult::SyncEmulatorRomsUp { system, result } => match result {
                Ok(report) => {
                    self.install.status_message = format!(
                        "Sync Roms Up ({}) completed: copied {}, unchanged {}, deleted {}",
                        system.to_uppercase(),
                        report.copied,
                        report.unchanged,
                        report.deleted
                    );
                }
                Err(err) => {
                    self.install.status_message =
                        format!("Sync Roms Up ({}) failed: {}", system.to_uppercase(), err);
                }
            },
            GuiBackgroundJobResult::SyncEmulatorRomsDown { system, result } => match result {
                Ok((sync_report, emulator_report)) => {
                    self.refresh_games();
                    self.install.status_message = format!(
                        "Sync Roms Down ({}) completed: copied {}, unchanged {}, deleted {} | Emulator discover: found {}, added {}, updated {}, existing {}",
                        system.to_uppercase(),
                        sync_report.copied,
                        sync_report.unchanged,
                        sync_report.deleted,
                        emulator_report.found,
                        emulator_report.added,
                        emulator_report.updated,
                        emulator_report.already_exists
                    );
                }
                Err(err) => {
                    self.install.status_message =
                        format!("Sync Roms Down ({}) failed: {}", system.to_uppercase(), err);
                }
            },
            GuiBackgroundJobResult::SyncEmulatorSavesUp { system, result } => match result {
                Ok(report) => {
                    self.install.status_message = format!(
                        "Sync Saves Up ({}) completed: copied {}, unchanged {}, deleted {}",
                        system.to_uppercase(),
                        report.copied,
                        report.unchanged,
                        report.deleted
                    );
                }
                Err(err) => {
                    self.install.status_message =
                        format!("Sync Saves Up ({}) failed: {}", system.to_uppercase(), err);
                }
            },
            GuiBackgroundJobResult::SyncEmulatorSavesDown { system, result } => match result {
                Ok(report) => {
                    self.install.status_message = format!(
                        "Sync Saves Down ({}) completed: copied {}, unchanged {}, deleted {}",
                        system.to_uppercase(),
                        report.copied,
                        report.unchanged,
                        report.deleted
                    );
                }
                Err(err) => {
                    self.install.status_message = format!(
                        "Sync Saves Down ({}) failed: {}",
                        system.to_uppercase(),
                        err
                    );
                }
            },
            GuiBackgroundJobResult::SyncMattmcUp(result) => match result {
                Ok(_) => {
                    self.library.status_message = "SyncUp completed for MattMC".to_string();
                }
                Err(err) => {
                    self.library.status_message = format!("SyncUp failed: {}", err);
                }
            },
            GuiBackgroundJobResult::SyncMattmcDown(result) => match result {
                Ok(_) => {
                    self.library.status_message = "SyncDown completed for MattMC".to_string();
                }
                Err(err) => {
                    self.library.status_message = format!("SyncDown failed: {}", err);
                }
            },
            GuiBackgroundJobResult::UpdateMattmc {
                status_target,
                result,
            } => match result {
                Ok(_) => {
                    self.set_background_status(status_target, "Update completed for MattMC");
                    self.refresh_games();
                }
                Err(err) => {
                    self.set_background_status(
                        status_target,
                        &format!("MattMC update failed: {}", err),
                    );
                }
            },
        }
    }

    fn apply_discover_result(&mut self, result: CoreResult<core::DiscoverReport>) {
        match result {
            Ok(report) => {
                self.refresh_games();

                let mattmc_message = match report.mattmc {
                    Some(DiscoverResult::Added) => "MattMC added".to_string(),
                    Some(DiscoverResult::AlreadyExists) => "MattMC already exists".to_string(),
                    Some(DiscoverResult::NotFound) => "MattMC not found".to_string(),
                    None => "MattMC skipped".to_string(),
                };

                let steam_message = match report.steam {
                    Some(steam) => format!(
                        "Steam: found {}, added {}, existing {}",
                        steam.found, steam.added, steam.already_exists
                    ),
                    None => "Steam skipped".to_string(),
                };

                let emulator_message = match report.emulators {
                    Some(emulators) => format!(
                        "Emulators: found {}, added {}, updated {}, existing {}",
                        emulators.found,
                        emulators.added,
                        emulators.updated,
                        emulators.already_exists
                    ),
                    None => "Emulators skipped".to_string(),
                };

                self.library.status_message = format!(
                    "Discover complete | {} | {} | {}",
                    mattmc_message, steam_message, emulator_message
                );
            }
            Err(err) => {
                self.library.status_message = format!("Discover failed: {}", err);
            }
        }
    }

    fn set_background_status(&mut self, target: GuiBackgroundStatusTarget, message: &str) {
        match target {
            GuiBackgroundStatusTarget::Library => {
                self.library.status_message = message.to_string();
            }
            GuiBackgroundStatusTarget::Install => {
                self.install.status_message = message.to_string();
            }
        }
    }
}
