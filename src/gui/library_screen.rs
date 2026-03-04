use eframe::egui::{
    self, vec2, Align2, CentralPanel, Color32, FontId, Frame, Layout, Margin, ScrollArea, Sense,
    RichText, SidePanel, Stroke, StrokeKind,
};

use crate::core::{self, GameEntry};

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn render_library_screen(
        &mut self,
        ctx: &egui::Context,
        region_gray: Color32,
        white_line: Stroke,
        right_panel_width: f32,
    ) {
        SidePanel::right("right_panel")
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
                    ui.heading("Details");
                    ui.separator();

                    if let Some(selected) = self.selected_game().cloned() {
                        ui.label(RichText::new(format!("Name: {}", selected.name)).size(body_text_size));
                        ui.label(
                            RichText::new(format!("Runner: {}", selected.runner_kind.as_str()))
                                .size(body_text_size),
                        );
                        ui.label(RichText::new("Target:").size(body_text_size));
                        ui.label(
                            RichText::new(&selected.launch_target)
                                .size(secondary_text_size)
                                .monospace(),
                        );

                        if ui
                            .button(RichText::new("Launch Selected").size(body_text_size))
                            .clicked()
                        {
                            match core::launch_game(&selected.name) {
                                Ok(_) => {
                                    self.status_message = format!("Launched {}", selected.name);
                                }
                                Err(err) => {
                                    self.status_message = format!("Launch failed: {}", err);
                                }
                            }
                        }

                        if selected.name.eq_ignore_ascii_case("MattMC") {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .button(RichText::new("SyncUp").size(body_text_size))
                                    .clicked()
                                {
                                    self.sync_mattmc_up_from_gui();
                                }

                                if ui
                                    .button(RichText::new("SyncDown").size(body_text_size))
                                    .clicked()
                                {
                                    self.sync_mattmc_down_from_gui();
                                }
                            });
                        }
                    } else {
                        ui.label(
                            RichText::new("Select a game tile to view details.")
                                .size(secondary_text_size),
                        );
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.label(RichText::new("Add Game Inputs").size(body_text_size));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Name").size(body_text_size));
                        ui.text_edit_singleline(&mut self.add_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Script").size(body_text_size));
                        ui.text_edit_singleline(&mut self.add_script_path);
                    });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.label(RichText::new("Status").size(body_text_size));
                    if self.status_message.is_empty() {
                        ui.label(RichText::new("Ready").size(secondary_text_size));
                    } else {
                        ui.label(RichText::new(&self.status_message).size(secondary_text_size));
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
                self.render_game_grid(ui, white_line);
            });
    }

    fn fit_title_font_size(&self, ui: &egui::Ui, text: &str, max_width: f32) -> f32 {
        let max_size = 16.0;
        let min_size = 8.0;
        let mut size = max_size;

        while size >= min_size {
            let galley = ui.painter().layout_no_wrap(
                text.to_string(),
                FontId::proportional(size),
                Color32::WHITE,
            );

            if galley.size().x <= max_width {
                return size;
            }

            size -= 0.5;
        }

        min_size
    }

    fn render_game_grid(&mut self, ui: &mut egui::Ui, border_stroke: Stroke) {
        const TILE_WIDTH: f32 = 150.0;
        const TEXT_STRIP_HEIGHT: f32 = 40.0;
        const TILE_HEIGHT: f32 = TILE_WIDTH + TEXT_STRIP_HEIGHT;
        const TILE_SPACING: f32 = 24.0;
        const WALL_PADDING: f32 = 24.0;
        const SCROLLBAR_GUTTER: f32 = 18.0;

        let usable_width =
            (ui.available_width() - (WALL_PADDING * 2.0) - SCROLLBAR_GUTTER).max(TILE_WIDTH);
        let columns = ((usable_width + TILE_SPACING) / (TILE_WIDTH + TILE_SPACING)).floor() as usize;
        let columns = columns.max(1);

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add_space(WALL_PADDING);

            if self.games.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(WALL_PADDING);
                    ui.label("No games found. Use Add, Discover, or CLI commands to add entries.");
                    ui.add_space(WALL_PADDING + SCROLLBAR_GUTTER);
                });
                ui.add_space(WALL_PADDING);
                return;
            }

            let mut index = 0usize;
            while index < self.games.len() {
                ui.horizontal(|ui| {
                    ui.add_space(WALL_PADDING);

                    for col in 0..columns {
                        if index >= self.games.len() {
                            break;
                        }

                        let game = self.games[index].clone();
                        let is_selected = self.selected_index == Some(index);
                        if self.render_tile(
                            ui,
                            border_stroke,
                            TILE_WIDTH,
                            TILE_HEIGHT,
                            &game,
                            is_selected,
                        ) {
                            self.selected_index = Some(index);
                        }

                        if col + 1 < columns && index + 1 < self.games.len() {
                            ui.add_space(TILE_SPACING);
                        }

                        index += 1;
                    }

                    ui.add_space(WALL_PADDING + SCROLLBAR_GUTTER);
                });

                if index < self.games.len() {
                    ui.add_space(TILE_SPACING);
                }
            }

            ui.add_space(WALL_PADDING);
        });
    }

    fn render_tile(
        &mut self,
        ui: &mut egui::Ui,
        border_stroke: Stroke,
        tile_width: f32,
        tile_height: f32,
        game: &GameEntry,
        selected: bool,
    ) -> bool {
        let text_strip_height = (tile_height - tile_width).max(24.0);

        let (tile_rect, response) = ui.allocate_exact_size(vec2(tile_width, tile_height), Sense::click());

        let tile_stroke = if selected {
            Stroke::new(2.0, Color32::WHITE)
        } else {
            border_stroke
        };
        ui.painter().rect_stroke(tile_rect, 0.0, tile_stroke, StrokeKind::Inside);

        let icon_rect = egui::Rect::from_min_size(tile_rect.min, egui::vec2(tile_width, tile_width));

        if let Some(artwork) = self.artwork_store.artwork_for_game(ui.ctx(), game) {
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
            let portrait_container = icon_rect.shrink(6.0);
            let draw_rect = aspect_fit_rect(
                portrait_container,
                fg_width as f32,
                fg_height as f32,
            );

            ui.painter().image(
                artwork.foreground.id(),
                draw_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
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
            .rect_stroke(icon_rect, 0.0, border_stroke, StrokeKind::Inside);

        let text_rect = egui::Rect::from_min_max(
            egui::pos2(tile_rect.min.x, tile_rect.max.y - text_strip_height),
            tile_rect.max,
        );

        let tile_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(text_rect)
                .layout(Layout::centered_and_justified(egui::Direction::TopDown)),
        );

        let title_max_width = (text_rect.width() - 8.0).max(8.0);
        let title_size = self.fit_title_font_size(&tile_ui, &game.name, title_max_width);
        tile_ui.painter().text(
            text_rect.center(),
            Align2::CENTER_CENTER,
            &game.name,
            FontId::proportional(title_size),
            Color32::WHITE,
        );

        response.clicked()
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
