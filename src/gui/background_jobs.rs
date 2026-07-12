use std::sync::mpsc::{self, TryRecvError};
use std::thread;

use crate::core;

use super::app::BasaltApp;

pub(super) enum GuiBackgroundJobResult {
    InstallMattmc(Result<(), String>),
    InstallEmulatorCore {
        system: String,
        result: Result<(), String>,
    },
    SyncEmulatorRomsUp {
        system: String,
        result: Result<core::EmulationRomSyncReport, String>,
    },
    SyncEmulatorRomsDown {
        system: String,
        result: Result<(core::EmulationRomSyncReport, core::EmulatorDiscoverReport), String>,
    },
    SyncEmulatorSavesUp {
        system: String,
        result: Result<core::EmulationRomSyncReport, String>,
    },
    SyncEmulatorSavesDown {
        system: String,
        result: Result<core::EmulationRomSyncReport, String>,
    },
    SyncMattmcUp(Result<(), String>),
    SyncMattmcDown(Result<(), String>),
}

#[derive(Clone, Copy)]
pub(super) enum GuiBackgroundStatusTarget {
    Library,
    Install,
}

impl BasaltApp {
    pub(super) fn has_background_job(&self) -> bool {
        self.background_job_rx.is_some()
    }

    pub(super) fn start_background_job<F>(
        &mut self,
        status_target: GuiBackgroundStatusTarget,
        pending_message: String,
        build_result: F,
    ) where
        F: FnOnce() -> GuiBackgroundJobResult + Send + 'static,
    {
        if self.background_job_rx.is_some() {
            self.set_background_status(status_target, "Another operation is already running");
            return;
        }

        let (tx, rx) = mpsc::channel::<GuiBackgroundJobResult>();
        self.background_job_rx = Some(rx);
        self.set_background_status(status_target, &pending_message);

        thread::spawn(move || {
            let _ = tx.send(build_result());
        });
    }

    pub(super) fn poll_background_job(&mut self) {
        let poll_result = self
            .background_job_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());

        let Some(received) = poll_result else {
            return;
        };

        match received {
            Ok(result) => {
                self.background_job_rx = None;
                self.apply_background_job_result(result);
            }
            Err(TryRecvError::Disconnected) => {
                self.background_job_rx = None;
                self.status_message = "Operation failed: background task disconnected".to_string();
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn apply_background_job_result(&mut self, result: GuiBackgroundJobResult) {
        match result {
            GuiBackgroundJobResult::InstallMattmc(result) => match result {
                Ok(_) => {
                    self.install_status_message = "MattMC install completed".to_string();
                    self.refresh_games();
                }
                Err(err) => {
                    self.install_status_message = format!("Install failed: {}", err);
                }
            },
            GuiBackgroundJobResult::InstallEmulatorCore { system, result } => match result {
                Ok(_) => {
                    self.install_status_message =
                        format!("Installed {} emulator core", system.to_uppercase());
                }
                Err(err) => {
                    self.install_status_message =
                        format!("{} core install failed: {}", system.to_uppercase(), err);
                }
            },
            GuiBackgroundJobResult::SyncEmulatorRomsUp { system, result } => match result {
                Ok(report) => {
                    self.install_status_message = format!(
                        "Sync Roms Up ({}) completed: copied {}, unchanged {}, deleted {}",
                        system.to_uppercase(),
                        report.copied,
                        report.unchanged,
                        report.deleted
                    );
                }
                Err(err) => {
                    self.install_status_message =
                        format!("Sync Roms Up ({}) failed: {}", system.to_uppercase(), err);
                }
            },
            GuiBackgroundJobResult::SyncEmulatorRomsDown { system, result } => match result {
                Ok((sync_report, emulator_report)) => {
                    self.refresh_games();
                    self.install_status_message = format!(
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
                    self.install_status_message =
                        format!("Sync Roms Down ({}) failed: {}", system.to_uppercase(), err);
                }
            },
            GuiBackgroundJobResult::SyncEmulatorSavesUp { system, result } => match result {
                Ok(report) => {
                    self.install_status_message = format!(
                        "Sync Saves Up ({}) completed: copied {}, unchanged {}, deleted {}",
                        system.to_uppercase(),
                        report.copied,
                        report.unchanged,
                        report.deleted
                    );
                }
                Err(err) => {
                    self.install_status_message =
                        format!("Sync Saves Up ({}) failed: {}", system.to_uppercase(), err);
                }
            },
            GuiBackgroundJobResult::SyncEmulatorSavesDown { system, result } => match result {
                Ok(report) => {
                    self.install_status_message = format!(
                        "Sync Saves Down ({}) completed: copied {}, unchanged {}, deleted {}",
                        system.to_uppercase(),
                        report.copied,
                        report.unchanged,
                        report.deleted
                    );
                }
                Err(err) => {
                    self.install_status_message = format!(
                        "Sync Saves Down ({}) failed: {}",
                        system.to_uppercase(),
                        err
                    );
                }
            },
            GuiBackgroundJobResult::SyncMattmcUp(result) => match result {
                Ok(_) => {
                    self.status_message = "SyncUp completed for MattMC".to_string();
                }
                Err(err) => {
                    self.status_message = format!("SyncUp failed: {}", err);
                }
            },
            GuiBackgroundJobResult::SyncMattmcDown(result) => match result {
                Ok(_) => {
                    self.status_message = "SyncDown completed for MattMC".to_string();
                }
                Err(err) => {
                    self.status_message = format!("SyncDown failed: {}", err);
                }
            },
        }
    }

    fn set_background_status(&mut self, target: GuiBackgroundStatusTarget, message: &str) {
        match target {
            GuiBackgroundStatusTarget::Library => {
                self.status_message = message.to_string();
            }
            GuiBackgroundStatusTarget::Install => {
                self.install_status_message = message.to_string();
            }
        }
    }
}
