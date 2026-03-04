use eframe::egui::{
    self, vec2, CentralPanel, Color32, Frame, Layout, Margin, Sense, SidePanel, Stroke,
    RichText, StrokeKind,
};

use super::app::BasaltApp;
use super::search;
use super::tile_math::{center_crop_uv, fit_height_rect};

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
                let body_text_size = 16.0;
                let secondary_text_size = 15.0;

                ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
                    ui.heading("MattMC Install");
                    ui.separator();
                    ui.label(
                        RichText::new("Install or update MattMC into ~/Documents/MattMC.")
                            .size(body_text_size),
                    );

                    if ui.button(RichText::new("Install").size(body_text_size)).clicked() {
                        self.install_mattmc_from_gui();
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.label(RichText::new("Status").size(body_text_size));
                    if self.install_status_message.is_empty() {
                        ui.label(RichText::new("Ready").size(secondary_text_size));
                    } else {
                        ui.label(RichText::new(&self.install_status_message).size(secondary_text_size));
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
                ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                    let show_mattmc_tile = search::matches_query("MattMC", &self.install_search_query);
                    if !show_mattmc_tile {
                        ui.label(RichText::new("No install entries match your search.").size(15.0));
                        return;
                    }

                    const TILE_ART_SIZE: f32 = 180.0;
                    const TEXT_STRIP_HEIGHT: f32 = 42.0;
                    const TILE_WIDTH: f32 = TILE_ART_SIZE;
                    const TILE_HEIGHT: f32 = TILE_ART_SIZE + TEXT_STRIP_HEIGHT;

                    let (tile_rect, _) =
                        ui.allocate_exact_size(vec2(TILE_WIDTH, TILE_HEIGHT), Sense::hover());
                    ui.painter()
                        .rect_stroke(tile_rect, 0.0, white_line, StrokeKind::Inside);

                    let icon_rect =
                        egui::Rect::from_min_size(tile_rect.min, egui::vec2(TILE_ART_SIZE, TILE_ART_SIZE));

                    if let Some(artwork) = self.artwork_store.mattmc_artwork(ui.ctx()) {
                        let [bg_width, bg_height] = artwork.background_blur.size();
                        let bg_uv = center_crop_uv(
                            bg_width as f32,
                            bg_height as f32,
                            icon_rect.width(),
                            icon_rect.height(),
                        );
                        ui.painter().image(
                            artwork.background_blur.id(),
                            icon_rect,
                            bg_uv,
                            Color32::WHITE,
                        );

                        ui.painter().rect_filled(
                            icon_rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(0, 0, 0, 45),
                        );

                        let [fg_width, fg_height] = artwork.foreground.size();
                        let draw_rect = fit_height_rect(
                            icon_rect,
                            fg_width as f32,
                            fg_height as f32,
                        );

                        ui.painter().image(
                            artwork.foreground.id(),
                            draw_rect,
                            egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            ),
                            Color32::WHITE,
                        );

                        ui.painter().rect_stroke(
                            draw_rect,
                            0.0,
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 120)),
                            StrokeKind::Inside,
                        );
                    }

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
