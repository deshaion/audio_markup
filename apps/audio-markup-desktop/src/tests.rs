use std::{
    fs,
    path::Path,
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use audio_markup_engine::{Effect, Event, Segment, SplitPlan};
use eframe::egui;

use crate::{EventSender, audio, files};

fn fixture(path: &Path, seconds: u32) {
    let length = 48_000 * 2 * seconds;
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + length).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16_u32.to_le_bytes());
    wav.extend(1_u16.to_le_bytes()); // PCM
    wav.extend(1_u16.to_le_bytes()); // Mono
    wav.extend(48_000_u32.to_le_bytes());
    wav.extend(96_000_u32.to_le_bytes());
    wav.extend(2_u16.to_le_bytes());
    wav.extend(16_u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(length.to_le_bytes());
    wav.resize(44 + length as usize, 0);
    fs::write(path, wav).unwrap();
}

#[test]
#[ignore = "requires an available audio output device"]
fn native_audio_loads_and_seeks_while_paused() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("test.wav");
    fixture(&path, 6);
    let (commands, receiver) = mpsc::channel();
    let (sender, events) = mpsc::channel();
    let service = EventSender {
        sender,
        context: egui::Context::default(),
    };
    let worker = thread::spawn(move || audio::run(receiver, service));

    commands.send(Effect::Load { path, volume: 0 }).unwrap();
    match events.recv_timeout(Duration::from_secs(10)).unwrap() {
        Event::Loaded(Ok(track)) => assert_eq!(track.duration, Some(Duration::from_secs(6))),
        Event::Loaded(Err(error)) => panic!("{error}"),
        _ => panic!("expected a loaded audio track"),
    }
    commands.send(Effect::SetPlaying(false)).unwrap();
    commands.send(Effect::Seek(Duration::from_secs(2))).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "playback did not report the target position"
        );
        match events.recv_timeout(remaining).unwrap() {
            Event::Playback {
                position,
                playing: false,
            } if position == Duration::from_secs(2) => break,
            Event::Error(error) => panic!("{error}"),
            _ => {}
        }
    }
    drop(commands);
    worker.join().unwrap();
}

#[test]
#[ignore = "requires FFmpeg and ffprobe on PATH"]
fn ffmpeg_exports_tagged_audio_and_preserves_existing_outputs() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.wav");
    let output = temp.path().join("Artist - Track.mp3");
    fixture(&input, 3);
    let (sender, events) = mpsc::channel();
    let service = EventSender {
        sender,
        context: egui::Context::default(),
    };
    let plan = || SplitPlan {
        input: input.clone(),
        copy_mp3: false,
        segments: vec![Segment {
            start: Duration::from_millis(500),
            length: Some(Duration::from_secs(1)),
            artist: "Artist".into(),
            title: "Track".into(),
            output: output.clone(),
        }],
    };
    files::split(plan(), &service);
    assert!(matches!(
        events.recv().unwrap(),
        Event::SplitProgress {
            completed: 1,
            total: 1
        }
    ));
    match events.recv().unwrap() {
        Event::SplitFinished(Ok(1)) => {}
        Event::SplitFinished(Err(error)) => panic!("{error}"),
        _ => panic!("expected one completed export"),
    }
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:format_tags=artist,title",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(probe.status.success());
    let info = String::from_utf8(probe.stdout).unwrap();
    assert!(info.contains("TAG:artist=Artist"));
    assert!(info.contains("TAG:title=Track"));
    let duration: f32 = info
        .lines()
        .find_map(|line| line.strip_prefix("duration="))
        .unwrap()
        .parse()
        .unwrap();
    assert!((1.0..1.1).contains(&duration));

    let bytes = fs::read(&output).unwrap();
    files::split(plan(), &service);
    assert!(matches!(
        events.recv().unwrap(),
        Event::SplitProgress { .. }
    ));
    assert!(
        matches!(events.recv().unwrap(), Event::SplitFinished(Err(error)) if error.contains("already exists"))
    );
    assert_eq!(fs::read(&output).unwrap(), bytes);

    let copied = temp.path().join("Artist - Copy.mp3");
    files::split(
        SplitPlan {
            input: output,
            copy_mp3: true,
            segments: vec![Segment {
                start: Duration::ZERO,
                length: None,
                artist: "Artist".into(),
                title: "Copy".into(),
                output: copied.clone(),
            }],
        },
        &service,
    );
    assert!(matches!(
        events.recv().unwrap(),
        Event::SplitProgress { .. }
    ));
    assert!(matches!(
        events.recv().unwrap(),
        Event::SplitFinished(Ok(1))
    ));
    assert!(copied.metadata().unwrap().len() > 0);
}

#[test]
fn cue_save_writes_utf8_and_replaces_only_on_request() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("markers.cue");
    files::save_cue(&path, "TITLE Café", false).unwrap();
    assert!(files::save_cue(&path, "other", false).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "TITLE Café");
    files::save_cue(&path, "replacement", true).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "replacement");
}
