use eframe::egui::{
    self, vec2, Align2, CentralPanel, Color32, FontId, Frame, Layout, Margin, ScrollArea, Sense,
    RichText, SidePanel, Stroke, StrokeKind,
};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

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

                    if let Some(selected) = self.selected_game() {
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

        if let Some(texture_handle) = self.get_or_load_steam_tile_texture(ui.ctx(), game) {
            let [texture_width, texture_height] = texture_handle.size();
            let draw_rect = aspect_fit_rect(
                icon_rect,
                texture_width as f32,
                texture_height as f32,
            );

            ui.painter().image(
                texture_handle.id(),
                draw_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }

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

    fn get_or_load_steam_tile_texture(
        &mut self,
        ctx: &egui::Context,
        game: &GameEntry,
    ) -> Option<egui::TextureHandle> {
        if game.runner_kind.as_str() != "steam" {
            return None;
        }

        let appid = extract_steam_appid(&game.launch_target)?;

        if let Some(existing_texture) = self.steam_tile_textures.get(&appid) {
            return Some(existing_texture.clone());
        }

        let Some(artwork_path) = find_cached_steam_portrait_artwork_path(&appid) else {
            if !self.steam_artwork_missing.contains(&appid)
                && self.steam_artwork_requested.insert(appid.clone())
            {
                let _ = self.steam_artwork_download_tx.send(appid);
            }
            return None;
        };

        let image_bytes = match std::fs::read(&artwork_path) {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = std::fs::remove_file(&artwork_path);
                return None;
            }
        };

        let decoded = match image::load_from_memory(&image_bytes) {
            Ok(decoded_image) => decoded_image.to_rgba8(),
            Err(_) => {
                let _ = std::fs::remove_file(&artwork_path);
                return None;
            }
        };

        let width = usize::try_from(decoded.width()).ok()?;
        let height = usize::try_from(decoded.height()).ok()?;
        let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], decoded.as_raw());

        let texture_handle = ctx.load_texture(
            format!("steam-artwork-{}", appid),
            color_image,
            egui::TextureOptions::LINEAR,
        );

        self.steam_tile_textures.insert(appid.clone(), texture_handle);
        self.steam_tile_textures.get(&appid).cloned()
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

pub(super) fn extract_steam_appid(launch_target: &str) -> Option<String> {
    let trimmed = launch_target.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().all(|char_value| char_value.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }

    for prefix in [
        "steam://rungameid/",
        "steam://run/",
        "steam:appid:",
        "steam-appid:",
    ] {
        if let Some(value) = trimmed.strip_prefix(prefix) {
            if value.chars().all(|char_value| char_value.is_ascii_digit()) {
                return Some(value.to_string());
            }
        }
    }

    None
}

pub(super) fn find_cached_steam_portrait_artwork_path(appid: &str) -> Option<PathBuf> {
    let cache_dir = steam_artwork_cache_dir()?;
    let cached_candidates = [
        cache_dir.join(format!("{}_library_600x900_2x.jpg", appid)),
        cache_dir.join(format!("{}_library_600x900.jpg", appid)),
        cache_dir.join(format!("{}_library_600x900.png", appid)),
        cache_dir.join(format!("{}_library_600x900_2x_alt.jpg", appid)),
        cache_dir.join(format!("{}_library_600x900_alt.jpg", appid)),
        cache_dir.join(format!("{}_library_600x900_alt.png", appid)),
    ];

    for candidate in cached_candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

pub(super) fn download_and_cache_steam_portrait_artwork(appid: &str) -> Option<PathBuf> {
    let cache_dir = steam_artwork_cache_dir()?;
    if let Some(existing_cached) = find_cached_steam_portrait_artwork_path(appid) {
        if is_valid_portrait_artwork(&existing_cached) {
            return Some(existing_cached);
        }

        let _ = std::fs::remove_file(existing_cached);
    }

    let urls_and_targets = [
        (
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900_2x.jpg",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900_2x.jpg", appid)),
        ),
        (
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900.jpg", appid)),
        ),
        (
            format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.png",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900.png", appid)),
        ),
        (
            format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900_2x.jpg",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900_2x_alt.jpg", appid)),
        ),
        (
            format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900.jpg",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900_alt.jpg", appid)),
        ),
        (
            format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_600x900.png",
                appid
            ),
            cache_dir.join(format!("{}_library_600x900_alt.png", appid)),
        ),
    ];

    for (url, target_path) in urls_and_targets {
        if target_path.is_file() {
            if is_valid_portrait_artwork(&target_path) {
                return Some(target_path);
            }

            let _ = std::fs::remove_file(&target_path);
        }

        let output = Command::new("curl")
            .args(["-fsSL", "--retry", "2", "--output"])
            .arg(&target_path)
            .arg(&url)
            .output()
            .ok()?;

        if output.status.success() && target_path.is_file() && is_valid_portrait_artwork(&target_path) {
            return Some(target_path);
        }

        let _ = std::fs::remove_file(&target_path);
    }

    None
}

fn steam_artwork_cache_dir() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let cache_dir = PathBuf::from(home)
        .join(".basalt")
        .join("cache")
        .join("steam_artwork");

    if std::fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }

    Some(cache_dir)
}

fn is_valid_portrait_artwork(path: &Path) -> bool {
    let Ok(image_reader) = image::ImageReader::open(path) else {
        return false;
    };

    let Ok(image_data) = image_reader.decode() else {
        return false;
    };

    let width = image_data.width();
    let height = image_data.height();
    if width < 300 || height < 450 {
        return false;
    }

    let aspect_ratio = width as f32 / height as f32;
    (0.60..=0.74).contains(&aspect_ratio)
}
