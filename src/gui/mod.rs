mod app;
mod app_actions;
mod app_startup;
mod artwork;
mod background_jobs;
mod game_tile;
mod install_screen;
mod library_screen;
mod search;
mod settings_screen;
mod top_bar;
mod update_actions;

pub fn run() -> Result<(), String> {
    app::run()
}
