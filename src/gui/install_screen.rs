use eframe::egui::{
    self, vec2, CentralPanel, Color32, Frame, Layout, Margin, Sense, SidePanel, Stroke,
    StrokeKind,
};

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn render_install_screen(
        &mut self,
        ctx: &egui::Context,
        region_gray: Color32,
        white_line: Stroke,
        right_panel_width: f32,
    ) {
        SidePanel::right("install_right_panel")
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
                    ui.heading("MattMC Install");
                    ui.separator();
                    ui.label("Install or update MattMC into ~/Documents/MattMC.");

                    if ui.button("Install").clicked() {
                        self.install_mattmc_from_gui();
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.label("Status");
                    if self.install_status_message.is_empty() {
                        ui.small("Ready");
                    } else {
                        ui.small(&self.install_status_message);
                    }
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
                ui.with_layout(Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                    const TILE_WIDTH: f32 = 170.0;
                    const TILE_HEIGHT: f32 = 170.0;
                    const TEXT_STRIP_HEIGHT: f32 = 36.0;

                    let (tile_rect, _) =
                        ui.allocate_exact_size(vec2(TILE_WIDTH, TILE_HEIGHT), Sense::hover());
                    ui.painter()
                        .rect_stroke(tile_rect, 0.0, white_line, StrokeKind::Inside);

                    let icon_rect = egui::Rect::from_min_max(
                        tile_rect.min,
                        egui::pos2(tile_rect.max.x, tile_rect.max.y - TEXT_STRIP_HEIGHT),
                    );
                    ui.painter()
                        .rect_stroke(icon_rect, 0.0, white_line, StrokeKind::Inside);

                    let text_rect = egui::Rect::from_min_max(
                        egui::pos2(tile_rect.min.x, tile_rect.max.y - TEXT_STRIP_HEIGHT),
                        tile_rect.max,
                    );

                    ui.painter().text(
                        text_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "MattMC",
                        egui::FontId::proportional(18.0),
                        Color32::WHITE,
                    );
                });
            });
    }
}
