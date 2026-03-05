use std::sync::mpsc::TryRecvError;

use crate::core::GameEntry;

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn poll_startup_games_load(&mut self) {
        let poll_result = self
            .startup_games_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());

        let Some(received) = poll_result else {
            return;
        };

        match received {
            Ok(Ok(games)) => {
                self.startup_games_rx = None;
                self.apply_loaded_games(games);
                self.status_message.clear();
            }
            Ok(Err(err)) => {
                self.startup_games_rx = None;
                self.games.clear();
                self.selected_index = None;
                self.status_message = format!("Failed to load games: {}", err);
            }
            Err(TryRecvError::Disconnected) => {
                self.startup_games_rx = None;
                self.status_message = "Failed to load games: background task disconnected"
                    .to_string();
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    pub(super) fn apply_loaded_games(&mut self, games: Vec<GameEntry>) {
        self.games = games;
        if let Some(index) = self.selected_index {
            if index >= self.games.len() {
                self.selected_index = None;
            }
        }

        self.artwork_store.prepare_for_games(&self.games);
        self.refresh_playlists();
    }
}
