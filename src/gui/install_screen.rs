use eframe::egui::{
    self, vec2, CentralPanel, Color32, Frame, Layout, Margin, Sense, SidePanel, Stroke,
    RichText, StrokeKind,
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
                ui.with_layout(Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
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
                        let portrait_container = icon_rect.shrink(8.0);
                        let draw_rect = aspect_fit_rect(
                            portrait_container,
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

fn aspect_fit_rect(container: egui::Rect, image_width: f32, image_height: f32) -> egui::Rect {
    if image_width <= 0.0 || image_height <= 0.0 {
        return container;
    }

    let container_width = container.width();
    let container_height = container.height();
    if container_width <= 0.0 || container_height <= 0.0 {
        return container;
    }

    let width_scale = container_width / image_width;
    let height_scale = container_height / image_height;
    let scale = width_scale.min(height_scale);

    let draw_width = image_width * scale;
    let draw_height = image_height * scale;
    egui::Rect::from_center_size(container.center(), egui::vec2(draw_width, draw_height))
}

fn center_crop_uv(
    image_width: f32,
    image_height: f32,
    target_width: f32,
    target_height: f32,
) -> egui::Rect {
    if image_width <= 0.0 || image_height <= 0.0 || target_width <= 0.0 || target_height <= 0.0 {
        return egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    }

    let image_aspect = image_width / image_height;
    let target_aspect = target_width / target_height;

    if image_aspect > target_aspect {
        let normalized_width = target_aspect / image_aspect;
        let x_min = (1.0 - normalized_width) * 0.5;
        let x_max = x_min + normalized_width;
        return egui::Rect::from_min_max(egui::pos2(x_min, 0.0), egui::pos2(x_max, 1.0));
    }

    let normalized_height = image_aspect / target_aspect;
    let y_min = (1.0 - normalized_height) * 0.5;
    let y_max = y_min + normalized_height;
    egui::Rect::from_min_max(egui::pos2(0.0, y_min), egui::pos2(1.0, y_max))
}
