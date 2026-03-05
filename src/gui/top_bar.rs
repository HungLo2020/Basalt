use eframe::egui::{
    self, vec2, Button, Color32, Frame, Layout, Margin, RichText, TopBottomPanel,
};

use super::app::BasaltApp;
use super::search;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TopBarTab {
    Library,
    Install,
    Settings,
}

#[derive(Clone)]
pub(super) enum PlaylistSelection {
    AllGames,
    Named(String),
}

pub(super) struct TopBarActions {
    pub(super) switch_to_tab: Option<TopBarTab>,
    pub(super) select_playlist: Option<PlaylistSelection>,
    pub(super) open_settings: bool,
    pub(super) go_back_from_settings: bool,
    pub(super) trigger_discover: bool,
    pub(super) trigger_refresh: bool,
}

impl TopBarActions {
    fn new() -> Self {
        Self {
            switch_to_tab: None,
            select_playlist: None,
            open_settings: false,
            go_back_from_settings: false,
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
    ) -> TopBarActions {
        let mut actions = TopBarActions::new();
        let tab_button_height = 36.0;
        let playlist_button_gap = 1.0;
        let playlist_row_height = 22.0;
        let top_row_height = tab_button_height + playlist_button_gap;
        let top_bar_height = top_row_height + playlist_row_height + 1.0;

        TopBottomPanel::top("top_bar")
            .frame(
                Frame::new()
                    .fill(region_gray)
                    .inner_margin(Margin::same(10))
                    .stroke(egui::Stroke::NONE),
            )
            .exact_height(top_bar_height)
            .show(ctx, |ui| {
                let panel_rect = ui.max_rect();
                let top_row_rect = egui::Rect::from_min_max(
                    panel_rect.min,
                    egui::pos2(panel_rect.max.x, panel_rect.min.y + top_row_height),
                );
                let in_settings = self.active_tab == TopBarTab::Settings;

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

                if !in_settings && self.active_tab == TopBarTab::Library {
                    if action_ui.button("Discover").clicked() {
                        actions.trigger_discover = true;
                    }
                    if action_ui.button("Refresh").clicked() {
                        actions.trigger_refresh = true;
                    }
                }

                if !in_settings {
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
                        TopBarTab::Install => {
                            (&mut self.install_search_query, "Search installs")
                        }
                        TopBarTab::Settings => unreachable!(),
                    };

                    search::render_search_field(&mut search_ui, active_query, hint_text, 14.0);
                }

                let second_row_rect = egui::Rect::from_min_max(
                    egui::pos2(panel_rect.min.x, top_row_rect.max.y - 4.0),
                    egui::pos2(panel_rect.max.x, panel_rect.max.y - 2.0),
                );
                let second_row_right_rect = egui::Rect::from_min_max(
                    egui::pos2(search_rect.min.x, second_row_rect.min.y),
                    second_row_rect.max,
                );
                let second_row_right_inner = egui::Rect::from_min_max(
                    egui::pos2(second_row_right_rect.min.x + 2.0, second_row_right_rect.min.y - 3.0),
                    egui::pos2(second_row_right_rect.max.x - 2.0, second_row_right_rect.max.y - 5.0),
                );
                let settings_button_height = second_row_right_inner.height().min(24.0).max(20.0);
                let mut second_row_right_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(second_row_right_inner)
                        .layout(Layout::right_to_left(egui::Align::Min)),
                );

                if in_settings {
                    if second_row_right_ui
                        .add_sized([96.0, settings_button_height], Button::new("Back"))
                        .clicked()
                    {
                        actions.go_back_from_settings = true;
                    }
                } else if second_row_right_ui
                    .add_sized([96.0, settings_button_height], Button::new("Settings"))
                    .clicked()
                {
                    actions.open_settings = true;
                }

                if !in_settings && self.active_tab == TopBarTab::Library {
                    let playlist_rect = egui::Rect::from_min_max(
                        second_row_rect.min,
                        egui::pos2(search_rect.min.x - horizontal_gap, second_row_rect.max.y),
                    );
                    let playlist_inner_rect = egui::Rect::from_min_max(
                        egui::pos2(playlist_rect.min.x + 2.0, playlist_rect.min.y - 4.0),
                        egui::pos2(playlist_rect.max.x - 2.0, playlist_rect.max.y - 4.0),
                    );
                    let mut playlist_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(playlist_inner_rect)
                            .layout(Layout::left_to_right(egui::Align::Min)),
                    );

                    playlist_ui.horizontal(|ui| {
                        let selected_playlist_text = self
                            .selected_playlist
                            .as_deref()
                            .unwrap_or("All Games");

                        egui::ComboBox::from_id_salt("playlist-selector")
                            .selected_text(selected_playlist_text)
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                let all_games_selected = self.selected_playlist.is_none();
                                if ui.selectable_label(all_games_selected, "All Games").clicked() {
                                    actions.select_playlist = Some(PlaylistSelection::AllGames);
                                }

                                for playlist in &self.playlists {
                                    let is_selected = self
                                        .selected_playlist
                                        .as_ref()
                                        .map(|selected| selected == &playlist.name)
                                        .unwrap_or(false);

                                    if ui
                                        .selectable_label(is_selected, &playlist.name)
                                        .clicked()
                                    {
                                        actions.select_playlist =
                                            Some(PlaylistSelection::Named(playlist.name.clone()));
                                    }
                                }
                            });
                    });
                }
            });

        actions
    }
}
