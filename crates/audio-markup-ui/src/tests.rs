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

fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        events,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 500.0),
        )),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        app.shortcuts(ui.ctx());
        egui::Panel::top("transport").show(ui, |ui| app.toolbar(ui));
        egui::CentralPanel::default().show(ui, |ui| app.marker_table(ui));
        app.show_shortcut_help(ui.ctx());
    });
    output.textures_delta.clear();
    output
}

fn key(key: egui::Key, modifiers: egui::Modifiers, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

fn alt(pressed_key: egui::Key) -> egui::Event {
    key(pressed_key, egui::Modifiers::ALT, true)
}

fn tap(ctx: &egui::Context, app: &mut App, pressed_key: egui::Key, modifiers: egui::Modifiers) {
    frame(
        ctx,
        app,
        vec![
            key(pressed_key, modifiers, true),
            key(pressed_key, modifiers, false),
        ],
    );
}

#[test]
fn arrow_keys_select_rows_in_table_order_and_leave_cell_editing_alone() {
    use egui::{Key, Modifiers};

    let (ctx, mut app, services, _) = app();
    app.dispatch(Command::AddMarker);
    let first = app.session.markers()[0].id;
    let second = app.session.markers()[1].id;

    tap(&ctx, &mut app, Key::ArrowDown, Modifiers::NONE);
    assert_eq!(app.state.selected, Some(first));
    let field = egui::Id::unique(("audio-markup-marker", first, "Artist"));
    ctx.memory_mut(|memory| memory.request_focus(field));
    frame(&ctx, &mut app, vec![]);
    tap(&ctx, &mut app, Key::ArrowDown, Modifiers::NONE);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(field));
    assert_eq!(app.state.selected, Some(first));

    tap(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    tap(&ctx, &mut app, Key::ArrowDown, Modifiers::NONE);
    tap(&ctx, &mut app, Key::ArrowDown, Modifiers::NONE);
    assert_eq!(app.state.selected, Some(second));
    tap(&ctx, &mut app, Key::ArrowUp, Modifiers::NONE);
    assert_eq!(app.state.selected, Some(first));
    assert!(services.0.borrow().is_empty());
}

#[test]
fn selecting_an_offscreen_marker_scrolls_it_into_view() {
    let (ctx, mut app, _, _) = app();
    for _ in 0..30 {
        app.dispatch(Command::AddMarker);
    }
    let last = app.session.markers().last().unwrap().id;
    let field = egui::Id::unique(("audio-markup-marker", last, "Pos"));

    frame(&ctx, &mut app, vec![]);
    assert!(ctx.read_response(field).is_none());
    tap(&ctx, &mut app, egui::Key::ArrowUp, egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.state.selected, Some(last));
    assert!(
        ctx.read_response(field)
            .unwrap()
            .interact_rect
            .is_positive()
    );
}

#[test]
fn add_and_delete_shortcuts_update_selection_without_repeating() {
    use egui::{Key, Modifiers};

    let (ctx, mut app, _, _) = app();
    let first = app.session.markers()[0].id;
    frame(&ctx, &mut app, vec![alt(Key::N), alt(Key::N)]);
    assert_eq!(app.session.markers().len(), 2);
    let second = app.session.markers()[1].id;
    assert_eq!(app.state.selected, Some(second));
    assert!(app.session.markers()[1].data.position.is_none());

    tap(&ctx, &mut app, Key::ArrowUp, Modifiers::NONE);
    assert_eq!(app.state.selected, Some(first));
    frame(&ctx, &mut app, vec![alt(Key::D), alt(Key::D)]);
    assert_eq!(app.session.markers().len(), 1);
    assert_eq!(app.session.markers()[0].id, second);
    assert_eq!(app.state.selected, Some(second));
    frame(&ctx, &mut app, vec![key(Key::D, Modifiers::ALT, false)]);
    tap(&ctx, &mut app, Key::D, Modifiers::ALT);
    assert!(app.session.markers().is_empty());
    assert_eq!(app.state.selected, None);
}

#[test]
fn toolbar_help_opens_and_blocks_marker_shortcuts_until_closed() {
    use egui::{Key, Modifiers, accesskit};

    let (ctx, mut app, _, _) = app();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![]);
    let update = output.platform_output.accesskit_update.unwrap();
    let (button, _) = update
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == accesskit::Role::Button && node.label() == Some("Keyboard shortcuts")
        })
        .unwrap();

    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            accesskit::ActionRequest {
                action: accesskit::Action::Click,
                target_node: *button,
                target_tree: accesskit::TreeId::ROOT,
                data: None,
            },
        )],
    );
    assert!(app.state.shortcut_help_open);
    tap(&ctx, &mut app, Key::N, Modifiers::ALT);
    assert_eq!(app.session.markers().len(), 1);
    tap(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(!app.state.shortcut_help_open);
    tap(&ctx, &mut app, Key::N, Modifiers::ALT);
    assert_eq!(app.session.markers().len(), 2);
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
