use eframe::egui::{self, CentralPanel, Direction, Layout};

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Basalt",
        options,
        Box::new(|_cc| Ok(Box::new(BasaltApp::default()))),
    )
    .map_err(|err| format!("Failed to launch GUI: {}", err))
}

#[derive(Default)]
struct BasaltApp;

impl eframe::App for BasaltApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::centered_and_justified(Direction::TopDown), |ui| {
                ui.heading("Hello world");
            });
        });
    }
}
