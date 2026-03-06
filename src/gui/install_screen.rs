use eframe::egui::{
    self, vec2, CentralPanel, Color32, Frame, Layout, Margin, RichText, ScrollArea, Sense,
    SidePanel,
};

use crate::core;

use super::app::BasaltApp;
use super::game_tile::paint_game_tile;
use super::search;

#[derive(Clone, Copy)]
enum InstallTileKind {
    Mattmc,
    EmulatorCore(&'static str),
}

#[derive(Clone, Copy)]
struct InstallTile {
    key: &'static str,
    title: &'static str,
    description: &'static str,
    kind: InstallTileKind,
}

impl BasaltApp {
    pub(super) fn render_install_screen(
        &mut self,
        ctx: &egui::Context,
        main_region_gray: Color32,
        right_region_gray: Color32,
        right_panel_width: f32,
    ) {
        let filtered_tiles = self.filtered_install_tiles();
        if let Some(selected_key) = self.selected_install_tile_key.as_ref() {
            let selected_visible = filtered_tiles
                .iter()
                .any(|tile| tile.key == selected_key.as_str());
            if !selected_visible {
                self.selected_install_tile_key = None;
            }
        }

        let selected_tile = self
            .selected_install_tile_key
            .as_ref()
            .and_then(|selected_key| {
                filtered_tiles
                    .iter()
                    .find(|tile| tile.key == selected_key.as_str())
            })
            .copied();

        SidePanel::right("install_right_panel")
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

                    if let Some(tile) = selected_tile {
                        ui.label(RichText::new(tile.title).size(body_text_size));
                        ui.label(RichText::new(tile.description).size(secondary_text_size));
                        ui.add_space(8.0);

                        match tile.kind {
                            InstallTileKind::Mattmc => {
                                if ui
                                    .button(RichText::new("Install MattMC").size(body_text_size))
                                    .clicked()
                                {
                                    self.install_mattmc_from_gui();
                                }
                            }
                            InstallTileKind::EmulatorCore(system) => {
                                let status = match core::is_emulation_core_installed_for_system(system)
                                {
                                    Ok(true) => "Installed",
                                    Ok(false) => "Not installed",
                                    Err(_) => "Unknown",
                                };
                                let supports_save_sync =
                                    core::is_emulation_save_sync_supported_for_system(system);
                                ui.label(
                                    RichText::new(format!("Core status: {}", status))
                                        .size(secondary_text_size),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "ROMs: ~/Games/Emulators/roms/{}",
                                        system
                                    ))
                                    .size(secondary_text_size),
                                );
                                if supports_save_sync {
                                    ui.label(
                                        RichText::new(format!(
                                            "Saves: ~/Games/Emulators/saves/{}",
                                            system
                                        ))
                                        .size(secondary_text_size),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("Saves: not supported")
                                            .size(secondary_text_size),
                                    );
                                }

                                if ui
                                    .button(
                                        RichText::new(format!(
                                            "Install {} Core",
                                            system.to_uppercase()
                                        ))
                                        .size(body_text_size),
                                    )
                                    .clicked()
                                {
                                    self.install_emulator_core_from_gui(system);
                                }

                                if ui
                                    .button(RichText::new("Sync Roms Up").size(body_text_size))
                                    .clicked()
                                {
                                    self.sync_emulator_roms_up_from_gui(system);
                                }

                                if ui
                                    .button(RichText::new("Sync Roms Down").size(body_text_size))
                                    .clicked()
                                {
                                    self.sync_emulator_roms_down_from_gui(system);
                                }

                                if supports_save_sync {
                                    if ui
                                        .button(RichText::new("Sync Saves Up").size(body_text_size))
                                        .clicked()
                                    {
                                        self.sync_emulator_saves_up_from_gui(system);
                                    }

                                    if ui
                                        .button(RichText::new("Sync Saves Down").size(body_text_size))
                                        .clicked()
                                    {
                                        self.sync_emulator_saves_down_from_gui(system);
                                    }
                                } else {
                                    ui.label(
                                        RichText::new("Save sync: not supported for this system")
                                            .size(secondary_text_size),
                                    );
                                }
                            }
                        }

                        if !self.install_status_message.is_empty() {
                            ui.add_space(12.0);
                            ui.separator();
                            ui.label(RichText::new("Status").size(body_text_size));
                            ui.label(
                                RichText::new(&self.install_status_message)
                                    .size(secondary_text_size),
                            );
                        }
                    } else {
                        ui.label(
                            RichText::new("Select an install tile to view details.")
                                .size(secondary_text_size),
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
                self.render_install_tile_grid(ui, &filtered_tiles);
            });
    }

    fn all_install_tiles(&self) -> Vec<InstallTile> {
        let mut tiles = vec![
            InstallTile {
                key: "mattmc",
                title: "MattMC",
                description: "Install or update MattMC into ~/Games/MattMC.",
                kind: InstallTileKind::Mattmc,
            },
            InstallTile {
                key: "core-gba",
                title: "GBA Core",
                description: "RetroArch mGBA core for GBA ROMs.",
                kind: InstallTileKind::EmulatorCore("gba"),
            },
            InstallTile {
                key: "core-nes",
                title: "NES Core",
                description: "RetroArch Nestopia core for NES ROMs.",
                kind: InstallTileKind::EmulatorCore("nes"),
            },
            InstallTile {
                key: "core-snes",
                title: "SNES Core",
                description: "RetroArch Snes9x core for SNES ROMs.",
                kind: InstallTileKind::EmulatorCore("snes"),
            },
            InstallTile {
                key: "core-atari2600",
                title: "Atari 2600 Core",
                description: "RetroArch Stella core for Atari 2600 ROMs.",
                kind: InstallTileKind::EmulatorCore("atari2600"),
            },
            InstallTile {
                key: "core-nds",
                title: "NDS Core",
                description: "RetroArch melonDS core for Nintendo DS ROMs.",
                kind: InstallTileKind::EmulatorCore("nds"),
            },
            InstallTile {
                key: "core-3ds",
                title: "3DS Core",
                description: "RetroArch Citra core for Nintendo 3DS ROMs.",
                kind: InstallTileKind::EmulatorCore("3ds"),
            },
        ];

        tiles.sort_by(|left, right| left.title.cmp(right.title));
        tiles
    }

    fn filtered_install_tiles(&self) -> Vec<InstallTile> {
        self.all_install_tiles()
            .into_iter()
            .filter(|tile| {
                search::matches_query(tile.title, &self.install_search_query)
                    || search::matches_query(tile.description, &self.install_search_query)
            })
            .collect()
    }

    fn render_install_tile_grid(&mut self, ui: &mut egui::Ui, tiles: &[InstallTile]) {
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

            if tiles.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(WALL_PADDING);
                    if self.install_search_query.trim().is_empty() {
                        ui.label("No installable items available.");
                    } else {
                        ui.label("No install items match your search.");
                    }
                    ui.add_space(WALL_PADDING + SCROLLBAR_GUTTER);
                });
                ui.add_space(WALL_PADDING);
                return;
            }

            let categorized_tiles = self.categorized_install_tiles(tiles);

            for (category_position, (category_name, category_tiles))
                in categorized_tiles.iter().enumerate()
            {
                let header_text = format!("{} ({})", category_name, category_tiles.len());
                let collapsing = egui::CollapsingHeader::new(header_text)
                    .id_salt(format!("install_category_{}", category_name))
                    .default_open(true);

                collapsing.show(ui, |ui| {
                    ui.add_space(8.0);
                    self.render_install_tile_rows(
                        ui,
                        category_tiles,
                        columns,
                        TILE_WIDTH,
                        TILE_HEIGHT,
                        TILE_SPACING,
                        WALL_PADDING,
                        SCROLLBAR_GUTTER,
                    );
                });

                if category_position + 1 < categorized_tiles.len() {
                    ui.add_space(12.0);
                }
            }

            ui.add_space(WALL_PADDING);
        });
    }

    fn categorized_install_tiles(&self, tiles: &[InstallTile]) -> Vec<(&'static str, Vec<InstallTile>)> {
        let mut my_games_tiles = Vec::new();
        let mut emulator_tiles = Vec::new();

        for tile in tiles {
            match tile.kind {
                InstallTileKind::Mattmc => my_games_tiles.push(*tile),
                InstallTileKind::EmulatorCore(_) => emulator_tiles.push(*tile),
            }
        }

        let mut categories = Vec::new();
        if !my_games_tiles.is_empty() {
            categories.push(("MyGames", my_games_tiles));
        }

        if !emulator_tiles.is_empty() {
            categories.push(("Emulators", emulator_tiles));
        }

        categories
    }

    fn render_install_tile_rows(
        &mut self,
        ui: &mut egui::Ui,
        tiles: &[InstallTile],
        columns: usize,
        tile_width: f32,
        tile_height: f32,
        tile_spacing: f32,
        wall_padding: f32,
        scrollbar_gutter: f32,
    ) {
        let mut visible_index = 0usize;
        while visible_index < tiles.len() {
            ui.horizontal(|ui| {
                ui.add_space(wall_padding);

                for col in 0..columns {
                    if visible_index >= tiles.len() {
                        break;
                    }

                    let tile = tiles[visible_index];
                    if self.render_install_tile(ui, tile_width, tile_height, tile) {
                        self.selected_install_tile_key = Some(tile.key.to_string());
                    }

                    if col + 1 < columns && visible_index + 1 < tiles.len() {
                        ui.add_space(tile_spacing);
                    }

                    visible_index += 1;
                }

                ui.add_space(wall_padding + scrollbar_gutter);
            });

            if visible_index < tiles.len() {
                ui.add_space(tile_spacing);
            }
        }
    }

    fn render_install_tile(
        &mut self,
        ui: &mut egui::Ui,
        tile_width: f32,
        tile_height: f32,
        tile: InstallTile,
    ) -> bool {
        let text_strip_height = (tile_height - tile_width).max(24.0);
        let (tile_rect, response) = ui.allocate_exact_size(vec2(tile_width, tile_height), Sense::click());

        let selected = self
            .selected_install_tile_key
            .as_ref()
            .map(|selected_key| selected_key == tile.key)
            .unwrap_or(false);

        let artwork = match tile.kind {
            InstallTileKind::Mattmc => self.artwork_store.mattmc_artwork(ui.ctx()),
            InstallTileKind::EmulatorCore(_) => None,
        };

        paint_game_tile(
            ui,
            tile_rect,
            text_strip_height,
            tile.title,
            18.0,
            artwork.as_ref(),
            selected,
        );

        response.clicked()
    }
}
