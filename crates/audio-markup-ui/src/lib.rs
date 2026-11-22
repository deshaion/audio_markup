mod markers;
mod shortcuts;
mod theme;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use audio_markup_engine::{Command, CueSheet, Effect, Event, MarkerId, Result, Session};
use eframe::egui::{self, Color32, RichText};

pub enum FileRequest {
    Audio,
    Cue,
    SaveCue(String),
    SplitFolder,
}

pub trait Services {
    fn submit(&self, effect: Effect);
    fn choose(&self, request: FileRequest) -> Option<PathBuf>;
    fn confirm(&self, title: &str, message: &str) -> bool;
    fn save_cue(&self, path: &Path, contents: &str, replace: bool) -> Result<()>;
}

pub struct App {
    session: Session,
    state: UiState,
    services: Box<dyn Services>,
    events: Receiver<Event>,
}

#[derive(Default)]
struct UiState {
    selected: Option<MarkerId>,
    scroll_to_marker: Option<usize>,
    positions: HashMap<MarkerId, markers::PositionDraft>,
    shortcut_help_open: bool,
    error: Option<String>,
    notice: Option<Notice>,
}

struct Notice {
    title: &'static str,
    detail: String,
    expires: Instant,
}

impl App {
    pub fn new(
        context: &egui::Context,
        services: Box<dyn Services>,
        events: Receiver<Event>,
    ) -> Self {
        theme::install(context);
        Self {
            session: Session::default(),
            state: UiState::default(),
            services,
            events,
        }
    }

    fn dispatch(&mut self, command: Command) {
        if let Err(error) = self.execute(command) {
            self.state.error = Some(error);
        }
    }

    fn execute(&mut self, command: Command) -> Result<()> {
        if let Some(effect) = self.session.execute(command)? {
            self.services.submit(effect);
        }
        Ok(())
    }

    fn open(&mut self) -> Result<()> {
        if let Some(path) = self.services.choose(FileRequest::Audio) {
            self.execute(Command::Open {
                path,
                markers: None,
            })?;
        }
        Ok(())
    }

    fn import_cue(&mut self) -> Result<()> {
        let Some(cue_path) = self.services.choose(FileRequest::Cue) else {
            return Ok(());
        };
        let text = fs::read_to_string(&cue_path)
            .map_err(|error| format!("Could not read the CUE sheet: {error}"))?;
        let cue = CueSheet::parse(&text)?;
        let mut path = cue.resolve_audio(&cue_path);
        if !path.is_file() {
            let message = format!(
                "The CUE sheet references {}. Choose the matching audio file to continue.",
                cue.audio_file
            );
            if !self.services.confirm("Locate source audio", &message) {
                return Ok(());
            }
            let Some(chosen) = self.services.choose(FileRequest::Audio) else {
                return Ok(());
            };
            path = chosen;
        }
        let rows = self.session.markers();
        if (rows.len() != 1
            || rows[0].data.position != Some(Duration::ZERO)
            || !rows[0].data.artist.is_empty()
            || !rows[0].data.title.is_empty()
            || self
                .state
                .positions
                .values()
                .any(|draft| draft.error.is_some()))
            && !self.services.confirm(
                "Import CUE",
                "Replace the marker rows with the tracks from this CUE sheet?",
            )
        {
            return Ok(());
        }
        self.execute(Command::Open {
            path,
            markers: Some(cue.markers),
        })
    }

    fn export_cue(&mut self) -> Result<()> {
        self.check_positions()?;
        let track = self.session.track().ok_or("Open an audio file first.")?;
        let stem = track
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("markers");
        let Some(mut path) = self
            .services
            .choose(FileRequest::SaveCue(format!("{stem}.cue")))
        else {
            return Ok(());
        };
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("cue"))
        {
            let file = path.file_name().ok_or("Choose a CUE filename.")?;
            let mut file = file.to_os_string();
            file.push(".cue");
            path.set_file_name(file);
        }
        if path == track.path {
            return Err("Choose a different filename from the source audio.".into());
        }
        let contents = CueSheet::encode(track, self.session.markers(), &path)?;
        let replace = path.exists();
        if replace
            && !self
                .services
                .confirm("Replace CUE file?", &format!("Replace {}?", path.display()))
        {
            return Ok(());
        }
        self.services.save_cue(&path, &contents, replace)?;
        self.notice("CUE sheet saved", path.display().to_string());
        Ok(())
    }

    fn split(&mut self) -> Result<()> {
        self.check_positions()?;
        if let Some(directory) = self.services.choose(FileRequest::SplitFolder) {
            self.execute(Command::Split(directory))?;
        }
        Ok(())
    }

    fn check_positions(&self) -> Result<()> {
        for (index, marker) in self.session.markers().iter().enumerate() {
            if let Some(error) = self
                .state
                .positions
                .get(&marker.id)
                .and_then(|draft| draft.error.as_ref())
            {
                return Err(format!("Row {}: {error}", index + 1));
            }
        }
        Ok(())
    }

    fn capture(&mut self) -> Result<()> {
        let id = self.state.selected.ok_or("Select a marker row first.")?;
        self.execute(Command::CapturePosition(id))?;
        self.state.positions.remove(&id);
        Ok(())
    }

    fn seek_to_marker(&mut self) -> Result<()> {
        let id = self.state.selected.ok_or("Select a marker row first.")?;
        if let Some(error) = self
            .state
            .positions
            .get(&id)
            .and_then(|draft| draft.error.clone())
        {
            return Err(error);
        }
        self.execute(Command::SeekToMarker(id))
    }

    fn notice(&mut self, title: &'static str, detail: String) {
        self.state.notice = Some(Notice {
            title,
            detail,
            expires: Instant::now() + Duration::from_secs(6),
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let opening = self.session.opening();
        let ready = self.session.track().is_some() && !opening;
        let split_in_progress = self.session.split_progress().is_some();
        ui.horizontal_wrapped(|ui| {
            ui.heading("Audio Markup");
            ui.separator();
            if ui
                .add_enabled(!opening, egui::Button::new("Open Audio…"))
                .clicked()
            {
                let result = self.open();
                self.report(result);
            }
            if ui
                .add_enabled(!opening, egui::Button::new("Import CUE…"))
                .clicked()
            {
                let result = self.import_cue();
                self.report(result);
            }
            if ui
                .add_enabled(ready, egui::Button::new("Export CUE…"))
                .clicked()
            {
                let result = self.export_cue();
                self.report(result);
            }
            if ui
                .add_enabled(
                    ready && !split_in_progress,
                    egui::Button::new(RichText::new("Split to MP3…").color(Color32::WHITE))
                        .fill(theme::ACCENT),
                )
                .clicked()
            {
                let result = self.split();
                self.report(result);
            }
            self.shortcut_help_button(ui);
        });
        ui.add_space(4.0);
        let source = if opening {
            "Opening audio…".into()
        } else {
            self.session.track().map_or_else(
                || "Open an audio file or import a CUE sheet.".into(),
                |track| track.path.display().to_string(),
            )
        };
        ui.add(egui::Label::new(RichText::new(&source).color(theme::MUTED)).truncate())
            .on_hover_text(&source);

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(ready, |ui| {
                if ui
                    .button("−10 s")
                    .on_hover_text("Seek backward (Alt+Left)")
                    .clicked()
                {
                    self.dispatch(Command::SeekBy(-10));
                }
                if self.session.playing() {
                    if ui.button("Pause").on_hover_text("Alt+Down").clicked() {
                        self.dispatch(Command::SetPlaying(false));
                    }
                } else if ui.button("Play").on_hover_text("Alt+Up").clicked() {
                    self.dispatch(Command::SetPlaying(true));
                }
                if ui
                    .button("+10 s")
                    .on_hover_text("Seek forward (Alt+Right)")
                    .clicked()
                {
                    self.dispatch(Command::SeekBy(10));
                }
            });
            ui.label(
                RichText::new(format!(
                    "{} / {}",
                    clock(self.session.position()),
                    self.session
                        .track()
                        .and_then(|track| track.duration)
                        .map_or_else(|| "—".into(), clock)
                ))
                .monospace()
                .strong(),
            );
            ui.separator();
            let mut volume = self.session.volume();
            let slider = egui::Slider::new(&mut volume, 0..=100)
                .step_by(10.0)
                .suffix("%")
                .text("Volume");
            if ui.add(slider).changed() {
                self.dispatch(Command::SetVolume(volume));
            }
        });
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        if let Some((completed, total)) = self.session.split_progress() {
            ui.add(
                egui::ProgressBar::new(completed as f32 / total as f32)
                    .text(format!("Splitting audio · {completed} of {total}")),
            );
        }
        if let Some(message) = &self.state.error {
            let dismiss = ui
                .horizontal_wrapped(|ui| {
                    let dismiss = ui.button("Dismiss").clicked();
                    egui::ScrollArea::vertical()
                        .max_height(110.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(message).color(theme::ERROR));
                        });
                    dismiss
                })
                .inner;
            if dismiss {
                self.state.error = None;
            }
        }
    }

    fn show_notice(&mut self, context: &egui::Context) {
        let Some(notice) = &self.state.notice else {
            return;
        };
        let now = Instant::now();
        if now >= notice.expires {
            self.state.notice = None;
            return;
        }
        context.request_repaint_after(notice.expires - now);
        let dismissed = egui::Area::new(egui::Id::unique("audio-markup-notification"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
            .order(egui::Order::Foreground)
            .show(context, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_width(270.0);
                        let title =
                            ui.label(RichText::new(notice.title).strong().color(theme::ACCENT));
                        context.accesskit_node_builder(title.id, |node| {
                            node.set_live(egui::accesskit::Live::Polite)
                        });
                        ui.add(egui::Label::new(&notice.detail).truncate())
                            .on_hover_text(&notice.detail);
                        ui.small_button("Dismiss").clicked()
                    })
                    .inner
            })
            .inner;
        if dismissed {
            self.state.notice = None;
        }
    }

    fn report(&mut self, result: Result<()>) {
        if let Err(error) = result {
            self.state.error = Some(error);
        }
    }
    fn poll_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            let loaded = matches!(event, Event::Loaded(Ok(_)));
            match self.session.apply(event) {
                Ok(Some(count)) => self.notice(
                    "Split is completed",
                    format!(
                        "{count} MP3 {} exported.",
                        if count == 1 { "file" } else { "files" }
                    ),
                ),
                Ok(None) => {}
                Err(error) => self.state.error = Some(error),
            }
            if loaded {
                self.state
                    .positions
                    .retain(|id, _| self.session.markers().iter().any(|marker| marker.id == *id));
                self.state.selected = self
                    .state
                    .selected
                    .filter(|id| self.session.markers().iter().any(|marker| marker.id == *id));
            }
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, _context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shortcuts(ui.ctx());
        egui::Panel::top("transport")
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(12)
                    .stroke(egui::Stroke::new(1.0, theme::LINE)),
            )
            .show(ui, |ui| self.toolbar(ui));
        if self.session.split_progress().is_some() || self.state.error.is_some() {
            egui::Panel::bottom("status").show(ui, |ui| self.footer(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| self.marker_table(ui));
        self.show_notice(ui.ctx());
        self.show_shortcut_help(ui.ctx());
    }
}

#[cfg(test)]
mod tests;

fn clock(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
