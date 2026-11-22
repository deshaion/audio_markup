use audio_markup_engine::{Command, MarkerId, format_marker_time, parse_marker_time};
use eframe::egui::{self, RichText};
use egui_extras::{Column, TableBuilder};

use crate::{App, theme};

pub struct PositionDraft {
    text: String,
    pub error: Option<String>,
}

impl App {
    pub(crate) fn marker_table(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Markers");
            ui.separator();
            if ui.button("Add marker").clicked() {
                self.dispatch(Command::AddMarker);
            }
            let ready = self.state.selected.is_some()
                && self.session.track().is_some()
                && !self.session.opening();
            if ui
                .add_enabled(ready, egui::Button::new("Set from playhead"))
                .on_hover_text("Alt+X")
                .clicked()
            {
                let result = self.capture();
                self.report(result);
            }
            if ui
                .add_enabled(ready, egui::Button::new("Seek to marker"))
                .on_hover_text("Alt+S")
                .clicked()
            {
                let result = self.seek_to_marker();
                self.report(result);
            }
        });
        ui.add_space(4.0);

        let markers = self.session.markers();
        let mut commands = Vec::new();
        TableBuilder::new(ui)
            .id_salt("markers")
            .striped(true)
            .resizable(true)
            .auto_shrink([false, false])
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(105.0).range(80.0..=180.0))
            .column(Column::initial(235.0).at_least(105.0))
            .column(Column::remainder().at_least(105.0))
            .column(Column::exact(72.0))
            .header(30.0, |mut row| {
                for heading in ["Pos", "Artist", "Title", ""] {
                    row.col(|ui| {
                        ui.label(RichText::new(heading).strong().color(theme::MUTED));
                    });
                }
            })
            .body(|body| {
                body.rows(35.0, markers.len(), |mut row| {
                    let marker = &markers[row.index()];
                    let id = marker.id;
                    row.set_selected(self.state.selected == Some(id));
                    row.col(|ui| {
                        let drafts = &mut self.state.positions;
                        let existing = drafts.get(&id);
                        let mut text = existing.map_or_else(
                            || {
                                marker
                                    .data
                                    .position
                                    .map(format_marker_time)
                                    .unwrap_or_default()
                            },
                            |draft| draft.text.clone(),
                        );
                        let invalid = existing.and_then(|draft| draft.error.as_deref());
                        let response = field(ui, id, "Pos", &mut text, invalid.is_some())
                            .on_hover_text(
                                invalid.unwrap_or("Position in MM:SS, MM:SS.mmm, or MM:SS:FF"),
                            );
                        if response.changed() {
                            let parsed = if text.trim().is_empty() {
                                Ok(None)
                            } else {
                                parse_marker_time(&text).map(Some)
                            };
                            let error = match parsed {
                                Ok(position) => {
                                    commands.push(Command::SetPosition(id, position));
                                    None
                                }
                                Err(error) => Some(error),
                            };
                            drafts.insert(id, PositionDraft { text, error });
                        }
                        if response.lost_focus()
                            && drafts.get(&id).is_some_and(|draft| draft.error.is_none())
                        {
                            drafts.remove(&id);
                        }
                        if response.clicked() || response.has_focus() {
                            self.state.selected = Some(id);
                        }
                    });
                    row.col(|ui| {
                        let mut text = marker.data.artist.clone();
                        let pasted = ui.input(|input| {
                            input
                                .events
                                .iter()
                                .any(|event| matches!(event, egui::Event::Paste(_)))
                        });
                        let response = field(ui, id, "Artist", &mut text, false)
                            .on_hover_text("Paste Artist - Title to fill both fields.");
                        if response.changed() {
                            commands.push(Command::SetArtist(id, text, pasted));
                        }
                        if response.clicked() || response.has_focus() {
                            self.state.selected = Some(id);
                        }
                    });
                    row.col(|ui| {
                        let mut text = marker.data.title.clone();
                        let response = field(ui, id, "Title", &mut text, false);
                        if response.changed() {
                            commands.push(Command::SetTitle(id, text));
                        }
                        if response.clicked() || response.has_focus() {
                            self.state.selected = Some(id);
                        }
                    });
                    row.col(|ui| {
                        if ui
                            .small_button("Delete")
                            .on_hover_text("Delete this marker")
                            .clicked()
                        {
                            self.state.positions.remove(&id);
                            if self.state.selected == Some(id) {
                                self.state.selected = None;
                            }
                            commands.push(Command::DeleteMarker(id));
                        }
                    });
                })
            });
        for command in commands {
            self.dispatch(command);
        }
    }
}

fn field(
    ui: &mut egui::Ui,
    id: MarkerId,
    label: &str,
    value: &mut String,
    invalid: bool,
) -> egui::Response {
    let edit = egui::TextEdit::singleline(value)
        .id(egui::Id::unique(("audio-markup-marker", id, label)))
        .desired_width(f32::INFINITY)
        .hint_text(label);
    let response = ui.add(if invalid {
        edit.text_color(theme::ERROR)
    } else {
        edit
    });
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label));
    response
}
