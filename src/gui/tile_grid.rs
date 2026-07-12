use eframe::egui::{self, ScrollArea};

pub(super) const TILE_WIDTH: f32 = 150.0;
pub(super) const TEXT_STRIP_HEIGHT: f32 = 40.0;
pub(super) const TILE_HEIGHT: f32 = TILE_WIDTH + TEXT_STRIP_HEIGHT;
pub(super) const TILE_SPACING: f32 = 24.0;
pub(super) const WALL_PADDING: f32 = 24.0;
pub(super) const SCROLLBAR_GUTTER: f32 = 18.0;

pub(super) fn column_count(ui: &egui::Ui) -> usize {
    let usable_width =
        (ui.available_width() - (WALL_PADDING * 2.0) - SCROLLBAR_GUTTER).max(TILE_WIDTH);
    let columns = ((usable_width + TILE_SPACING) / (TILE_WIDTH + TILE_SPACING)).floor() as usize;
    columns.max(1)
}

pub(super) fn show_categorized_grid<T, C, R, E>(
    ui: &mut egui::Ui,
    id_prefix: &str,
    categories: &[(C, Vec<T>)],
    empty_message: E,
    mut render_rows: R,
) where
    C: AsRef<str>,
    R: FnMut(&mut egui::Ui, &[T], usize),
    E: FnOnce(&mut egui::Ui),
{
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add_space(WALL_PADDING);

            if categories.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(WALL_PADDING);
                    empty_message(ui);
                    ui.add_space(WALL_PADDING + SCROLLBAR_GUTTER);
                });
                ui.add_space(WALL_PADDING);
                return;
            }

            let columns = column_count(ui);
            for (category_position, (category_name, category_items)) in
                categories.iter().enumerate()
            {
                let header_text = format!("{} ({})", category_name.as_ref(), category_items.len());
                let collapsing = egui::CollapsingHeader::new(header_text)
                    .id_salt(format!("{}_category_{}", id_prefix, category_name.as_ref()))
                    .default_open(true);

                collapsing.show(ui, |ui| {
                    ui.add_space(8.0);
                    render_rows(ui, category_items, columns);
                });

                if category_position + 1 < categories.len() {
                    ui.add_space(12.0);
                }
            }

            ui.add_space(WALL_PADDING);
        });
}

pub(super) fn show_tile_rows<T, R>(
    ui: &mut egui::Ui,
    items: &[T],
    columns: usize,
    mut render_tile: R,
) where
    R: FnMut(&mut egui::Ui, &T),
{
    let mut visible_index = 0usize;
    while visible_index < items.len() {
        ui.horizontal(|ui| {
            ui.add_space(WALL_PADDING);

            for col in 0..columns {
                if visible_index >= items.len() {
                    break;
                }

                render_tile(ui, &items[visible_index]);

                if col + 1 < columns && visible_index + 1 < items.len() {
                    ui.add_space(TILE_SPACING);
                }

                visible_index += 1;
            }

            ui.add_space(WALL_PADDING + SCROLLBAR_GUTTER);
        });

        if visible_index < items.len() {
            ui.add_space(TILE_SPACING);
        }
    }
}
