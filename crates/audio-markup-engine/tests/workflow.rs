use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use audio_markup_engine::{
    Command, CueSheet, Effect, Event, MarkerData, Session, SplitPlan, Track, format_marker_time,
    parse_marker_time,
};

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

fn track() -> Track {
    Track {
        path: PathBuf::from("/audio/Live Set.mp3"),
        duration: Some(seconds(300)),
    }
}

fn load(session: &mut Session, markers: Option<Vec<MarkerData>>) {
    let path = track().path;
    assert!(matches!(
        session.execute(Command::Open { path, markers }).unwrap(),
        Some(Effect::Load { .. })
    ));
    session.apply(Event::Loaded(Ok(track()))).unwrap();
}

#[test]
fn marker_workflow_preserves_ids_and_routes_transport_commands() {
    let mut session = Session::default();
    let first = session.markers()[0].id;
    load(&mut session, None);
    session.execute(Command::AddMarker).unwrap();
    let second = session.markers()[1].id;
    session
        .apply(Event::Playback {
            position: Duration::from_millis(13_500),
            playing: true,
        })
        .unwrap();
    session.execute(Command::CapturePosition(second)).unwrap();
    session
        .execute(Command::SetArtist(
            second,
            "Performer - Live - Encore".into(),
            true,
        ))
        .unwrap();
    session.execute(Command::DeleteMarker(first)).unwrap();
    let marker = &session.markers()[0];
    assert_eq!(marker.id, second);
    assert_eq!(marker.data.position, Some(seconds(13)));
    assert_eq!(marker.data.artist, "Performer");
    assert_eq!(marker.data.title, "Live - Encore");
    assert!(
        matches!(session.execute(Command::SeekToMarker(second)).unwrap(), Some(Effect::Seek(position)) if position == seconds(13))
    );
    assert!(matches!(
        session.execute(Command::SeekBy(-10)).unwrap(),
        Some(Effect::SeekBy(-10))
    ));
    assert!(matches!(
        session.execute(Command::SetPlaying(false)).unwrap(),
        Some(Effect::SetPlaying(false))
    ));
    load(&mut session, None);
    assert_eq!(session.markers()[0].id, second);
}

#[test]
fn unsuccessful_cue_load_preserves_the_current_track_and_markers() {
    let mut session = Session::default();
    load(&mut session, None);
    let first = session.markers()[0].id;
    let markers = vec![MarkerData {
        position: Some(seconds(12)),
        title: "Imported".into(),
        ..Default::default()
    }];
    session
        .execute(Command::Open {
            path: PathBuf::from("missing.wav"),
            markers: Some(markers.clone()),
        })
        .unwrap();
    assert!(
        session
            .apply(Event::Loaded(Err("Missing file".into())))
            .is_err()
    );
    assert_eq!(session.track().unwrap().path, track().path);
    assert_eq!(session.markers()[0].id, first);
    load(&mut session, Some(markers.clone()));
    assert_ne!(session.markers()[0].id, first);
    assert_eq!(session.markers()[0].data, markers[0]);
}

#[test]
fn cue_round_trip_retains_frames_and_track_metadata() {
    let cue = CueSheet::parse("\u{feff}PERFORMER \"Album Artist\"\r\nTITLE \"Album\"\r\nFILE \"Live Set.mp3\" MP3\r\n  TRACK 01 AUDIO\r\n    TITLE \"Café\"\r\n    INDEX 01 00:00:00\r\n  TRACK 02 AUDIO\r\n    PERFORMER \"Guest\"\r\n    TITLE \"The \\\"second\\\" song\"\r\n    INDEX 00 01:04:00\r\n    INDEX 01 01:05:37\r\n").unwrap();
    assert_eq!(cue.markers[0].artist, "Album Artist");
    assert_eq!(cue.markers[1].title, "The \"second\" song");
    assert_eq!(
        format_marker_time(cue.markers[1].position.unwrap()),
        "01:05:37"
    );

    let mut session = Session::default();
    let expected = cue.markers.clone();
    load(&mut session, Some(cue.markers));
    let encoded = CueSheet::encode(
        session.track().unwrap(),
        session.markers(),
        Path::new("/audio/Live Set.cue"),
    )
    .unwrap();
    assert!(encoded.starts_with("FILE \"Live Set.mp3\" MP3"));
    assert_eq!(CueSheet::parse(&encoded).unwrap().markers, expected);
}

#[test]
fn rejects_invalid_or_unrepresentable_cue_input() {
    let header = "FILE test.wav WAVE\nTRACK 01 AUDIO\n";
    assert!(CueSheet::parse(&format!("{header}INDEX 00 00:00:00")).is_err());
    assert!(CueSheet::parse(&format!("{header}INDEX 01 00:01:75")).is_err());
    assert!(CueSheet::parse(&format!("{header}INDEX 01 00:00:00\nFILE another.wav WAVE")).is_err());
    assert!(parse_marker_time("01:60").is_err());
    assert_eq!(
        parse_marker_time("01:03.125").unwrap(),
        Duration::from_millis(63_125)
    );
}

#[test]
fn blank_metadata_still_marks_the_previous_cut_and_split_uses_a_snapshot() {
    let mut session = Session::default();
    let markers = vec![
        MarkerData {
            position: Some(seconds(5)),
            artist: "A/B".into(),
            title: "First".into(),
        },
        MarkerData {
            position: Some(seconds(30)),
            ..Default::default()
        },
        MarkerData {
            position: Some(seconds(40)),
            title: "Last".into(),
            ..Default::default()
        },
    ];
    load(&mut session, Some(markers));
    let Some(Effect::Split(plan)) = session
        .execute(Command::Split(PathBuf::from("/export")))
        .unwrap()
    else {
        panic!("expected split plan")
    };
    assert!(plan.copy_mp3);
    assert_eq!(plan.segments.len(), 2);
    assert_eq!(plan.segments[0].length, Some(seconds(25)));
    assert_eq!(
        plan.segments[0].output,
        Path::new("/export/A_B - First.mp3")
    );
    assert_eq!(plan.segments[1].length, None);
    assert!(
        session
            .execute(Command::Split(PathBuf::from("/export")))
            .is_err()
    );
    let first = session.markers()[0].id;
    session.execute(Command::DeleteMarker(first)).unwrap();
    assert_eq!(plan.segments[0].title, "First");
    assert_eq!(session.apply(Event::SplitFinished(Ok(2))).unwrap(), Some(2));
    assert_eq!(session.split_progress(), None);
}

#[test]
fn split_reports_bad_boundaries_and_duplicate_output_names() {
    let mut session = Session::default();
    let mut markers = vec![
        MarkerData {
            position: Some(seconds(10)),
            title: "Same".into(),
            ..Default::default()
        },
        MarkerData {
            position: Some(seconds(12)),
            title: "SAME".into(),
            ..Default::default()
        },
    ];
    load(&mut session, Some(markers.clone()));
    assert!(SplitPlan::new(&track(), session.markers(), Path::new("out")).is_err());
    markers[1].position = None;
    markers[1].title.clear();
    load(&mut session, Some(markers));
    assert!(
        matches!(SplitPlan::new(&track(), session.markers(), Path::new("out")), Err(message) if message.contains("Row 2"))
    );
}
