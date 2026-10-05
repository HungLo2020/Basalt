use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Stroke, StrokeKind, Ui,
};

use super::artwork::ArtworkTextures;

pub(super) fn paint_game_tile(
    ui: &mut Ui,
    tile_rect: Rect,
    text_strip_height: f32,
    title: &str,
    title_font_size: f32,
    artwork: Option<&ArtworkTextures>,
    selected: bool,
) {
    let corner_radius = 11;
    let tile_rounding = CornerRadius::same(corner_radius);
    let artwork_rounding = CornerRadius {
        nw: corner_radius,
        ne: corner_radius,
        sw: 0,
        se: 0,
    };
    let text_rounding = CornerRadius {
        nw: 0,
        ne: 0,
        sw: corner_radius,
        se: corner_radius,
    };

    let icon_size = (tile_rect.height() - text_strip_height).max(0.0);
    let icon_rect = Rect::from_min_size(tile_rect.min, egui::vec2(tile_rect.width(), icon_size));
    let text_rect = Rect::from_min_max(
        egui::pos2(tile_rect.min.x, tile_rect.max.y - text_strip_height),
        tile_rect.max,
    );

    ui.painter()
        .rect_filled(icon_rect, artwork_rounding, Color32::from_rgb(36, 42, 52));

    if let Some(artwork) = artwork {
        egui::Image::from_texture(&artwork.foreground)
            .fit_to_exact_size(icon_rect.size())
            .maintain_aspect_ratio(true)
            .corner_radius(artwork_rounding)
            .bg_fill(Color32::from_rgb(36, 42, 52))
            .paint_at(ui, icon_rect);
    }

    ui.painter().rect_stroke(
        icon_rect,
        artwork_rounding,
        Stroke::new(1.0, Color32::from_rgb(52, 60, 72)),
        StrokeKind::Inside,
    );

    ui.painter()
        .rect_filled(text_rect, text_rounding, Color32::from_rgb(52, 60, 72));

    ui.painter().text(
        text_rect.center(),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(title_font_size),
        Color32::WHITE,
    );

    let tile_stroke = if selected {
        Stroke::new(3.0, Color32::WHITE)
    } else {
        Stroke::new(1.0, Color32::from_rgb(52, 60, 72))
    };

    ui.painter()
        .rect_stroke(tile_rect, tile_rounding, tile_stroke, StrokeKind::Inside);
}
