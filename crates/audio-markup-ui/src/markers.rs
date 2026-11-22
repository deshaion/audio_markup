use audio_markup_engine::{Command, MarkerId, Result, format_marker_time, parse_marker_time};
use eframe::egui::{self, RichText};
use egui_extras::{Column, TableBuilder};

use crate::{App, shortcuts::focused_text_edit, theme};

pub struct PositionDraft {
    text: String,
    pub error: Option<String>,
}

impl App {
    pub(crate) fn move_marker(&mut self, down: bool) {
        let markers = self.session.markers();
        let next = match markers
            .iter()
            .position(|marker| Some(marker.id) == self.state.selected)
        {
            Some(index) if down => Some(index + 1),
            Some(index) => index.checked_sub(1),
            None if down => Some(0),
            None => markers.len().checked_sub(1),
        };
        if let Some(index) = next {
            self.select_marker(index);
        }
    }

    pub(crate) fn add_marker(&mut self, context: &egui::Context) -> Result<()> {
        self.execute(Command::AddMarker)?;
        leave_text_edit(context);
        self.select_marker(self.session.markers().len() - 1);
        Ok(())
    }

    pub(crate) fn delete_marker(&mut self, context: &egui::Context, id: MarkerId) -> Result<()> {
        let index = self
            .session
            .markers()
            .iter()
            .position(|marker| marker.id == id);
        self.execute(Command::DeleteMarker(id))?;
        self.state.positions.remove(&id);
        if self.state.selected == Some(id) {
            leave_text_edit(context);
            self.state.selected = None;
            self.state.scroll_to_marker = None;
            if let (Some(index), Some(last)) = (index, self.session.markers().len().checked_sub(1))
            {
                self.select_marker(index.min(last));
            }
        }
        context.request_repaint();
        Ok(())
    }

    fn select_marker(&mut self, index: usize) {
        if let Some(marker) = self.session.markers().get(index) {
            self.state.selected = Some(marker.id);
            self.state.scroll_to_marker = Some(index);
        }
    }

    pub(crate) fn marker_table(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Markers");
            ui.separator();
            if ui.button("Add marker").on_hover_text("Alt+N").clicked() {
                let result = self.add_marker(ui.ctx());
                self.report(result);
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
        let mut table = TableBuilder::new(ui)
            .id_salt("markers")
            .striped(true)
            .resizable(true)
            .auto_shrink([false, false])
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(105.0).range(80.0..=180.0))
            .column(Column::initial(235.0).at_least(105.0))
            .column(Column::remainder().at_least(105.0))
            .column(Column::exact(72.0));
        if let Some(index) = self.state.scroll_to_marker.take() {
            table = table.scroll_to_row(index, None).animate_scrolling(false);
        }
        table
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
                            commands.push(Command::DeleteMarker(id));
                        }
                    });
                })
            });
        for command in commands {
            match command {
                Command::DeleteMarker(id) => {
                    let result = self.delete_marker(ui.ctx(), id);
                    self.report(result);
                }
                command => self.dispatch(command),
            }
        }
    }
}

fn leave_text_edit(context: &egui::Context) {
    if let Some(id) = focused_text_edit(context) {
        context.memory_mut(|memory| memory.surrender_focus(id));
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
