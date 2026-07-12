use std::collections::BTreeMap;

use eframe::egui::{
    self, vec2, Align, CentralPanel, Color32, FontId, Frame, Layout, Margin, RichText, Sense,
    SidePanel,
};
use gilrs::{Axis, Button, EventType};

use crate::core::{self, EmulationLaunchTarget, GameEntry};

use super::app::BasaltApp;
use super::game_tile::paint_game_tile;
use super::tile_grid;

impl BasaltApp {
    pub(super) fn render_library_screen(
        &mut self,
        ctx: &egui::Context,
        main_region_gray: Color32,
        right_region_gray: Color32,
        right_panel_width: f32,
    ) {
        let can_start_background_job = !self.has_background_job();
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
                        ui.label(
                            RichText::new(format!("Name: {}", selected.name)).size(body_text_size),
                        );
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
                                    self.library.status_message =
                                        format!("Launched {}", selected.name);
                                }
                                Err(err) => {
                                    self.library.status_message = format!("Launch failed: {}", err);
                                }
                            }
                        }

                        let is_favorited = self.is_game_favorited(&selected.name);
                        let favorite_label = if is_favorited {
                            "Unfavorite"
                        } else {
                            "Favorite"
                        };

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
                                    .add_enabled(
                                        can_start_background_job,
                                        egui::Button::new(
                                            RichText::new("SyncUp").size(body_text_size),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.sync_mattmc_up_from_gui();
                                }

                                if ui
                                    .add_enabled(
                                        can_start_background_job,
                                        egui::Button::new(
                                            RichText::new("SyncDown").size(body_text_size),
                                        ),
                                    )
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
                    if self.library.status_message.is_empty() {
                        ui.label(RichText::new("Ready").size(secondary_text_size));
                    } else {
                        ui.label(
                            RichText::new(&self.library.status_message).size(secondary_text_size),
                        );
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
                if let Some(selected_index) = self.library.selected_index {
                    if !filtered_indices.contains(&selected_index) {
                        self.library.selected_index = None;
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
        let categorized_indices = self.categorize_filtered_indices(filtered_indices);
        let columns = tile_grid::column_count(ui);
        self.process_library_controller_input(&categorized_indices, columns);
        let has_empty_search = self.library.search_query.trim().is_empty();

        tile_grid::show_categorized_grid(
            ui,
            "library",
            &categorized_indices,
            |ui| {
                if has_empty_search {
                    ui.label("No games found. Use Add, Discover, or CLI commands to add entries.");
                } else {
                    ui.label("No games match your search.");
                }
            },
            |ui, category_indices, columns| {
                self.render_category_tile_rows(ui, category_indices, columns);
            },
        );
    }

    fn process_library_controller_input(
        &mut self,
        categorized_indices: &[(String, Vec<usize>)],
        columns: usize,
    ) {
        let Some(controller) = self.controller.gilrs.as_mut() else {
            return;
        };

        let mut move_x = 0i32;
        let mut move_y = 0i32;
        let mut launch_selected = false;

        while let Some(event) = controller.next_event() {
            match event.event {
                EventType::ButtonPressed(Button::DPadLeft, _) => move_x -= 1,
                EventType::ButtonPressed(Button::DPadRight, _) => move_x += 1,
                EventType::ButtonPressed(Button::DPadUp, _) => move_y -= 1,
                EventType::ButtonPressed(Button::DPadDown, _) => move_y += 1,
                EventType::ButtonPressed(Button::South, _) => launch_selected = true,
                EventType::AxisChanged(Axis::LeftStickX, value, _) => {
                    if value <= -0.6 && !self.controller.stick_x_held {
                        move_x -= 1;
                        self.controller.stick_x_held = true;
                    } else if value >= 0.6 && !self.controller.stick_x_held {
                        move_x += 1;
                        self.controller.stick_x_held = true;
                    } else if value.abs() < 0.3 {
                        self.controller.stick_x_held = false;
                    }
                }
                EventType::AxisChanged(Axis::LeftStickY, value, _) => {
                    if value <= -0.6 && !self.controller.stick_y_held {
                        move_y += 1;
                        self.controller.stick_y_held = true;
                    } else if value >= 0.6 && !self.controller.stick_y_held {
                        move_y -= 1;
                        self.controller.stick_y_held = true;
                    } else if value.abs() < 0.3 {
                        self.controller.stick_y_held = false;
                    }
                }
                _ => {}
            }
        }

        let mut ordered_indices = Vec::new();
        for (_, category_indices) in categorized_indices {
            ordered_indices.extend(category_indices.iter().copied());
        }

        if ordered_indices.is_empty() {
            self.library.selected_index = None;
            return;
        }

        let has_navigation_input = move_x != 0 || move_y != 0;
        if (has_navigation_input || launch_selected) && self.library.selected_index.is_none() {
            self.library.selected_index = Some(ordered_indices[0]);
            self.library.pending_scroll_to_selected = true;
        }

        if has_navigation_input {
            let max_index = ordered_indices.len().saturating_sub(1);
            let mut selected_position = self
                .library
                .selected_index
                .and_then(|selected| ordered_indices.iter().position(|&index| index == selected))
                .unwrap_or(0);

            if move_x < 0 {
                selected_position = selected_position.saturating_sub(1);
            } else if move_x > 0 {
                selected_position = selected_position.saturating_add(1).min(max_index);
            }

            if move_y < 0 {
                selected_position = selected_position.saturating_sub(columns.max(1));
            } else if move_y > 0 {
                selected_position = selected_position
                    .saturating_add(columns.max(1))
                    .min(max_index);
            }

            let next_selection = ordered_indices[selected_position];
            if self.library.selected_index != Some(next_selection) {
                self.library.selected_index = Some(next_selection);
                self.library.pending_scroll_to_selected = true;
            }
        }

        if launch_selected {
            if let Some(selected_game) = self.selected_game().cloned() {
                match core::launch_game(&selected_game.name) {
                    Ok(_) => {
                        self.library.status_message = format!("Launched {}", selected_game.name);
                    }
                    Err(err) => {
                        self.library.status_message = format!("Launch failed: {}", err);
                    }
                }
            }
        }
    }

    fn render_category_tile_rows(
        &mut self,
        ui: &mut egui::Ui,
        category_indices: &[usize],
        columns: usize,
    ) {
        tile_grid::show_tile_rows(ui, category_indices, columns, |ui, game_index| {
            let game = self.library.games[*game_index].clone();
            let is_selected = self.library.selected_index == Some(*game_index);
            if self.render_tile(
                ui,
                tile_grid::TILE_WIDTH,
                tile_grid::TILE_HEIGHT,
                &game,
                is_selected,
            ) {
                self.library.selected_index = Some(*game_index);
            }
        });
    }

    fn categorize_filtered_indices(&self, filtered_indices: &[usize]) -> Vec<(String, Vec<usize>)> {
        let mut steam_indices = Vec::new();
        let mut my_games_indices = Vec::new();
        let mut emulator_sections: BTreeMap<String, Vec<usize>> = BTreeMap::new();

        for &game_index in filtered_indices {
            let game = &self.library.games[game_index];
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

        let (tile_rect, response) =
            ui.allocate_exact_size(vec2(tile_width, tile_height), Sense::click());

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

        if selected && self.library.pending_scroll_to_selected {
            response.scroll_to_me(Some(Align::Center));
            self.library.pending_scroll_to_selected = false;
        }

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
