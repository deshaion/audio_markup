use audio_markup_engine::{Command, MarkerId, Result, format_marker_time, parse_marker_time};
use eframe::egui::{self, RichText};
use egui_extras::{Column, TableBuilder};

use crate::{App, UiState, shortcuts::focused_text_edit, theme};

pub struct PositionDraft {
    text: String,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum MarkerColumn {
    #[default]
    Position,
    Artist,
    Title,
}

impl MarkerColumn {
    const ALL: [Self; 3] = [Self::Position, Self::Artist, Self::Title];

    fn label(self) -> &'static str {
        match self {
            Self::Position => "Pos",
            Self::Artist => "Artist",
            Self::Title => "Title",
        }
    }

    fn field_id(self, id: MarkerId) -> egui::Id {
        egui::Id::unique(("audio-markup-marker", id, self.label()))
    }
}

impl App {
    fn selected_marker_index(&self) -> Option<usize> {
        self.session
            .markers()
            .iter()
            .position(|marker| Some(marker.id) == self.state.selected)
    }

    pub(crate) fn move_marker(&mut self, down: bool) {
        let next = match self.selected_marker_index() {
            Some(index) if down => Some(index + 1),
            Some(index) => index.checked_sub(1),
            None if down => Some(0),
            None => self.session.markers().len().checked_sub(1),
        };
        if let Some(index) = next {
            self.select_marker(index);
        }
    }

    pub(crate) fn move_marker_column(&mut self, right: bool) {
        let Some(index) = self.selected_marker_index() else {
            self.select_marker(0);
            return;
        };
        self.select_marker(index);
        self.state.active_column = match (self.state.active_column, right) {
            (MarkerColumn::Position, true) | (MarkerColumn::Title, false) => MarkerColumn::Artist,
            (MarkerColumn::Artist, true) => MarkerColumn::Title,
            (MarkerColumn::Artist, false) => MarkerColumn::Position,
            (column, _) => column,
        };
    }

    pub(crate) fn edit_marker_cell(&mut self) {
        self.select_marker(self.selected_marker_index().unwrap_or(0));
        if let Some(id) = self.state.selected {
            self.state.pending_cell_focus = Some(self.state.active_column.field_id(id));
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
            self.state.pending_cell_focus = None;
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
            self.state.pending_cell_focus = None;
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
                for heading in MarkerColumn::ALL.map(MarkerColumn::label) {
                    row.col(|ui| {
                        ui.label(RichText::new(heading).strong().color(theme::MUTED));
                    });
                }
                row.col(|_| {});
            })
            .body(|body| {
                body.rows(35.0, markers.len(), |mut row| {
                    let marker = &markers[row.index()];
                    let id = marker.id;
                    row.set_selected(self.state.selected == Some(id));
                    row.col(|ui| {
                        let existing = self.state.positions.get(&id);
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
                        let invalid = existing.is_some_and(|draft| draft.error.is_some());
                        let response = field(
                            ui,
                            &mut self.state,
                            id,
                            MarkerColumn::Position,
                            &mut text,
                            invalid,
                        );
                        let drafts = &mut self.state.positions;
                        let response = response.on_hover_text(
                            drafts
                                .get(&id)
                                .and_then(|draft| draft.error.as_deref())
                                .unwrap_or("Position in MM:SS, MM:SS.mmm, or MM:SS:FF"),
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
                    });
                    row.col(|ui| {
                        let mut text = marker.data.artist.clone();
                        let pasted = ui.input(|input| {
                            input
                                .events
                                .iter()
                                .any(|event| matches!(event, egui::Event::Paste(_)))
                        });
                        let response = field(
                            ui,
                            &mut self.state,
                            id,
                            MarkerColumn::Artist,
                            &mut text,
                            false,
                        )
                        .on_hover_text("Paste Artist - Title to fill both fields.");
                        if response.changed() {
                            commands.push(Command::SetArtist(id, text, pasted));
                        }
                    });
                    row.col(|ui| {
                        let mut text = marker.data.title.clone();
                        let response = field(
                            ui,
                            &mut self.state,
                            id,
                            MarkerColumn::Title,
                            &mut text,
                            false,
                        );
                        if response.changed() {
                            commands.push(Command::SetTitle(id, text));
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
    state: &mut UiState,
    marker: MarkerId,
    column: MarkerColumn,
    value: &mut String,
    invalid: bool,
) -> egui::Response {
    let id = column.field_id(marker);
    let label = column.label();
    if state.pending_cell_focus == Some(id) {
        ui.memory_mut(|memory| memory.request_focus(id));
        state.pending_cell_focus = None;
    }
    let edit = egui::TextEdit::singleline(value)
        .id(id)
        .desired_width(f32::INFINITY)
        .hint_text(label);
    let response = ui.add(if invalid {
        edit.text_color(theme::ERROR)
    } else {
        edit
    });
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label));
    let active = (Some(marker), column);
    if (response.clicked() || response.has_focus())
        && (state.selected, state.active_column) != active
    {
        (state.selected, state.active_column) = active;
        state.pending_cell_focus = None;
        ui.ctx().request_repaint();
    }
    if (state.selected, state.active_column) == active {
        ui.painter().rect_stroke(
            response.rect,
            ui.visuals().widgets.inactive.corner_radius,
            egui::Stroke::new(2.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    response
}
