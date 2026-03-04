mod artwork;
mod app;
mod app_actions;
mod app_startup;
mod install_screen;
mod library_screen;
mod search;
mod tile_math;
mod top_bar;

pub fn run() -> Result<(), String> {
    app::run()
}
