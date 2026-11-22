#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod audio;
mod files;

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread,
};

use audio_markup_engine::{Effect, Event, Result};
use audio_markup_ui::{App, FileRequest, Services};
use eframe::egui;

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id("audio-markup")
        .with_inner_size([920.0, 590.0])
        .with_min_inner_size([620.0, 370.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon.png"))
    {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Audio Markup",
        options,
        Box::new(|context| {
            let (sender, receiver) = mpsc::channel();
            let events = EventSender {
                sender,
                context: context.egui_ctx.clone(),
            };
            let (audio, commands) = mpsc::channel();
            let audio_events = events.clone();
            thread::Builder::new()
                .name("audio-playback".into())
                .spawn(move || audio::run(commands, audio_events))?;
            Ok(Box::new(App::new(
                &context.egui_ctx,
                Box::new(Desktop { audio, events }),
                receiver,
            )))
        }),
    )
}

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct EventSender {
    sender: Sender<Event>,
    context: egui::Context,
}

impl EventSender {
    fn send(&self, event: Event) {
        if self.sender.send(event).is_ok() {
            self.context.request_repaint();
        }
    }
}

struct Desktop {
    audio: Sender<Effect>,
    events: EventSender,
}

impl Services for Desktop {
    fn submit(&self, effect: Effect) {
        if let Effect::Split(plan) = effect {
            let events = self.events.clone();
            if let Err(error) = thread::Builder::new()
                .name("audio-export".into())
                .spawn(move || files::split(plan, &events))
            {
                self.events.send(Event::SplitFinished(Err(format!(
                    "Could not start the split: {error}"
                ))));
            }
        } else if let Err(error) = self.audio.send(effect) {
            let message = "The audio worker stopped.".to_owned();
            if matches!(error.0, Effect::Load { .. }) {
                self.events.send(Event::Loaded(Err(message)));
            } else {
                self.events.send(Event::Error(message));
            }
        }
    }

    fn choose(&self, request: FileRequest) -> Option<PathBuf> {
        let dialog = rfd::FileDialog::new();
        match request {
            FileRequest::Audio => dialog
                .set_title("Open Audio")
                .add_filter(
                    "Audio",
                    &[
                        "mp3", "wav", "flac", "ogg", "m4a", "aac", "mp4", "aif", "aiff", "caf",
                        "mka",
                    ],
                )
                .add_filter("All files", &["*"])
                .pick_file(),
            FileRequest::Cue => dialog
                .set_title("Import CUE")
                .add_filter("CUE sheet", &["cue"])
                .pick_file(),
            FileRequest::SaveCue(name) => dialog
                .set_title("Export CUE")
                .set_file_name(name)
                .add_filter("CUE sheet", &["cue"])
                .save_file(),
            FileRequest::SplitFolder => dialog.set_title("Choose MP3 Output Folder").pick_folder(),
        }
    }

    fn confirm(&self, title: &str, message: &str) -> bool {
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(message)
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show()
            == rfd::MessageDialogResult::Ok
    }

    fn save_cue(&self, path: &Path, contents: &str, replace: bool) -> Result<()> {
        files::save_cue(path, contents, replace)
    }
}
