use eframe::egui::{
    self, CentralPanel, Color32, Direction, Frame, Layout, Margin, SidePanel, Stroke,
    TopBottomPanel,
};

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
        let region_gray = Color32::from_gray(55);
        let white_line = Stroke::new(1.0, Color32::WHITE);
        let window_width = ctx.screen_rect().width();
        let right_panel_width = window_width / 4.0;

        TopBottomPanel::top("top_bar")
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(10))
                    .stroke(white_line),
            )
            .exact_height(56.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Top Bar (future options)");
                });
            });

        SidePanel::right("right_panel")
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(white_line),
            )
            .min_width(right_panel_width)
            .max_width(right_panel_width)
            .resizable(false)
            .show(ctx, |ui| {
                ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
                    ui.label("Right Panel (reserved)");
                });
            });

        CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(white_line),
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::centered_and_justified(Direction::TopDown), |ui| {
                    ui.heading("Main Region");
                });
            });
    }
}
