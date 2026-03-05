use eframe::egui::{self, Color32};

pub(super) fn render_search_field(
    ui: &mut egui::Ui,
    query: &mut String,
    hint_text: &str,
    _text_size: f32,
) {
    ui.horizontal(|ui| {
        let available_width = ui.available_width().max(120.0);
        let text_edit_width = available_width.max(110.0);

        let text_edit = egui::TextEdit::singleline(query)
            .desired_width(text_edit_width)
            .background_color(Color32::from_rgb(63, 73, 88))
            .hint_text(hint_text);
        ui.add_sized([text_edit_width, 25.0], text_edit);
    });
}

pub(super) fn matches_query(value: &str, query: &str) -> bool {
    let normalized_query = query.trim().to_lowercase();
    if normalized_query.is_empty() {
        return true;
    }

    value.to_lowercase().contains(&normalized_query)
}
