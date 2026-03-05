use eframe::egui::{
    self, vec2, Button, Color32, Frame, Layout, Margin, RichText, Stroke, TopBottomPanel,
};

use super::app::BasaltApp;
use super::search;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TopBarTab {
    Library,
    Install,
}

#[derive(Clone)]
pub(super) enum PlaylistSelection {
    AllGames,
    Named(String),
}

pub(super) struct TopBarActions {
    pub(super) switch_to_tab: Option<TopBarTab>,
    pub(super) select_playlist: Option<PlaylistSelection>,
    pub(super) trigger_add: bool,
    pub(super) trigger_discover: bool,
    pub(super) trigger_refresh: bool,
}

impl TopBarActions {
    fn new() -> Self {
        Self {
            switch_to_tab: None,
            select_playlist: None,
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
        let tab_button_height = 40.0;
        let playlist_button_gap = 2.0;
        let playlist_row_height = 30.0;
        let top_row_height = tab_button_height + playlist_button_gap;

        TopBottomPanel::top("top_bar")
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(10))
                    .stroke(white_line),
            )
            .exact_height(top_row_height + playlist_row_height)
            .show(ctx, |ui| {
                let panel_rect = ui.max_rect();
                let top_row_rect = egui::Rect::from_min_max(
                    panel_rect.min,
                    egui::pos2(panel_rect.max.x, panel_rect.min.y + top_row_height),
                );

                let horizontal_gap = 10.0;
                let mut action_region_width = 290.0;
                let action_region_min_width = 170.0;

                let mut search_region_width = (top_row_rect.width() * 0.30).clamp(180.0, 360.0);
                let min_search_region_width = 120.0;
                let min_center_region_width = 120.0;

                let available_inner_width = (top_row_rect.width() - (horizontal_gap * 2.0)).max(0.0);
                let mut center_region_width =
                    available_inner_width - action_region_width - search_region_width;

                if center_region_width < min_center_region_width {
                    let mut remaining_needed = min_center_region_width - center_region_width;

                    let search_reducible = (search_region_width - min_search_region_width).max(0.0);
                    let shrink_search = remaining_needed.min(search_reducible);
                    search_region_width -= shrink_search;
                    remaining_needed -= shrink_search;

                    let action_reducible =
                        (action_region_width - action_region_min_width).max(0.0);
                    let shrink_action = remaining_needed.min(action_reducible);
                    action_region_width -= shrink_action;

                    center_region_width =
                        available_inner_width - action_region_width - search_region_width;
                }

                center_region_width = center_region_width.max(60.0);

                let action_rect = egui::Rect::from_min_max(
                    top_row_rect.min,
                    egui::pos2(top_row_rect.min.x + action_region_width, top_row_rect.max.y),
                );
                let center_rect = egui::Rect::from_min_max(
                    egui::pos2(action_rect.max.x + horizontal_gap, top_row_rect.min.y),
                    egui::pos2(
                        action_rect.max.x + horizontal_gap + center_region_width,
                        top_row_rect.max.y,
                    ),
                );
                let search_rect = egui::Rect::from_min_max(
                    egui::pos2(center_rect.max.x + horizontal_gap, top_row_rect.min.y),
                    top_row_rect.max,
                );

                let mut action_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(action_rect)
                        .layout(Layout::left_to_right(egui::Align::Min)),
                );

                if self.active_tab == TopBarTab::Library {
                    if action_ui.button("Add").clicked() {
                        actions.trigger_add = true;
                    }
                    if action_ui.button("Discover").clicked() {
                        actions.trigger_discover = true;
                    }
                    if action_ui.button("Refresh").clicked() {
                        actions.trigger_refresh = true;
                    }
                }

                let tab_spacing = ui.spacing().item_spacing.x;
                let tab_button_width =
                    (((center_rect.width() - tab_spacing).max(140.0)) / 2.0).min(130.0);
                let tabs_total_width = (tab_button_width * 2.0) + tab_spacing;
                let tabs_width = tabs_total_width.min(center_rect.width());
                let tabs_left = (center_rect.center().x - (tabs_width / 2.0))
                    .clamp(center_rect.min.x, center_rect.max.x - tabs_width);
                let tabs_rect = egui::Rect::from_min_size(
                    egui::pos2(tabs_left, center_rect.min.y),
                    vec2(tabs_width, tab_button_height),
                );

                let mut tabs_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(tabs_rect)
                        .layout(Layout::left_to_right(egui::Align::Min)),
                );

                let library_button = Button::new(RichText::new("Library").size(18.0))
                    .min_size(vec2(tab_button_width, tab_button_height))
                    .fill(if self.active_tab == TopBarTab::Library {
                        Color32::from_rgb(86, 98, 116)
                    } else {
                        Color32::from_rgb(63, 73, 88)
                    });

                if tabs_ui.add(library_button).clicked() {
                    actions.switch_to_tab = Some(TopBarTab::Library);
                }

                let install_button = Button::new(RichText::new("Install").size(18.0))
                    .min_size(vec2(tab_button_width, tab_button_height))
                    .fill(if self.active_tab == TopBarTab::Install {
                        Color32::from_rgb(86, 98, 116)
                    } else {
                        Color32::from_rgb(63, 73, 88)
                    });

                if tabs_ui.add(install_button).clicked() {
                    actions.switch_to_tab = Some(TopBarTab::Install);
                }

                let mut search_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(search_rect.shrink2(vec2(2.0, 2.0)))
                        .layout(Layout::left_to_right(egui::Align::Min)),
                );

                let (active_query, hint_text) = match self.active_tab {
                    TopBarTab::Library => (
                        &mut self.library_search_query,
                        "Search library (name/runner/target)",
                    ),
                    TopBarTab::Install => (&mut self.install_search_query, "Search installs"),
                };

                search::render_search_field(&mut search_ui, active_query, hint_text, 14.0);

                if self.active_tab == TopBarTab::Library {
                    let playlist_row_top = top_row_rect.min.y + tab_button_height + playlist_button_gap;
                    let playlist_rect = egui::Rect::from_min_max(
                        egui::pos2(panel_rect.min.x, playlist_row_top),
                        panel_rect.max,
                    );
                    let mut playlist_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(playlist_rect.shrink2(vec2(2.0, 0.0)))
                            .layout(Layout::left_to_right(egui::Align::Min)),
                    );

                    playlist_ui.horizontal_wrapped(|ui| {
                        let all_games_selected = self.selected_playlist.is_none();
                        let all_games_button = Button::new(RichText::new("All Games").size(14.0))
                            .fill(if all_games_selected {
                                Color32::from_rgb(86, 98, 116)
                            } else {
                                Color32::from_rgb(63, 73, 88)
                            });

                        if ui.add(all_games_button).clicked() {
                            actions.select_playlist = Some(PlaylistSelection::AllGames);
                        }

                        for playlist in &self.playlists {
                            let is_selected = self
                                .selected_playlist
                                .as_ref()
                                .map(|selected| selected == &playlist.name)
                                .unwrap_or(false);

                            let playlist_button = Button::new(RichText::new(&playlist.name).size(14.0))
                                .fill(if is_selected {
                                    Color32::from_rgb(86, 98, 116)
                                } else {
                                    Color32::from_rgb(63, 73, 88)
                                });

                            if ui.add(playlist_button).clicked() {
                                actions.select_playlist =
                                    Some(PlaylistSelection::Named(playlist.name.clone()));
                            }
                        }
                    });
                }
            });

        actions
    }
}
