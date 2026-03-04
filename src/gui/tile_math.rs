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

pub(super) fn center_crop_uv(
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
