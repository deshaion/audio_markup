use std::{io::Write, path::Path, process::Command};

use audio_markup_engine::{Event, Result, Segment, SplitPlan, ffmpeg_time};
use tempfile::Builder;

use crate::EventSender;

pub fn split(plan: SplitPlan, events: &EventSender) {
    let total = plan.segments.len();
    let mut errors = Vec::new();
    for (index, segment) in plan.segments.iter().enumerate() {
        if let Err(error) = write_segment(&plan, segment) {
            errors.push(format!("{}: {error}", segment.output.display()));
        }
        events.send(Event::SplitProgress {
            completed: index + 1,
            total,
        });
    }
    let result = if errors.is_empty() {
        Ok(total)
    } else {
        Err(format!(
            "Exported {} of {total} segments.\n\n{}",
            total - errors.len(),
            errors.join("\n\n")
        ))
    };
    events.send(Event::SplitFinished(result));
}

fn write_segment(plan: &SplitPlan, segment: &Segment) -> Result<()> {
    if segment.output.exists() {
        return Err("An output file with this name already exists.".into());
    }
    let directory = segment.output.parent().ok_or("Missing output directory.")?;
    let temp = Builder::new()
        .prefix(".audio-markup-")
        .suffix(".mp3")
        .tempfile_in(directory)
        .map_err(|error| error.to_string())?;
    let mut command = Command::new("ffmpeg");
    command
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&plan.input)
        .args(["-ss", &ffmpeg_time(segment.start)]);
    if let Some(length) = segment.length {
        command.args(["-t", &ffmpeg_time(length)]);
    }
    command
        .args(["-map", "0:a:0", "-map_metadata", "-1", "-metadata"])
        .arg(format!("artist={}", segment.artist))
        .arg("-metadata")
        .arg(format!("title={}", segment.title));
    if plan.copy_mp3 {
        command.args(["-c:a", "copy"]);
    } else {
        command.args(["-c:a", "libmp3lame", "-q:a", "2"]);
    }
    let output = command
        .arg(temp.path())
        .output()
        .map_err(|error| format!("Could not run FFmpeg: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "FFmpeg failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    temp.persist_noclobber(&segment.output)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

pub fn save_cue(path: &Path, contents: &str, replace: bool) -> Result<()> {
    let directory = path.parent().ok_or("Missing CUE output directory.")?;
    let mut temp = Builder::new()
        .prefix(".audio-markup-")
        .tempfile_in(directory)
        .map_err(|error| error.to_string())?;
    temp.write_all(contents.as_bytes())
        .map_err(|error| error.to_string())?;
    let result = if replace {
        temp.persist(path)
    } else {
        temp.persist_noclobber(path)
    };
    result.map_err(|error| error.error.to_string())?;
    Ok(())
}
