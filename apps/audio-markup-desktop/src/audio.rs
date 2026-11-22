use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, RecvTimeoutError},
    time::Duration,
};

use audio_markup_engine::{Effect, Event, Result, Track};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

use crate::EventSender;

const POSITION_INTERVAL: Duration = Duration::from_millis(40);

pub fn run(commands: Receiver<Effect>, events: EventSender) {
    let mut audio = Audio::default();
    loop {
        if audio.is_playing() {
            match commands.recv_timeout(POSITION_INTERVAL) {
                Ok(command) => audio.apply(command, &events),
                Err(RecvTimeoutError::Timeout) => audio.tick(&events),
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match commands.recv() {
                Ok(command) => audio.apply(command, &events),
                Err(_) => break,
            }
        }
    }
}

#[derive(Default)]
struct Audio {
    active: Option<PlayingFile>,
    stream: Option<OutputStream>,
}

struct PlayingFile {
    path: PathBuf,
    sink: Sink,
    offset: Duration,
    duration: Option<Duration>,
}

impl Audio {
    fn apply(&mut self, command: Effect, events: &EventSender) {
        match command {
            Effect::Load { path, volume } => events.send(Event::Loaded(self.load(path, volume))),
            Effect::Seek(position) => match self.seek(position) {
                Ok(()) => self.publish(events),
                Err(error) => events.send(Event::Error(error)),
            },
            Effect::SeekBy(seconds) => {
                if let Some(active) = &self.active {
                    let current = active.position();
                    let step = Duration::from_secs(seconds.unsigned_abs());
                    let target = if seconds < 0 {
                        current.saturating_sub(step)
                    } else {
                        current.saturating_add(step)
                    };
                    self.apply(Effect::Seek(target), events);
                }
            }
            Effect::SetPlaying(playing) => {
                if let Some(active) = &self.active {
                    if playing {
                        active.sink.play();
                    } else {
                        active.sink.pause();
                    }
                    self.publish(events);
                }
            }
            Effect::SetVolume(volume) => {
                if let Some(active) = &self.active {
                    active.sink.set_volume(f32::from(volume) / 100.0);
                }
            }
            Effect::Split(_) => {}
        }
    }

    fn load(&mut self, path: PathBuf, volume: u8) -> Result<Track> {
        let source = decoder(&path)?;
        let duration = source.total_duration();
        if self.stream.is_none() {
            let stream = OutputStreamBuilder::open_default_stream()
                .map_err(|error| format!("Could not open an audio output device: {error}"))?;
            self.stream = Some(stream);
        }
        let sink = self.sink()?;
        sink.pause();
        sink.set_volume(f32::from(volume) / 100.0);
        sink.append(source);
        self.active = Some(PlayingFile {
            path: path.clone(),
            sink,
            offset: Duration::ZERO,
            duration,
        });
        if let Some(active) = &self.active {
            active.sink.play();
        }
        Ok(Track { path, duration })
    }

    fn seek(&mut self, position: Duration) -> Result<()> {
        let mut replacement = self.sink()?;
        let active = self.active.as_mut().ok_or("Open an audio file first.")?;
        let position = active.duration.map_or(position, |duration| {
            position.min(duration.saturating_sub(Duration::from_millis(1)))
        });
        let mut source = decoder(&active.path)?;
        if !position.is_zero() {
            source
                .try_seek(position)
                .map_err(|error| format!("Could not seek: {error}"))?;
        }
        let paused = active.sink.is_paused();
        replacement.pause();
        replacement.set_volume(active.sink.volume());
        replacement.append(source);
        std::mem::swap(&mut active.sink, &mut replacement);
        drop(replacement);
        active.offset = position;
        if !paused {
            active.sink.play();
        }
        Ok(())
    }

    fn tick(&mut self, events: &EventSender) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.sink.empty())
            && let Err(error) = self.seek(Duration::ZERO)
        {
            if let Some(active) = &self.active {
                active.sink.pause();
            }
            events.send(Event::Error(error));
        }
        self.publish(events);
    }

    fn sink(&self) -> Result<Sink> {
        self.stream
            .as_ref()
            .map(|stream| Sink::connect_new(stream.mixer()))
            .ok_or_else(|| "Audio output is unavailable.".into())
    }

    fn is_playing(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| !active.sink.is_paused())
    }

    fn publish(&self, events: &EventSender) {
        if let Some(active) = &self.active {
            events.send(Event::Playback {
                position: active.position(),
                playing: !active.sink.is_paused(),
            });
        }
    }
}

impl PlayingFile {
    fn position(&self) -> Duration {
        self.offset.saturating_add(self.sink.get_pos())
    }
}

fn decoder(path: &Path) -> Result<Decoder<BufReader<File>>> {
    let file =
        File::open(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    Decoder::try_from(file).map_err(|error| format!("Could not decode {}: {error}", path.display()))
}
