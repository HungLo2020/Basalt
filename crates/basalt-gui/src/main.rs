#![deny(clippy::disallowed_methods, clippy::disallowed_types)]
// Release builds are GUI-only on Windows: don't open a console window alongside the launcher.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod app_actions;
mod app_startup;
mod app_state;
mod artwork;
mod artwork_loader;
mod background_jobs;
mod game_launches;
mod game_tile;
mod image_prep;
mod install_screen;
mod library_screen;
mod search;
mod settings_screen;
mod tile_grid;
mod top_bar;
mod update_actions;

use std::process::ExitCode;

fn main() -> ExitCode {
    match app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_message) => {
            eprintln!("Error: {}", error_message);
            ExitCode::FAILURE
        }
    }
}
