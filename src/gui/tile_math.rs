use eframe::egui;

pub(super) fn fit_height_rect(container: egui::Rect, image_width: f32, image_height: f32) -> egui::Rect {
    if image_width <= 0.0 || image_height <= 0.0 {
        return container;
    }

    let container_height = container.height();
    if container_height <= 0.0 {
        return container;
    }

    let image_aspect = image_width / image_height;
    let draw_height = container_height;
    let draw_width = draw_height * image_aspect;
    egui::Rect::from_center_size(container.center(), egui::vec2(draw_width, draw_height))
}
