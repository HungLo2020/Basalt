use eframe::egui::{
    self, CentralPanel, Color32, Frame, Layout, Margin, Stroke,
};

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn render_install_screen(
        &mut self,
        ctx: &egui::Context,
        region_gray: Color32,
        white_line: Stroke,
    ) {
        CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(white_line),
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                    ui.heading("Hello world");
                });
            });
    }
}
