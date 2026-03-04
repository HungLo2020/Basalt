use eframe::egui::{
    self, vec2, CentralPanel, Color32, Frame, Layout, Margin, ScrollArea, Sense,
    SidePanel, Stroke, StrokeKind, TopBottomPanel,
};

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Basalt",
        options,
        Box::new(|_cc| Ok(Box::new(BasaltApp::default()))),
    )
    .map_err(|err| format!("Failed to launch GUI: {}", err))
}

#[derive(Default)]
struct BasaltApp;

impl eframe::App for BasaltApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let region_gray = Color32::from_gray(55);
        let white_line = Stroke::new(1.0, Color32::WHITE);
        let window_width = ctx.screen_rect().width();
        let right_panel_width = window_width / 4.0;

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
                    ui.label("Top Bar (future options)");
                });
            });

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
                    ui.label("Right Panel (reserved)");
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
                self.render_placeholder_grid(ui, white_line);
            });
    }
}

impl BasaltApp {
    fn render_placeholder_grid(&mut self, ui: &mut egui::Ui, border_stroke: Stroke) {
        const TILE_WIDTH: f32 = 150.0;
        const TILE_HEIGHT: f32 = 150.0;
        const TILE_SPACING: f32 = 24.0;
        const WALL_PADDING: f32 = 24.0;
        const PLACEHOLDER_COUNT: usize = 24;

        let usable_width = (ui.available_width() - (WALL_PADDING * 2.0)).max(TILE_WIDTH);
        let columns = ((usable_width + TILE_SPACING) / (TILE_WIDTH + TILE_SPACING)).floor() as usize;
        let columns = columns.max(1);

        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(WALL_PADDING);

            let mut index = 0usize;
            while index < PLACEHOLDER_COUNT {
                ui.horizontal(|ui| {
                    ui.add_space(WALL_PADDING);

                    for col in 0..columns {
                        if index >= PLACEHOLDER_COUNT {
                            break;
                        }

                        self.render_tile(ui, border_stroke, TILE_WIDTH, TILE_HEIGHT, index + 1);

                        if col + 1 < columns && index + 1 < PLACEHOLDER_COUNT {
                            ui.add_space(TILE_SPACING);
                        }

                        index += 1;
                    }
                });

                if index < PLACEHOLDER_COUNT {
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
        number: usize,
    ) {
        const TEXT_STRIP_HEIGHT: f32 = 34.0;

        let (tile_rect, _) = ui.allocate_exact_size(vec2(tile_width, tile_height), Sense::hover());
        ui.painter()
            .rect_stroke(tile_rect, 0.0, border_stroke, StrokeKind::Inside);

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

        let mut tile_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(text_rect)
                .layout(Layout::centered_and_justified(egui::Direction::TopDown)),
        );

        tile_ui.small(format!("Game {}", number));
    }
}
