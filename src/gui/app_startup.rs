use std::sync::mpsc::TryRecvError;

use crate::core::GameEntry;

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn poll_startup_games_load(&mut self) {
        let poll_result = self
            .startup_load
            .games_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());

        let Some(received) = poll_result else {
            return;
        };

        match received {
            Ok(Ok(games)) => {
                self.startup_load.games_rx = None;
                self.apply_loaded_games(games);
                self.library.status_message.clear();
            }
            Ok(Err(err)) => {
                self.startup_load.games_rx = None;
                self.library.games.clear();
                self.library.selected_index = None;
                self.library.status_message = format!("Failed to load games: {}", err);
            }
            Err(TryRecvError::Disconnected) => {
                self.startup_load.games_rx = None;
                self.library.status_message =
                    "Failed to load games: background task disconnected".to_string();
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    pub(super) fn apply_loaded_games(&mut self, games: Vec<GameEntry>) {
        self.library.games = games;
        if let Some(index) = self.library.selected_index {
            if index >= self.library.games.len() {
                self.library.selected_index = None;
            }
        }

        self.artwork_store.prepare_for_games(&self.library.games);
        self.refresh_playlists();
    }
}
