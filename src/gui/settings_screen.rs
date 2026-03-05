use eframe::egui::{self, CentralPanel, Color32, Frame, Margin, RichText, SidePanel};

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn render_settings_screen(
        &mut self,
        ctx: &egui::Context,
        main_region_gray: Color32,
        right_region_gray: Color32,
        right_panel_width: f32,
    ) {
        SidePanel::right("settings_right_panel")
            .frame(
                Frame::new()
                    .fill(right_region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(egui::Stroke::NONE),
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
                    .fill(main_region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();
                ui.label("No settings are available yet.");
            });
    }
}
