use eframe::egui::{
    self, vec2, Align2, CentralPanel, Color32, FontId, Frame, Layout, Margin, ScrollArea, Sense,
    SidePanel, Stroke, StrokeKind, TopBottomPanel,
};

use crate::core::{self, DiscoverResult, GameEntry};

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Basalt",
        options,
        Box::new(|_cc| Ok(Box::new(BasaltApp::default()))),
    )
    .map_err(|err| format!("Failed to launch GUI: {}", err))
}

struct BasaltApp {
    games: Vec<GameEntry>,
    selected_index: Option<usize>,
    add_name: String,
    add_script_path: String,
    status_message: String,
}

impl Default for BasaltApp {
    fn default() -> Self {
        let mut app = Self {
            games: Vec::new(),
            selected_index: None,
            add_name: String::new(),
            add_script_path: String::new(),
            status_message: String::new(),
        };
        app.refresh_games();
        app
    }
}

impl eframe::App for BasaltApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let region_gray = Color32::from_gray(55);
        let white_line = Stroke::new(1.0, Color32::WHITE);
        let window_width = ctx.screen_rect().width();
        let right_panel_width = window_width / 4.0;

        let mut trigger_add = false;
        let mut trigger_discover = false;
        let mut trigger_refresh = false;

        TopBottomPanel::top("top_bar")
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(10))
                    .stroke(white_line),
            )
            .exact_height(56.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        trigger_add = true;
                    }
                    if ui.button("Discover").clicked() {
                        trigger_discover = true;
                    }
                    if ui.button("Refresh").clicked() {
                        trigger_refresh = true;
                    }
                    ui.separator();
                    ui.label("Basalt");
                });
            });

        if trigger_add {
            self.add_from_inputs();
        }
        if trigger_discover {
            self.discover_mattmc();
        }
        if trigger_refresh {
            self.refresh_games();
            self.status_message = "Game list refreshed".to_string();
        }

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
                ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
                    ui.heading("Details");
                    ui.separator();

                    if let Some(selected) = self.selected_game() {
                        ui.label(format!("Name: {}", selected.name));
                        ui.label(format!("Runner: {}", selected.runner_kind.as_str()));
                        ui.label("Target:");
                        ui.small(&selected.launch_target);

                        if ui.button("Launch Selected").clicked() {
                            match core::launch_game(&selected.name) {
                                Ok(_) => {
                                    self.status_message =
                                        format!("Launched {}", selected.name);
                                }
                                Err(err) => {
                                    self.status_message = format!("Launch failed: {}", err);
                                }
                            }
                        }
                    } else {
                        ui.small("Select a game tile to view details.");
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.label("Add Game Inputs");
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut self.add_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Script");
                        ui.text_edit_singleline(&mut self.add_script_path);
                    });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.label("Status");
                    if self.status_message.is_empty() {
                        ui.small("Ready");
                    } else {
                        ui.small(&self.status_message);
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
}

impl BasaltApp {
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

    fn refresh_games(&mut self) {
        match core::list_games() {
            Ok(games) => {
                self.games = games;
                if let Some(index) = self.selected_index {
                    if index >= self.games.len() {
                        self.selected_index = None;
                    }
                }
            }
            Err(err) => {
                self.games.clear();
                self.selected_index = None;
                self.status_message = format!("Failed to load games: {}", err);
            }
        }
    }

    fn discover_mattmc(&mut self) {
        match core::discover_games() {
            Ok(report) => {
                self.refresh_games();

                let mattmc_message = match report.mattmc {
                    Some(DiscoverResult::Added) => "MattMC added".to_string(),
                    Some(DiscoverResult::AlreadyExists) => "MattMC already exists".to_string(),
                    Some(DiscoverResult::NotFound) => "MattMC not found".to_string(),
                    None => "MattMC skipped".to_string(),
                };

                let steam_message = match report.steam {
                    Some(steam) => format!(
                        "Steam: found {}, added {}, existing {}",
                        steam.found, steam.added, steam.already_exists
                    ),
                    None => "Steam skipped".to_string(),
                };

                self.status_message = format!("Discover complete | {} | {}", mattmc_message, steam_message);
            }
            Err(err) => {
                self.status_message = format!("Discover failed: {}", err);
            }
        }
    }

    fn add_from_inputs(&mut self) {
        let name = self.add_name.trim().to_string();
        let script_path = self.add_script_path.trim().to_string();

        if name.is_empty() || script_path.is_empty() {
            self.status_message = "Add requires both Name and Script path".to_string();
            return;
        }

        match core::add_game(&name, &script_path) {
            Ok(_) => {
                self.refresh_games();
                self.status_message = format!("Added {}", name);
            }
            Err(err) => {
                self.status_message = format!("Add failed: {}", err);
            }
        }
    }

    fn selected_game(&self) -> Option<&GameEntry> {
        self.selected_index.and_then(|index| self.games.get(index))
    }

    fn render_game_grid(&mut self, ui: &mut egui::Ui, border_stroke: Stroke) {
        const TILE_WIDTH: f32 = 150.0;
        const TILE_HEIGHT: f32 = 150.0;
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

                        let is_selected = self.selected_index == Some(index);
                        if self.render_tile(
                            ui,
                            border_stroke,
                            TILE_WIDTH,
                            TILE_HEIGHT,
                            &self.games[index],
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
        &self,
        ui: &mut egui::Ui,
        border_stroke: Stroke,
        tile_width: f32,
        tile_height: f32,
        game: &GameEntry,
        selected: bool,
    ) -> bool {
        const TEXT_STRIP_HEIGHT: f32 = 34.0;

        let (tile_rect, response) = ui.allocate_exact_size(vec2(tile_width, tile_height), Sense::click());

        let tile_stroke = if selected {
            Stroke::new(2.0, Color32::WHITE)
        } else {
            border_stroke
        };
        ui.painter().rect_stroke(tile_rect, 0.0, tile_stroke, StrokeKind::Inside);

        let icon_rect = egui::Rect::from_min_max(
            tile_rect.min,
            egui::pos2(tile_rect.max.x, tile_rect.max.y - TEXT_STRIP_HEIGHT),
        );
        ui.painter()
            .rect_stroke(icon_rect, 0.0, border_stroke, StrokeKind::Inside);

        let text_rect = egui::Rect::from_min_max(
            egui::pos2(tile_rect.min.x, tile_rect.max.y - TEXT_STRIP_HEIGHT),
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
