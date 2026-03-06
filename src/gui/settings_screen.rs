use eframe::egui::{self, CentralPanel, Color32, Frame, Margin, RichText, SidePanel};

use super::app::BasaltApp;

impl BasaltApp {
    pub(super) fn render_settings_screen(
        &mut self,
        ctx: &egui::Context,
        main_region_gray: Color32,
        right_region_gray: Color32,
        right_panel_width: f32,
    ) {
        SidePanel::right("settings_right_panel")
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
                ui.heading("Settings");
                ui.separator();

                ui.label(RichText::new("Remote Sync Defaults"));
                ui.label(
                    RichText::new("Used globally by Sync Roms Up/Down and Sync Saves Up/Down.")
                        .size(14.0),
                );

                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "ROMs root:\n{}",
                        self.settings_remote_roms_root_input
                    ))
                    .size(14.0),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "Saves root:\n{}",
                        self.settings_remote_saves_root_input
                    ))
                    .size(14.0),
                );

                ui.add_space(10.0);
                ui.separator();
                ui.label(RichText::new("Status"));
                if self.settings_status_message.trim().is_empty() {
                    ui.label(RichText::new("Ready").size(14.0));
                } else {
                    ui.label(RichText::new(&self.settings_status_message).size(14.0));
                }
            });

        CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(main_region_gray)
                    .inner_margin(Margin::same(12))
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();

                ui.label("Emulation Remote Paths");
                ui.label("These are global remote defaults only (not per game/core).");
                ui.add_space(8.0);

                ui.label("Default remote ROMs root path");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_remote_roms_root_input)
                        .desired_width(f32::INFINITY),
                );

                ui.add_space(8.0);
                ui.label("Default remote Saves root path");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_remote_saves_root_input)
                        .desired_width(f32::INFINITY),
                );

                ui.add_space(12.0);
                if ui.button("Save Remote Paths").clicked() {
                    self.save_emulation_remote_paths_from_gui();
                }
            });
    }
}
