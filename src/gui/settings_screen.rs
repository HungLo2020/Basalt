use eframe::egui::{self, CentralPanel, Color32, Frame, Margin, RichText, SidePanel, Stroke};

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn render_settings_screen(
        &mut self,
        ctx: &egui::Context,
        region_gray: Color32,
        white_line: Stroke,
        right_panel_width: f32,
    ) {
        SidePanel::right("settings_right_panel")
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
                ui.heading("Settings");
                ui.separator();
                ui.label(RichText::new("Settings coming soon."));
            });

        CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(white_line),
            )
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();
                ui.label("No settings are available yet.");
            });
    }
}
