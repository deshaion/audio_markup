mod cue;
mod split;
mod time;

pub use cue::CueSheet;
pub use split::{Segment, SplitPlan};
pub use time::{ffmpeg_time, format_marker_time, parse_marker_time};

use std::{path::PathBuf, time::Duration};

pub type MarkerId = u64;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkerData {
    pub position: Option<Duration>,
    pub artist: String,
    pub title: String,
}

pub struct Marker {
    pub id: MarkerId,
    pub data: MarkerData,
}

pub struct Track {
    pub path: PathBuf,
    pub duration: Option<Duration>,
}

pub enum Command {
    Open {
        path: PathBuf,
        markers: Option<Vec<MarkerData>>,
    },
    SetPlaying(bool),
    SeekBy(i64),
    SeekToMarker(MarkerId),
    SetVolume(u8),
    AddMarker,
    DeleteMarker(MarkerId),
    SetPosition(MarkerId, Option<Duration>),
    CapturePosition(MarkerId),
    SetArtist(MarkerId, String, bool),
    SetTitle(MarkerId, String),
    Split(PathBuf),
}

pub enum Effect {
    Load { path: PathBuf, volume: u8 },
    SetPlaying(bool),
    SeekBy(i64),
    Seek(Duration),
    SetVolume(u8),
    Split(SplitPlan),
}

pub enum Event {
    Loaded(Result<Track>),
    Playback { position: Duration, playing: bool },
    SplitProgress { completed: usize, total: usize },
    SplitFinished(Result<usize>),
    Error(String),
}

pub struct Session {
    markers: Vec<Marker>,
    next_id: MarkerId,
    track: Option<Track>,
    position: Duration,
    playing: bool,
    volume: u8,
    opening: bool,
    pending_markers: Option<Vec<MarkerData>>,
    split_progress: Option<(usize, usize)>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            markers: vec![Marker {
                id: 0,
                data: MarkerData {
                    position: Some(Duration::ZERO),
                    ..Default::default()
                },
            }],
            next_id: 1,
            track: None,
            position: Duration::ZERO,
            playing: false,
            volume: 100,
            opening: false,
            pending_markers: None,
            split_progress: None,
        }
    }
}

impl Session {
    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }
    pub fn track(&self) -> Option<&Track> {
        self.track.as_ref()
    }
    pub fn position(&self) -> Duration {
        self.position
    }
    pub fn playing(&self) -> bool {
        self.playing
    }
    pub fn volume(&self) -> u8 {
        self.volume
    }
    pub fn opening(&self) -> bool {
        self.opening
    }
    pub fn split_progress(&self) -> Option<(usize, usize)> {
        self.split_progress
    }

    pub fn execute(&mut self, command: Command) -> Result<Option<Effect>> {
        let effect = match command {
            Command::Open { path, markers } => {
                if self.opening {
                    return Err("An audio file is already opening.".into());
                }
                self.opening = true;
                self.pending_markers = markers;
                Effect::Load {
                    path,
                    volume: self.volume,
                }
            }
            Command::SetPlaying(playing) => {
                self.ready_track()?;
                Effect::SetPlaying(playing)
            }
            Command::SeekBy(seconds) => {
                self.ready_track()?;
                Effect::SeekBy(seconds)
            }
            Command::SeekToMarker(id) => {
                let position = self
                    .marker(id)?
                    .position
                    .ok_or("Enter a marker position first.")?;
                self.seek(position)?
            }
            Command::SetVolume(volume) => {
                self.volume = volume.min(100);
                Effect::SetVolume(self.volume)
            }
            Command::AddMarker => {
                self.add_marker(MarkerData::default());
                return Ok(None);
            }
            Command::DeleteMarker(id) => {
                self.markers.retain(|marker| marker.id != id);
                return Ok(None);
            }
            Command::SetPosition(id, position) => {
                self.marker(id)?.position = position;
                return Ok(None);
            }
            Command::CapturePosition(id) => {
                self.ready_track()?;
                let position = Duration::from_secs(self.position.as_secs());
                self.marker(id)?.position = Some(position);
                return Ok(None);
            }
            Command::SetArtist(id, value, split_pair) => {
                let marker = self.marker(id)?;
                if let Some((artist, title)) = split_pair
                    .then(|| value.split_once(" - ").or_else(|| value.split_once('-')))
                    .flatten()
                {
                    marker.artist = artist.trim().to_owned();
                    marker.title = title.trim().to_owned();
                } else {
                    marker.artist = value;
                }
                return Ok(None);
            }
            Command::SetTitle(id, title) => {
                self.marker(id)?.title = title;
                return Ok(None);
            }
            Command::Split(directory) => {
                if self.split_progress.is_some() {
                    return Err("A split is already running.".into());
                }
                let plan = SplitPlan::new(self.ready_track()?, &self.markers, &directory)?;
                self.split_progress = Some((0, plan.segments.len()));
                Effect::Split(plan)
            }
        };
        Ok(Some(effect))
    }

    pub fn apply(&mut self, event: Event) -> Result<Option<usize>> {
        match event {
            Event::Loaded(track) => {
                self.opening = false;
                let markers = self.pending_markers.take();
                self.track = Some(track?);
                self.position = Duration::ZERO;
                self.playing = true;
                if let Some(markers) = markers {
                    self.markers.clear();
                    for marker in markers {
                        self.add_marker(marker);
                    }
                }
            }
            Event::Playback { position, playing } => {
                self.position = position;
                self.playing = playing;
            }
            Event::SplitProgress { completed, total } => {
                self.split_progress = Some((completed, total))
            }
            Event::SplitFinished(result) => {
                self.split_progress = None;
                return result.map(Some);
            }
            Event::Error(message) => return Err(message),
        }
        Ok(None)
    }

    fn seek(&self, position: Duration) -> Result<Effect> {
        let track = self.ready_track()?;
        // Stay just inside the file at EOF so the audio backend can seek while paused.
        let position = match track.duration {
            Some(duration) => position.min(duration.saturating_sub(Duration::from_millis(1))),
            None => position,
        };
        Ok(Effect::Seek(position))
    }

    fn ready_track(&self) -> Result<&Track> {
        if self.opening {
            return Err("Wait for the audio file to open.".into());
        }
        self.track
            .as_ref()
            .ok_or_else(|| "Open an audio file first.".into())
    }

    fn add_marker(&mut self, data: MarkerData) {
        self.markers.push(Marker {
            id: self.next_id,
            data,
        });
        self.next_id += 1;
    }

    fn marker(&mut self, id: MarkerId) -> Result<&mut MarkerData> {
        self.markers
            .iter_mut()
            .find(|marker| marker.id == id)
            .map(|marker| &mut marker.data)
            .ok_or_else(|| "Select a marker row first.".into())
    }
}
