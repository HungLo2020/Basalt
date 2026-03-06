use eframe::egui::{
    self, vec2, CentralPanel, Color32, Frame, Layout, Margin, Sense, SidePanel, RichText,
};

use crate::core;

use super::app::BasaltApp;
use super::game_tile::paint_game_tile;
use super::search;

impl BasaltApp {
    pub(super) fn render_install_screen(
        &mut self,
        ctx: &egui::Context,
        main_region_gray: Color32,
        right_region_gray: Color32,
        right_panel_width: f32,
    ) {
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
                    ui.heading("MattMC Install");
                    ui.separator();
                    ui.label(
                        RichText::new("Install or update MattMC into ~/Games/MattMC.")
                            .size(body_text_size),
                    );

                    if ui
                        .button(RichText::new("Install MattMC").size(body_text_size))
                        .clicked()
                    {
                        self.install_mattmc_from_gui();
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.label(
                        RichText::new(
                            "ROM discovery is handled by Library > Discover. Place ROMs under ~/Games/Emulators/roms/<system>.",
                        )
                        .size(body_text_size),
                    );

                    ui.add_space(12.0);
                    ui.separator();
                    ui.label(RichText::new("Status").size(body_text_size));
                    if self.install_status_message.is_empty() {
                        ui.label(RichText::new("Ready").size(secondary_text_size));
                    } else {
                        ui.label(RichText::new(&self.install_status_message).size(secondary_text_size));
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
                ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                    let show_mattmc_tile = search::matches_query("MattMC", &self.install_search_query);
                    let show_nes_tile = search::matches_query("NES", &self.install_search_query);
                    let show_gba_tile = search::matches_query("GBA", &self.install_search_query);

                    if !show_mattmc_tile && !show_nes_tile && !show_gba_tile {
                        ui.label(RichText::new("No install entries match your search.").size(15.0));
                        return;
                    }

                    const TILE_ART_SIZE: f32 = 180.0;
                    const TEXT_STRIP_HEIGHT: f32 = 42.0;
                    const TILE_WIDTH: f32 = TILE_ART_SIZE;
                    const TILE_HEIGHT: f32 = TILE_ART_SIZE + TEXT_STRIP_HEIGHT;

                    if show_mattmc_tile {
                        let (tile_rect, _) =
                            ui.allocate_exact_size(vec2(TILE_WIDTH, TILE_HEIGHT), Sense::hover());
                        let artwork = self.artwork_store.mattmc_artwork(ui.ctx());
                        paint_game_tile(
                            ui,
                            tile_rect,
                            TEXT_STRIP_HEIGHT,
                            "MattMC",
                            18.0,
                            artwork.as_ref(),
                            false,
                        );
                        ui.add_space(16.0);
                    }

                    ui.horizontal(|ui| {
                        if show_nes_tile {
                            ui.vertical(|ui| {
                                let (tile_rect, _) = ui
                                    .allocate_exact_size(vec2(TILE_WIDTH, TILE_HEIGHT), Sense::hover());
                                paint_game_tile(
                                    ui,
                                    tile_rect,
                                    TEXT_STRIP_HEIGHT,
                                    "NES Core",
                                    18.0,
                                    None,
                                    false,
                                );

                                let nes_status = match core::is_emulation_core_installed_for_system("nes") {
                                    Ok(true) => "Installed",
                                    Ok(false) => "Not installed",
                                    Err(_) => "Unknown",
                                };
                                ui.label(RichText::new(nes_status).size(14.0));

                                if ui
                                    .button(RichText::new("Install NES Core").size(15.0))
                                    .clicked()
                                {
                                    self.install_emulator_core_from_gui("nes");
                                }
                            });
                        }

                        if show_nes_tile && show_gba_tile {
                            ui.add_space(20.0);
                        }

                        if show_gba_tile {
                            ui.vertical(|ui| {
                                let (tile_rect, _) = ui
                                    .allocate_exact_size(vec2(TILE_WIDTH, TILE_HEIGHT), Sense::hover());
                                paint_game_tile(
                                    ui,
                                    tile_rect,
                                    TEXT_STRIP_HEIGHT,
                                    "GBA Core",
                                    18.0,
                                    None,
                                    false,
                                );

                                let gba_status = match core::is_emulation_core_installed_for_system("gba") {
                                    Ok(true) => "Installed",
                                    Ok(false) => "Not installed",
                                    Err(_) => "Unknown",
                                };
                                ui.label(RichText::new(gba_status).size(14.0));

                                if ui
                                    .button(RichText::new("Install GBA Core").size(15.0))
                                    .clicked()
                                {
                                    self.install_emulator_core_from_gui("gba");
                                }
                            });
                        }
                    });
                });
            });
    }
}
