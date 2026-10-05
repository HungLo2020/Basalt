//! Launching games without blocking the UI.
//!
//! `core::launch_game` waits for the game process to exit (so failures can be reported), which
//! can take hours. Each launch therefore runs on its own thread and reports back through a
//! channel, waking the UI when it finishes. Launches are independent of the single background
//! job slot, so playing a game never blocks installs or syncs.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use basalt_core::{self as core, CoreResult};

use super::app::BasaltApp;

pub(super) struct GameLaunchState {
    result_tx: Sender<(String, CoreResult<()>)>,
    result_rx: Receiver<(String, CoreResult<()>)>,
    running: HashSet<String>,
}

impl Default for GameLaunchState {
    fn default() -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        Self {
            result_tx,
            result_rx,
            running: HashSet::new(),
        }
    }
}

impl BasaltApp {
    pub(super) fn is_game_running(&self, game_name: &str) -> bool {
        self.launches.running.contains(game_name)
    }

    pub(super) fn launch_game_from_gui(&mut self, game_name: &str) {
        if self.is_game_running(game_name) {
            self.library.status_message = format!("{} is already running", game_name);
            return;
        }

        let game_name = game_name.to_string();
        self.launches.running.insert(game_name.clone());
        self.library.status_message = format!("Launched {}", game_name);

        let result_tx = self.launches.result_tx.clone();
        let egui_ctx = self.egui_ctx.clone();
        thread::spawn(move || {
            let result = core::launch_game(&game_name);
            let _ = result_tx.send((game_name, result));
            egui_ctx.request_repaint();
        });
    }

    pub(super) fn poll_game_launches(&mut self) {
        while let Ok((game_name, result)) = self.launches.result_rx.try_recv() {
            self.launches.running.remove(&game_name);
            if let Err(err) = result {
                self.library.status_message = format!("Launch of {} failed: {}", game_name, err);
            }
        }
    }
}
