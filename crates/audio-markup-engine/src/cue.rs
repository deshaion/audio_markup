use std::{
    fmt::Write,
    path::{Path, PathBuf},
};

use crate::{
    Marker, MarkerData, Result, Track, parse_marker_time,
    time::{cue_frames, format_cue_frames},
};

pub struct CueSheet {
    pub audio_file: String,
    pub markers: Vec<MarkerData>,
}

impl CueSheet {
    pub fn parse(text: &str) -> Result<Self> {
        let mut audio_file = None;
        let mut album_artist = String::new();
        let mut tracks = Vec::new();
        let mut track: Option<CueTrack> = None;

        for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let (key, value) = field(line);
            let parsed = (|| -> Result<()> {
                match key.to_ascii_uppercase().as_str() {
                    "FILE" => {
                        if audio_file.is_some() {
                            return Err("Only single-file CUE sheets are supported.".into());
                        }
                        let (name, kind) = file_field(value)?;
                        if !matches!(
                            kind.to_ascii_uppercase().as_str(),
                            "MP3" | "WAVE" | "AIFF" | "FLAC" | "OGG" | "APE"
                        ) {
                            return Err(format!("Unsupported CUE audio file type: {kind}."));
                        }
                        audio_file = Some(name);
                    }
                    "TRACK" => {
                        if audio_file.is_none() {
                            return Err("TRACK must follow a FILE directive.".into());
                        }
                        if let Some(track) = track.take() {
                            tracks.push(track.finish(&album_artist)?);
                        }
                        let (number, kind) = field(value);
                        if !kind.eq_ignore_ascii_case("AUDIO") {
                            return Err("Only audio tracks are supported.".into());
                        }
                        let number = number
                            .parse::<u8>()
                            .ok()
                            .filter(|number| (1..=99).contains(number))
                            .ok_or("Track number must be between 01 and 99.")?;
                        track = Some(CueTrack {
                            number,
                            ..Default::default()
                        });
                    }
                    "PERFORMER" => {
                        let artist = metadata(value);
                        if let Some(track) = &mut track {
                            track.artist = Some(artist);
                        } else {
                            album_artist = artist;
                        }
                    }
                    "TITLE" => {
                        if let Some(track) = &mut track {
                            track.data.title = metadata(value);
                        }
                    }
                    "INDEX" => {
                        let track = track.as_mut().ok_or("INDEX must be inside a track.")?;
                        let (number, time) = field(value);
                        if matches!(number, "01" | "1") {
                            if track.data.position.is_some() || time.split(':').count() != 3 {
                                return Err("Expected one INDEX 01 in MM:SS:FF per track.".into());
                            }
                            track.data.position = Some(parse_marker_time(time)?);
                        }
                    }
                    _ => {}
                }
                Ok(())
            })();
            parsed.map_err(|error| format!("CUE line {}: {error}", index + 1))?;
        }
        if let Some(track) = track {
            tracks.push(track.finish(&album_artist)?);
        }
        if tracks.is_empty() {
            return Err("The CUE sheet has no audio tracks.".into());
        }
        if tracks
            .windows(2)
            .any(|rows| rows[0].position >= rows[1].position)
        {
            return Err("CUE track starts must be in ascending order.".into());
        }
        Ok(Self {
            audio_file: audio_file.ok_or("The CUE sheet has no FILE directive.")?,
            markers: tracks,
        })
    }

    pub fn resolve_audio(&self, cue_file: &Path) -> PathBuf {
        let portable = self.audio_file.replace('\\', "/");
        let parent = cue_file.parent().unwrap_or_else(|| Path::new(""));
        let original = parent.join(&self.audio_file);
        if original.is_file() {
            original
        } else {
            parent.join(portable)
        }
    }

    pub fn encode(track: &Track, markers: &[Marker], cue_file: &Path) -> Result<String> {
        let parent = cue_file.parent();
        let audio = if parent == track.path.parent() {
            track.path.file_name().and_then(|name| name.to_str())
        } else {
            track.path.to_str()
        }
        .ok_or("The audio path cannot be written as UTF-8.")?;
        if audio.contains(['"', '\r', '\n']) {
            return Err("The audio path contains a character a CUE FILE entry cannot use.".into());
        }
        let extension = track
            .path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();
        let kind = match extension.to_ascii_lowercase().as_str() {
            "mp3" => "MP3",
            "aif" | "aiff" | "aifc" => "AIFF",
            _ => "WAVE",
        };
        let mut text = format!("FILE \"{audio}\" {kind}\r\n");
        let mut previous = None;
        let mut number = 0;
        for (index, marker) in markers.iter().enumerate() {
            let data = &marker.data;
            let Some(position) = data.position else {
                if data.artist.is_empty() && data.title.is_empty() {
                    continue;
                }
                return Err(format!(
                    "Row {} needs a position for CUE export.",
                    index + 1
                ));
            };
            let frames = cue_frames(position);
            if previous.is_some_and(|previous| previous >= frames) {
                return Err(format!(
                    "Row {} must start at least one CUE frame after the previous row.",
                    index + 1
                ));
            }
            if track.duration.is_some_and(|duration| position >= duration) {
                return Err(format!("Row {} starts outside the audio file.", index + 1));
            }
            previous = Some(frames);
            number += 1;
            if number > 99 {
                return Err("A CUE sheet can contain at most 99 tracks.".into());
            }
            let title = quote_metadata(&data.title);
            let performer = quote_metadata(&data.artist);
            let time = format_cue_frames(frames);
            write!(text, "  TRACK {number:02} AUDIO\r\n    TITLE {title}\r\n    PERFORMER {performer}\r\n    INDEX 01 {time}\r\n")
                .expect("writing to a String cannot fail");
        }
        if number == 0 {
            return Err("Add at least one timestamped marker before exporting a CUE sheet.".into());
        }
        Ok(text)
    }
}

#[derive(Default)]
struct CueTrack {
    number: u8,
    artist: Option<String>,
    data: MarkerData,
}

impl CueTrack {
    fn finish(self, album_artist: &str) -> Result<MarkerData> {
        if self.data.position.is_none() {
            return Err(format!("Track {:02} has no INDEX 01.", self.number));
        }
        Ok(MarkerData {
            artist: self.artist.unwrap_or_else(|| album_artist.to_owned()),
            ..self.data
        })
    }
}

fn field(text: &str) -> (&str, &str) {
    let text = text.trim();
    text.split_once(char::is_whitespace)
        .map_or((text, ""), |(key, value)| (key, value.trim()))
}

fn file_field(value: &str) -> Result<(String, &str)> {
    let (name, kind) = if let Some(quoted) = value.strip_prefix('"') {
        quoted
            .rsplit_once('"')
            .ok_or("Expected a closing quote in FILE.")?
    } else {
        field(value)
    };
    if name.is_empty() || kind.trim().is_empty() {
        return Err("FILE needs an audio filename and type.".into());
    }
    Ok((name.to_owned(), kind.trim()))
}

fn metadata(value: &str) -> String {
    let inner = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'));
    inner.map_or_else(
        || value.to_owned(),
        |value| {
            let mut result = String::new();
            let mut chars = value.chars().peekable();
            while let Some(character) = chars.next() {
                if character == '\\' && matches!(chars.peek(), Some('"' | '\\')) {
                    result.extend(chars.next());
                } else {
                    result.push(character);
                }
            }
            result
        },
    )
}

fn quote_metadata(value: &str) -> String {
    let value = value
        .replace(['\r', '\n'], " ")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{value}\"")
}
