use audio_markup_engine::Command;
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::App;

#[derive(Clone, Copy)]
enum Action {
    Play,
    Pause,
    SeekBy(i64),
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    EditCell,
    Add,
    Delete,
    Capture,
    SeekToMarker,
}

const SHORTCUTS: &[(KeyboardShortcut, Action, &str)] = &[
    (
        KeyboardShortcut::new(Modifiers::NONE, Key::ArrowUp),
        Action::MoveUp,
        "Select previous marker",
    ),
    (
        KeyboardShortcut::new(Modifiers::NONE, Key::ArrowDown),
        Action::MoveDown,
        "Select next marker",
    ),
    (
        KeyboardShortcut::new(Modifiers::NONE, Key::ArrowLeft),
        Action::MoveLeft,
        "Select previous cell",
    ),
    (
        KeyboardShortcut::new(Modifiers::NONE, Key::ArrowRight),
        Action::MoveRight,
        "Select next cell",
    ),
    (
        KeyboardShortcut::new(Modifiers::NONE, Key::Enter),
        Action::EditCell,
        "Edit active cell",
    ),
    (
        KeyboardShortcut::new(Modifiers::ALT, Key::N),
        Action::Add,
        "Add marker",
    ),
    (
        KeyboardShortcut::new(Modifiers::ALT, Key::D),
        Action::Delete,
        "Delete selected marker",
    ),
    (
        KeyboardShortcut::new(Modifiers::ALT, Key::X),
        Action::Capture,
        "Set marker from playhead",
    ),
    (
        KeyboardShortcut::new(Modifiers::ALT, Key::S),
        Action::SeekToMarker,
        "Seek to selected marker",
    ),
    (
        KeyboardShortcut::new(Modifiers::ALT, Key::ArrowUp),
        Action::Play,
        "Resume playback",
    ),
    (
        KeyboardShortcut::new(Modifiers::ALT, Key::ArrowDown),
        Action::Pause,
        "Pause playback",
    ),
    (
        KeyboardShortcut::new(Modifiers::ALT, Key::ArrowLeft),
        Action::SeekBy(-10),
        "Seek back 10 seconds",
    ),
    (
        KeyboardShortcut::new(Modifiers::ALT, Key::ArrowRight),
        Action::SeekBy(10),
        "Seek forward 10 seconds",
    ),
];

impl Action {
    fn is_navigation(self) -> bool {
        matches!(
            self,
            Self::MoveUp | Self::MoveDown | Self::MoveLeft | Self::MoveRight
        )
    }
}

impl App {
    pub(crate) fn shortcuts(&mut self, context: &egui::Context) {
        if self.state.shortcut_help_open {
            return;
        }
        if self.state.pending_cell_focus.is_some()
            && context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.state.pending_cell_focus = None;
        }
        let mut editing = focused_text_edit(context).is_some();
        let mut widget_focused = context.memory(|memory| memory.focused().is_some());
        let has_markers = !self.session.markers().is_empty();
        let mut directional_key = false;
        let actions = context.input_mut(|input| {
            let mut actions = Vec::new();
            input.events.retain(|event| {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                let Some((_, action, _)) = SHORTCUTS.iter().find(|(binding, _, _)| {
                    binding.logical_key == *key && modifiers.matches_exact(binding.modifiers)
                }) else {
                    return true;
                };
                if action.is_navigation() {
                    directional_key = true;
                    if editing {
                        return true;
                    }
                    widget_focused = false;
                }
                if matches!(action, Action::EditCell) {
                    if widget_focused || editing || !has_markers {
                        return true;
                    }
                    editing = !repeat;
                }
                if !repeat || action.is_navigation() || matches!(action, Action::SeekBy(_)) {
                    actions.push(*action);
                }
                false
            });
            actions
        });

        if directional_key {
            // egui checks directional focus before the app consumes key events.
            context.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        for action in actions {
            if action.is_navigation() {
                context.memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.surrender_focus(id);
                    }
                });
            }
            let result = match action {
                Action::Play => self.execute(Command::SetPlaying(true)),
                Action::Pause => self.execute(Command::SetPlaying(false)),
                Action::SeekBy(seconds) => self.execute(Command::SeekBy(seconds)),
                Action::MoveUp | Action::MoveDown => {
                    self.move_marker(matches!(action, Action::MoveDown));
                    Ok(())
                }
                Action::MoveLeft | Action::MoveRight => {
                    self.move_marker_column(matches!(action, Action::MoveRight));
                    Ok(())
                }
                Action::EditCell => {
                    self.edit_marker_cell();
                    Ok(())
                }
                Action::Add => self.add_marker(context),
                Action::Delete => self
                    .state
                    .selected
                    .map_or(Ok(()), |id| self.delete_marker(context, id)),
                Action::Capture => self.capture(),
                Action::SeekToMarker => self.seek_to_marker(),
            };
            self.report(result);
        }
    }

    pub(crate) fn shortcut_help_button(&mut self, ui: &mut egui::Ui) {
        let response = ui.button("?").on_hover_text("Keyboard shortcuts");
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label("Keyboard shortcuts");
        });
        if response.clicked() {
            self.state.shortcut_help_open = true;
            self.state.pending_cell_focus = None;
        }
    }

    pub(crate) fn show_shortcut_help(&mut self, context: &egui::Context) {
        if !self.state.shortcut_help_open {
            return;
        }
        let modal = egui::Modal::new(egui::Id::unique("shortcut-help")).show(context, |ui| {
            ui.heading("Keyboard shortcuts");
            ui.add_space(6.0);
            egui::Grid::new("shortcut-list")
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (binding, _, description) in SHORTCUTS {
                        ui.monospace(context.format_shortcut(binding));
                        ui.label(*description);
                        ui.end_row();
                    }
                });
            ui.add_space(6.0);
            ui.label("Press Esc to finish editing and return to the active cell.");
            ui.add_space(6.0);
            if ui.button("Close").clicked() {
                ui.close();
            }
        });
        self.state.shortcut_help_open = !modal.should_close();
    }
}

pub(crate) fn focused_text_edit(context: &egui::Context) -> Option<egui::Id> {
    context
        .memory(|memory| memory.focused())
        .filter(|id| egui::TextEdit::load_state(context, *id).is_some())
}
