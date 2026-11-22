use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self, Sender},
};

use audio_markup_engine::Track;

use super::*;

#[derive(Default)]
struct FakeServices(Rc<RefCell<Vec<Effect>>>);

impl Services for FakeServices {
    fn submit(&self, effect: Effect) {
        self.0.borrow_mut().push(effect);
    }

    fn choose(&self, _request: FileRequest) -> Option<PathBuf> {
        None
    }

    fn confirm(&self, _title: &str, _message: &str) -> bool {
        false
    }

    fn save_cue(&self, _path: &Path, _contents: &str, _replace: bool) -> Result<()> {
        unreachable!()
    }
}

fn app() -> (egui::Context, App, FakeServices, Sender<Event>) {
    let ctx = egui::Context::default();
    let services = FakeServices::default();
    let (sender, receiver) = mpsc::channel();
    let app = App::new(&ctx, Box::new(FakeServices(services.0.clone())), receiver);
    (ctx, app, services, sender)
}

fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        events,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 500.0),
        )),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        app.shortcuts(ui.ctx());
        egui::CentralPanel::default().show(ui, |ui| app.marker_table(ui));
    })
    .textures_delta
    .clear();
}

fn alt(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::ALT,
    }
}

#[test]
fn selected_row_shortcuts_capture_and_seek_via_the_engine() {
    let (ctx, mut app, services, sender) = app();
    app.execute(Command::Open {
        path: "set.wav".into(),
        markers: None,
    })
    .unwrap();
    sender
        .send(Event::Loaded(Ok(Track {
            path: "set.wav".into(),
            duration: Some(Duration::from_secs(120)),
        })))
        .unwrap();
    sender
        .send(Event::Playback {
            position: Duration::from_secs(31),
            playing: true,
        })
        .unwrap();
    app.poll_events();

    frame(&ctx, &mut app, vec![]);
    let id = app.session.markers()[0].id;
    ctx.memory_mut(|memory| {
        memory.request_focus(egui::Id::unique(("audio-markup-marker", id, "Pos")))
    });
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.state.selected, Some(id));

    frame(&ctx, &mut app, vec![alt(egui::Key::X), alt(egui::Key::S)]);
    assert_eq!(
        app.session.markers()[0].data.position,
        Some(Duration::from_secs(31))
    );
    assert!(
        matches!(services.0.borrow().last(), Some(Effect::Seek(position)) if *position == Duration::from_secs(31))
    );
}

#[test]
fn split_completion_uses_a_notification_even_without_playback() {
    let (_ctx, mut app, _, sender) = app();
    sender.send(Event::SplitFinished(Ok(2))).unwrap();
    app.poll_events();
    let notice = app.state.notice.as_ref().unwrap();
    assert_eq!(notice.title, "Split is completed");
    assert!(notice.detail.contains("2 MP3 files"));
    assert!(notice.expires > Instant::now());
}
