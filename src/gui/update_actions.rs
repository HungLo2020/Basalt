use std::sync::mpsc::{self, TryRecvError};
use std::thread;

use crate::core;

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn poll_update_tasks(&mut self) {
        self.poll_update_check();
        self.poll_update_install();
    }

    pub(super) fn update_button_text(&self) -> &'static str {
        if self.update.install_rx.is_some() {
            "Updating..."
        } else if self.update.check_rx.is_some() {
            "Checking..."
        } else if self
            .update
            .latest_update
            .as_ref()
            .map(|update| update.update_available)
            .unwrap_or(false)
        {
            if core::can_install_basalt_updates() {
                "Update Basalt"
            } else {
                "Update unsupported"
            }
        } else {
            "Check for updates"
        }
    }

    pub(super) fn can_use_update_button(&self) -> bool {
        self.update.check_rx.is_none()
            && self.update.install_rx.is_none()
            && (!self
                .update
                .latest_update
                .as_ref()
                .map(|update| update.update_available)
                .unwrap_or(false)
                || core::can_install_basalt_updates())
    }

    pub(super) fn handle_update_button_click(&mut self) {
        if !self.can_use_update_button() {
            return;
        }

        if let Some(update) = self
            .update
            .latest_update
            .clone()
            .filter(|update| update.update_available)
        {
            self.start_update_install(update);
        } else {
            self.start_update_check();
        }
    }

    pub(super) fn start_update_check(&mut self) {
        if self.update.check_rx.is_some() || self.update.install_rx.is_some() {
            return;
        }

        let (tx, rx) = mpsc::channel::<Result<core::UpdateCheckResult, String>>();
        self.update.check_rx = Some(rx);
        self.update.status_message = "Checking for Basalt updates...".to_string();

        thread::spawn(move || {
            let result = core::check_for_basalt_updates();
            let _ = tx.send(result);
        });
    }

    fn start_update_install(&mut self, update: core::UpdateCheckResult) {
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        self.update.install_rx = Some(rx);
        self.update.status_message = format!("Downloading {}...", update.asset_name);

        thread::spawn(move || {
            let result = core::download_basalt_update(&update)
                .and_then(|downloaded| core::install_basalt_update_and_restart(&downloaded));
            let _ = tx.send(result);
        });
    }

    fn poll_update_check(&mut self) {
        let poll_result = self
            .update
            .check_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());

        let Some(received) = poll_result else {
            return;
        };

        match received {
            Ok(Ok(update)) => {
                self.update.check_rx = None;
                if update.update_available {
                    if core::can_install_basalt_updates() {
                        self.update.status_message = format!(
                            "Basalt update available: {} ({}) - {}",
                            update.release_name, update.latest.version, update.release_page_url
                        );
                    } else {
                        self.update.status_message = format!(
                            "Basalt update available: {} ({}), but automatic updates are not supported on this platform.",
                            update.release_name, update.latest.version
                        );
                    }
                } else {
                    self.update.status_message = format!(
                        "Basalt is up to date: {} ({})",
                        update.current.version, update.current.commit
                    );
                }
                self.update.latest_update = Some(update);
            }
            Ok(Err(error)) => {
                self.update.check_rx = None;
                self.update.latest_update = None;
                self.update.status_message = error;
            }
            Err(TryRecvError::Disconnected) => {
                self.update.check_rx = None;
                self.update.status_message =
                    "Update check failed: background task disconnected".to_string();
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn poll_update_install(&mut self) {
        let poll_result = self
            .update
            .install_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());

        let Some(received) = poll_result else {
            return;
        };

        match received {
            Ok(Ok(())) => {
                self.update.install_rx = None;
                self.update.status_message = "Basalt update completed".to_string();
            }
            Ok(Err(error)) => {
                self.update.install_rx = None;
                self.update.status_message = format!("Basalt update failed: {}", error);
            }
            Err(TryRecvError::Disconnected) => {
                self.update.install_rx = None;
                self.update.status_message =
                    "Basalt update failed: background task disconnected".to_string();
            }
            Err(TryRecvError::Empty) => {}
        }
    }
}
