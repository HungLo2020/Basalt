use eframe::egui::{self, Color32, RichText};

pub(super) fn render_search_field(
    ui: &mut egui::Ui,
    query: &mut String,
    hint_text: &str,
    text_size: f32,
) {
    ui.horizontal(|ui| {
        let available_width = ui.available_width().max(120.0);
        let show_clear_button = available_width >= 210.0;
        let clear_button_width = if show_clear_button { 56.0 } else { 0.0 };
        let text_edit_width = (available_width - clear_button_width).max(110.0);

        let text_edit = egui::TextEdit::singleline(query)
            .desired_width(text_edit_width)
            .background_color(Color32::from_rgb(63, 73, 88))
            .hint_text(hint_text);
        ui.add_sized([text_edit_width, 28.0], text_edit);

        if show_clear_button {
            if ui
                .add_sized(
                    [clear_button_width, 28.0],
                    egui::Button::new(RichText::new("Clear").size((text_size - 1.0).max(12.0))),
                )
                .clicked()
            {
                query.clear();
            }
        }
    });
}

pub(super) fn matches_query(value: &str, query: &str) -> bool {
    let normalized_query = query.trim().to_lowercase();
    if normalized_query.is_empty() {
        return true;
    }

    value.to_lowercase().contains(&normalized_query)
}
