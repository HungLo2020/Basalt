use eframe::egui::{
    self, vec2, Button, Color32, Frame, Layout, Margin, RichText, Stroke, TopBottomPanel,
};

use super::app::BasaltApp;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TopBarTab {
    Library,
    Install,
}

pub(super) struct TopBarActions {
    pub(super) switch_to_tab: Option<TopBarTab>,
    pub(super) trigger_add: bool,
    pub(super) trigger_discover: bool,
    pub(super) trigger_refresh: bool,
}

impl TopBarActions {
    fn new() -> Self {
        Self {
            switch_to_tab: None,
            trigger_add: false,
            trigger_discover: false,
            trigger_refresh: false,
        }
    }
}

impl BasaltApp {
    pub(super) fn render_top_bar(
        &mut self,
        ctx: &eframe::egui::Context,
        region_gray: Color32,
        white_line: Stroke,
    ) -> TopBarActions {
        let mut actions = TopBarActions::new();

        TopBottomPanel::top("top_bar")
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(10))
                    .stroke(white_line),
            )
            .exact_height(68.0)
            .show(ctx, |ui| {
                let panel_rect = ui.max_rect();

                ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                    if self.active_tab == TopBarTab::Library {
                        if ui.button("Add").clicked() {
                            actions.trigger_add = true;
                        }
                        if ui.button("Discover").clicked() {
                            actions.trigger_discover = true;
                        }
                        if ui.button("Refresh").clicked() {
                            actions.trigger_refresh = true;
                        }
                    }
                });

                let tab_button_size = vec2(130.0, 40.0);
                let tabs_total_width = (tab_button_size.x * 2.0) + ui.spacing().item_spacing.x;
                let tabs_rect = egui::Rect::from_center_size(
                    panel_rect.center(),
                    vec2(tabs_total_width, tab_button_size.y),
                );

                let mut tabs_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(tabs_rect)
                        .layout(Layout::left_to_right(egui::Align::Center)),
                );

                let library_button = Button::new(RichText::new("Library").size(18.0))
                    .min_size(tab_button_size)
                    .fill(if self.active_tab == TopBarTab::Library {
                        Color32::from_gray(95)
                    } else {
                        Color32::from_gray(70)
                    });

                if tabs_ui.add(library_button).clicked() {
                    actions.switch_to_tab = Some(TopBarTab::Library);
                }

                let install_button = Button::new(RichText::new("Install").size(18.0))
                    .min_size(tab_button_size)
                    .fill(if self.active_tab == TopBarTab::Install {
                        Color32::from_gray(95)
                    } else {
                        Color32::from_gray(70)
                    });

                if tabs_ui.add(install_button).clicked() {
                    actions.switch_to_tab = Some(TopBarTab::Install);
                }
            });

        actions
    }
}
