# Building a Rust desktop app with the PhotoCraft approach

This guide describes the architecture of the Rust desktop implementation of Audio Markup. The original Flutter behavior informed playback, seeking, marker editing, and tagged MP3 splitting. [Desktop launcher](apps/audio-markup-desktop/src/main.rs) · [Marker table](crates/audio-markup-ui/src/markers.rs) · [Current features](FEATURES.md)

## What PhotoCraft does

The reference is PhotoCraft commit [`71b76f8`](https://github.com/storytold/photocraft/commit/71b76f89126c74fc9d5fb13220f92176745580e2).

| Layer | How PhotoCraft implements it |
| --- | --- |
| Native launcher | `apps/photocraft` sets window and renderer options, constructs native services, and creates `PhotocraftApp` inside `eframe::run_native`. It supplies services for desktop integration and wakeups. [Desktop entry point](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/apps/photocraft/src/main.rs) |
| Rust UI | `crates/ui-egui` owns `PhotocraftApp` and `UiState`. egui builds controls and lays out the title and options bars, docked panels, canvas, status, and dialogs each UI pass. eframe provides the native window and event loop, with wgpu rendering. `App::logic` and `App::ui` delegate to separate frame methods. [App lifecycle](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/crates/ui-egui/src/app.rs) · [Frame composition](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/crates/ui-egui/src/app/frame.rs) |
| Headless engine | `crates/engine` owns the session and command dispatch. Its `Session::execute` accepts a registered command ID and JSON parameters. The GUI wrapper handles presentation and delegates to the engine. CLI and automation use the same engine. [Command execution](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/crates/engine/src/session/execute.rs) · [UI dispatch](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/crates/ui-egui/src/app/commands.rs) · [Command engine guide](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/book/src/architecture/command-engine.md) |
| Shared UI and long operations | Theme tokens supply colors, fonts, and sizing. Background jobs report progress and completion to the UI, which requests repaint while work is pending. [Theme](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/crates/ui-egui/src/theme.rs) · [Job UI](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/crates/ui-egui/src/jobs_ui.rs) |

Reuse the dependency direction and the command and frame patterns. For a small editor, plain modules and a single-window layout cover the initial scope. Add docking, CLI, automation, or custom GPU rendering when a feature needs them.

## Audio Markup workspace

```text
Cargo.toml                      # Cargo workspace and shared dependency versions
apps/audio-markup-desktop/      # main.rs, window options, native adapters
crates/audio-markup-ui/         # App, UiState, theme, shortcuts, views
crates/audio-markup-engine/     # session, model, commands, playback events, export plan
```

- **Engine.** Own the active track, playback state and marker list. Give each marker a stable ID and store positions as `Duration`. Parse and format timestamps at the boundary. Expose a typed `Command` enum and `Session::execute` so toolbar buttons, shortcuts, and headless tests share validation and effects. Start with typed Rust values. Introduce serialized command IDs at an external API boundary if one is added.
- **UI.** Own the selected marker ID, text being edited, focused control, dialogs, and visible status in `UiState`. Render from the engine's current state. Feed user actions back as commands such as `Play`, `SeekToMarker(id)`, `SetMarkerFromPlayhead(id)`, and `UpdateMarker { … }`. Keep a temporary position text buffer while a cell is being edited. Queue commands while laying out rows and dispatch after the table borrow ends.
- **Desktop.** Construct the audio output, native file and directory dialogs, and FFmpeg adapter, then pass them into the app behind small interfaces. Use the playback backend's reported position as the clock. Keep decoding and export off the UI thread. Pass worker results back as events and ignore results from an earlier track after opening another one. Keep export tied to a snapshot of the track and markers selected when it starts.

For example, both a shortcut and a row action can emit `SeekToMarker(id)`. The session resolves the ID and validates the position, the playback adapter seeks, and a backend event updates the displayed playhead. The same flow can be tested with a fake playback adapter.

## Build the UI in Rust

Use **egui** for widgets and layout, **eframe** to host the native app, and **wgpu** for window rendering. egui is immediate mode. In each pass you call the widgets you want to show, then use their `Response` values to detect input. Rust structs retain application data between passes. PhotoCraft pins a patched egui family and uses a shared Craft framework. Start with a matched upstream `eframe` and `egui_extras` release and use `eframe::egui` for UI types. The examples use the 0.37 lifecycle. [PhotoCraft workspace manifest](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/Cargo.toml) · [Upstream eframe 0.37 lifecycle](https://github.com/emilk/egui/blob/0.37.0/crates/eframe/src/epi.rs)

Declare `eframe` in both the launcher and UI packages through `[workspace.dependencies]`, and keep `egui_extras` in the UI package.

```toml
[dependencies]
eframe = { version = "0.37", default-features = false, features = ["wgpu", "default_fonts", "accesskit", "clipboard", "wayland", "x11"] }
egui_extras = { version = "0.37", default-features = false }
```

This standalone window demonstrates the startup and draw callbacks. Move `App` to the UI crate when wiring the session and services.

```rust
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 600.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native("Audio Markup", options, Box::new(|_cc| Ok(Box::new(App))))
}

struct App;

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Audio Markup");
        });
    }
}
```

1. Install the shared theme in the `run_native` creation callback using `cc.egui_ctx`. Define fonts, spacing, selection colors, and error styling in a `theme` module. For window geometry and user preferences across restarts, enable eframe persistence when needed and keep those settings separate from marker data.
2. Build a transport row with `ui.horizontal`, buttons, a position readout, and sliders. Use `egui_extras::TableBuilder` for the scrollable marker table with a fixed header. Scope cell widget IDs by marker ID and column so inserting or reordering rows preserves the intended focus. Use `TextEdit` for the cells and put export feedback in a status area. [egui table API](https://docs.rs/egui_extras/0.37.0/egui_extras/struct.TableBuilder.html)
3. Read widget responses such as `.clicked()` and `.changed()` to issue commands. Route shortcuts through the same dispatcher and let a focused text editor consume its editing keys first. Give controls accessible names, expose keyboard focus, and convey selection and errors with text or another cue in addition to color.
4. Drain playback and worker events in `App::logic`. Wake egui with `Context::request_repaint()` after publishing a result. Use `request_repaint_after` for the playhead while playback is active if the backend provides no position events. Draw the resulting state in `App::ui`. For a future waveform, cache decoded peaks, then allocate an interactive rectangle and draw the visible peaks with egui's painter.

## Workflow and verification

Exercise the full workflow when changing behavior. Open an audio file with the native dialog, play and seek, capture the playhead into a marker, edit its metadata, and export from the marker list. Verify seeking against an audio device on the target desktop. Include CUE import and export when changing timestamps or marker metadata. [Feature behavior](FEATURES.md) · [Build and integration checks](README.md)

Make the export plan independently testable. Validate segment bounds and output names before starting FFmpeg. Pass an argument list directly to `std::process::Command`, capture failures per segment, and show progress from the worker. Exercise open, seek boundaries, marker edits, and export planning headlessly. Use an egui test harness for focus, shortcut routing, and table interactions, then smoke-test native dialogs, playback, and export with a known audio fixture on the target desktop. PhotoCraft has separate integration tests for its UI crate and a development guide for its native build prerequisites. [PhotoCraft UI test suite](https://github.com/storytold/photocraft/tree/71b76f89126c74fc9d5fb13220f92176745580e2/crates/ui-egui/tests/it) · [PhotoCraft development guide](https://github.com/storytold/photocraft/blob/71b76f89126c74fc9d5fb13220f92176745580e2/docs/development.md)

Run these commands from the workspace root. Document the system packages required by the chosen audio, dialog, and window backends and how FFmpeg is provided for the target desktop.

```sh
cargo run -p audio-markup-desktop
cargo build --release -p audio-markup-desktop
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
```
