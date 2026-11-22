use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{Marker, Result, Track};

pub struct SplitPlan {
    pub input: PathBuf,
    pub copy_mp3: bool,
    pub segments: Vec<Segment>,
}

pub struct Segment {
    pub start: Duration,
    pub length: Option<Duration>,
    pub artist: String,
    pub title: String,
    pub output: PathBuf,
}

impl SplitPlan {
    pub fn new(track: &Track, markers: &[Marker], directory: &Path) -> Result<Self> {
        let mut segments = Vec::new();
        let mut names = HashSet::new();
        let mut previous_end = None;
        for (index, marker) in markers.iter().enumerate() {
            let data = &marker.data;
            if data.artist.is_empty() && data.title.is_empty() {
                continue;
            }
            let start = data
                .position
                .ok_or_else(|| format!("Row {} needs a start position.", index + 1))?;
            let end = markers
                .get(index + 1)
                .map(|next| {
                    next.data.position.ok_or_else(|| {
                        format!(
                            "Row {} needs a position to end the previous track.",
                            index + 2
                        )
                    })
                })
                .transpose()?;
            if end.is_some_and(|end| end <= start) || previous_end.is_some_and(|end| end > start) {
                return Err(format!(
                    "Check the order of the positions around row {}.",
                    index + 1
                ));
            }
            if track
                .duration
                .is_some_and(|duration| start >= duration || end.is_some_and(|end| end > duration))
            {
                return Err(format!(
                    "Row {} defines a segment outside the audio file.",
                    index + 1
                ));
            }
            previous_end = end;
            let name = format!(
                "{} - {}.mp3",
                filename_part(&data.artist),
                filename_part(&data.title)
            );
            if !names.insert(name.to_lowercase()) {
                return Err(format!("More than one track would be saved as {name}."));
            }
            segments.push(Segment {
                start,
                length: end.map(|end| end - start),
                artist: data.artist.clone(),
                title: data.title.clone(),
                output: directory.join(name),
            });
        }
        if segments.is_empty() {
            return Err("Add an artist or title to at least one marker before splitting.".into());
        }
        Ok(Self {
            input: track.path.clone(),
            copy_mp3: track
                .path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3")),
            segments,
        })
    }
}

fn filename_part(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() || r#"<>:"/\|?*"#.contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .to_owned()
}
