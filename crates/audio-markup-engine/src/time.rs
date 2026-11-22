use std::time::Duration;

use crate::Result;

const FRAMES_PER_SECOND: u128 = 75;
const NANOS_PER_SECOND: u128 = 1_000_000_000;

pub fn parse_marker_time(text: &str) -> Result<Duration> {
    let invalid = || "Enter a position as MM:SS, MM:SS.mmm, or MM:SS:FF.".to_owned();
    let text = text.trim();
    let (minutes, rest) = text.split_once(':').ok_or_else(invalid)?;
    let minutes: u64 = minutes.parse().map_err(|_| invalid())?;
    let (seconds, nanos) = if let Some((seconds, frames)) = rest.split_once(':') {
        let frames: u128 = frames.parse().map_err(|_| invalid())?;
        if frames >= FRAMES_PER_SECOND {
            return Err("CUE frames must be between 00 and 74.".into());
        }
        (
            seconds,
            (frames * NANOS_PER_SECOND / FRAMES_PER_SECOND) as u32,
        )
    } else if let Some((seconds, fraction)) = rest.split_once('.') {
        if fraction.is_empty()
            || fraction.len() > 9
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(invalid());
        }
        let fraction = format!("{fraction:0<9}");
        (seconds, fraction.parse().map_err(|_| invalid())?)
    } else {
        (rest, 0)
    };
    let seconds: u64 = seconds.parse().map_err(|_| invalid())?;
    if seconds >= 60 {
        return Err("Seconds must be between 00 and 59.".into());
    }
    let total = minutes
        .checked_mul(60)
        .and_then(|value| value.checked_add(seconds))
        .ok_or_else(invalid)?;
    Ok(Duration::new(total, nanos))
}

pub fn format_marker_time(time: Duration) -> String {
    let seconds = time.as_secs();
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    let mut text = format!("{minutes:02}:{seconds:02}");
    if time.subsec_nanos() != 0 {
        let frames = cue_frames(time);
        if from_cue_frames(frames) == time {
            text.push_str(&format!(":{:02}", frames % 75));
        } else {
            let fraction = format!("{:09}", time.subsec_nanos());
            text.push('.');
            text.push_str(fraction.trim_end_matches('0'));
        }
    }
    text
}

pub(crate) fn cue_frames(time: Duration) -> u128 {
    (time.as_nanos() * FRAMES_PER_SECOND + NANOS_PER_SECOND / 2) / NANOS_PER_SECOND
}

fn from_cue_frames(frames: u128) -> Duration {
    Duration::new(
        (frames / FRAMES_PER_SECOND) as u64,
        ((frames % FRAMES_PER_SECOND) * NANOS_PER_SECOND / FRAMES_PER_SECOND) as u32,
    )
}

pub(crate) fn format_cue_frames(frames: u128) -> String {
    let seconds = frames / FRAMES_PER_SECOND;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 60,
        seconds % 60,
        frames % FRAMES_PER_SECOND
    )
}

pub fn ffmpeg_time(time: Duration) -> String {
    format!("{}.{:06}", time.as_secs(), time.subsec_micros())
}
