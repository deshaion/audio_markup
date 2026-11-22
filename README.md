# Audio Markup

Audio Markup is a native Rust desktop editor for marking track starts in a long recording, exchanging CUE sheets, and splitting the audio into tagged MP3s.

## Release downloads

Publishing a GitHub release builds and attaches these packages from the release tag.

- **Linux x86-64** app in AppImage format. Make it executable with `chmod +x AudioMarkup-linux-x86_64.AppImage`. If FUSE is unavailable, run `./AudioMarkup-linux-x86_64.AppImage --appimage-extract-and-run`.
- **Windows x86-64** executable in a ZIP archive.
- **macOS** app bundles in separate ZIP archives for Apple Silicon and Intel. These builds use ad hoc signing without Apple notarization.

MP3 export uses FFmpeg from the app's `PATH`. On macOS, if Finder cannot find a terminal-installed FFmpeg, launch the bundle executable from a terminal where `ffmpeg` is on `PATH`.

## Run from source

Install Rust 1.95 or newer and put FFmpeg on your system path for MP3 export. On Linux, the build needs ALSA development headers and the app needs a desktop graphics driver and an XDG Desktop Portal file chooser. For a Debian or Ubuntu desktop, the relevant packages include `libasound2-dev`, `libxkbcommon-dev`, `mesa-vulkan-drivers`, `xdg-desktop-portal`, a portal backend for your desktop, and `ffmpeg`.

```sh
cargo run --locked -p audio-markup-desktop
```

Open audio or import a CUE sheet. Add a marker at each track start, enter its artist and title, and choose **Split to MP3…** to write the tagged files. Select a marker, then use **Alt+X** to capture the playhead or **Alt+S** to seek to that marker. [All features and shortcuts](FEATURES.md)

## Build and test

```sh
cargo build --locked --release -p audio-markup-desktop
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Run the desktop integration checks with FFmpeg, ffprobe, and an audio output device available.

```sh
cargo test --locked -p audio-markup-desktop -- --ignored
```

The desktop crate starts the native window and supplies dialogs, audio playback, and FFmpeg. The egui crate owns views and selection state. The engine crate owns marker data, typed commands, CUE encoding, and split planning. [Architecture guide](Agents.md)
