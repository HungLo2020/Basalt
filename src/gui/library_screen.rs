use std::collections::BTreeMap;

use eframe::egui::{
    self, vec2, CentralPanel, Color32, FontId, Frame, Layout, Margin, ScrollArea, Sense,
    RichText, SidePanel,
};

use crate::core::{self, EmulationLaunchTarget, GameEntry};

use super::app::BasaltApp;
use super::game_tile::paint_game_tile;

impl BasaltApp {
    pub(super) fn render_library_screen(
        &mut self,
        ctx: &egui::Context,
        main_region_gray: Color32,
        right_region_gray: Color32,
        right_panel_width: f32,
    ) {
        SidePanel::right("right_panel")
            .frame(
                Frame::new()
                    .fill(right_region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(egui::Stroke::NONE),
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
                            .add(
                                egui::Button::new(RichText::new("Play").size(body_text_size))
                                    .fill(Color32::DARK_GREEN),
                            )
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

                        let is_favorited = self.is_game_favorited(&selected.name);
                        let favorite_label = if is_favorited { "Unfavorite" } else { "Favorite" };

                        if ui
                            .button(RichText::new(favorite_label).size(body_text_size))
                            .clicked()
                        {
                            self.set_game_favorited_from_gui(&selected.name, !is_favorited);
                        }

                        if ui
                            .button(RichText::new("Remove").size(body_text_size))
                            .clicked()
                        {
                            self.remove_game_from_gui(&selected.name);
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
                    .fill(main_region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                let filtered_indices = self.filtered_library_indices();
                if let Some(selected_index) = self.selected_index {
                    if !filtered_indices.contains(&selected_index) {
                        self.selected_index = None;
                    }
                }

                self.render_game_grid(ui, &filtered_indices);
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

    fn render_game_grid(&mut self, ui: &mut egui::Ui, filtered_indices: &[usize]) {
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

            if filtered_indices.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(WALL_PADDING);
                    if self.library_search_query.trim().is_empty() {
                        ui.label("No games found. Use Add, Discover, or CLI commands to add entries.");
                    } else {
                        ui.label("No games match your search.");
                    }
                    ui.add_space(WALL_PADDING + SCROLLBAR_GUTTER);
                });
                ui.add_space(WALL_PADDING);
                return;
            }

            let categorized_indices = self.categorize_filtered_indices(filtered_indices);

            for (category_position, (category_name, category_indices))
                in categorized_indices.iter().enumerate()
            {
                let header_text = format!("{} ({})", category_name, category_indices.len());
                let collapsing = egui::CollapsingHeader::new(header_text)
                    .id_salt(format!("library_category_{}", category_name))
                    .default_open(true);

                collapsing.show(ui, |ui| {
                    ui.add_space(8.0);
                    self.render_category_tile_rows(
                        ui,
                        category_indices,
                        columns,
                        TILE_WIDTH,
                        TILE_HEIGHT,
                        TILE_SPACING,
                        WALL_PADDING,
                        SCROLLBAR_GUTTER,
                    );
                });

                if category_position + 1 < categorized_indices.len() {
                    ui.add_space(12.0);
                }
            }

            ui.add_space(WALL_PADDING);
        });
    }

    fn render_category_tile_rows(
        &mut self,
        ui: &mut egui::Ui,
        category_indices: &[usize],
        columns: usize,
        tile_width: f32,
        tile_height: f32,
        tile_spacing: f32,
        wall_padding: f32,
        scrollbar_gutter: f32,
    ) {
        let mut visible_index = 0usize;
        while visible_index < category_indices.len() {
            ui.horizontal(|ui| {
                ui.add_space(wall_padding);

                for col in 0..columns {
                    if visible_index >= category_indices.len() {
                        break;
                    }

                    let game_index = category_indices[visible_index];
                    let game = self.games[game_index].clone();
                    let is_selected = self.selected_index == Some(game_index);
                    if self.render_tile(ui, tile_width, tile_height, &game, is_selected) {
                        self.selected_index = Some(game_index);
                    }

                    if col + 1 < columns && visible_index + 1 < category_indices.len() {
                        ui.add_space(tile_spacing);
                    }

                    visible_index += 1;
                }

                ui.add_space(wall_padding + scrollbar_gutter);
            });

            if visible_index < category_indices.len() {
                ui.add_space(tile_spacing);
            }
        }
    }

    fn categorize_filtered_indices(&self, filtered_indices: &[usize]) -> Vec<(String, Vec<usize>)> {
        let mut steam_indices = Vec::new();
        let mut my_games_indices = Vec::new();
        let mut emulator_sections: BTreeMap<String, Vec<usize>> = BTreeMap::new();

        for &game_index in filtered_indices {
            let game = &self.games[game_index];
            match game.runner_kind.as_str() {
                "steam" => steam_indices.push(game_index),
                "bash" => my_games_indices.push(game_index),
                "emulator" => {
                    let category_name = emulator_category_name(&game.launch_target);
                    emulator_sections
                        .entry(category_name)
                        .or_default()
                        .push(game_index);
                }
                _ => my_games_indices.push(game_index),
            }
        }

        let mut categories = Vec::new();
        if !steam_indices.is_empty() {
            categories.push(("Steam".to_string(), steam_indices));
        }
        if !my_games_indices.is_empty() {
            categories.push(("MyGames".to_string(), my_games_indices));
        }

        for (category_name, indices) in emulator_sections {
            if !indices.is_empty() {
                categories.push((category_name, indices));
            }
        }

        categories
    }

    fn render_tile(
        &mut self,
        ui: &mut egui::Ui,
        tile_width: f32,
        tile_height: f32,
        game: &GameEntry,
        selected: bool,
    ) -> bool {
        let text_strip_height = (tile_height - tile_width).max(24.0);

        let (tile_rect, response) = ui.allocate_exact_size(vec2(tile_width, tile_height), Sense::click());

        let title_max_width = (tile_rect.width() - 8.0).max(8.0);
        let title_size = self.fit_title_font_size(ui, &game.name, title_max_width);
        let artwork = self.artwork_store.artwork_for_game(ui.ctx(), game);
        paint_game_tile(
            ui,
            tile_rect,
            text_strip_height,
            &game.name,
            title_size,
            artwork.as_ref(),
            selected,
        );

        response.clicked()
    }
}

fn emulator_category_name(launch_target: &str) -> String {
    let system_owned = EmulationLaunchTarget::decode(launch_target)
        .ok()
        .map(|target| target.system_key().to_string())
        .unwrap_or_default();
    let system = system_owned.trim();

    if system.is_empty() {
        "Emulation".to_string()
    } else {
        system.to_uppercase()
    }
}
