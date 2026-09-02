//! Music and sound effect playback. Audio is *presentation*: it never feeds back into the
//! simulation, so it can be absent (headless, no device) without changing any hash.
//!
//! The game's music files are Ogg Vorbis streams with a `.wav` extension; effects are PCM WAVE
//! (`docs/formats/sound.md`). Bytes are sniffed, not trusted by extension.

use std::io::Cursor;

use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{Decoder, Player, Source};
use thiserror::Error;

/// Audio errors.
#[derive(Debug, Error)]
pub enum AudioError {
    /// No output device could be opened.
    #[error("no audio output: {0}")]
    NoOutput(String),
    /// The bytes are not a supported container.
    #[error("unsupported audio data ({0})")]
    Unsupported(String),
}

/// Container kinds recognised by [`sniff`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// Ogg (Vorbis) stream.
    Ogg,
    /// RIFF WAVE.
    Wave,
    /// Unknown.
    Unknown,
}

/// Identify an audio container from its first bytes.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Container {
    if bytes.starts_with(b"OggS") {
        Container::Ogg
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        Container::Wave
    } else {
        Container::Unknown
    }
}

/// The audio device with one music channel and fire-and-forget effects.
pub struct Audio {
    stream: MixerDeviceSink,
    music: Player,
    effects: Vec<Player>,
    music_volume: f32,
    effects_volume: f32,
}

impl std::fmt::Debug for Audio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Audio")
            .field("music_playing", &!self.music.empty())
            .field("effects", &self.effects.len())
            .finish_non_exhaustive()
    }
}

impl Audio {
    /// Open the default output device.
    pub fn open() -> Result<Self, AudioError> {
        let mut stream = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| AudioError::NoOutput(e.to_string()))?;
        stream.log_on_drop(false);
        let music = Player::connect_new(stream.mixer());
        Ok(Self {
            stream,
            music,
            effects: Vec::new(),
            music_volume: 1.0,
            effects_volume: 1.0,
        })
    }

    /// Replace the music track (looping when `looped`).
    pub fn play_music(&mut self, bytes: Vec<u8>, looped: bool) -> Result<(), AudioError> {
        if sniff(&bytes) == Container::Unknown {
            return Err(AudioError::Unsupported("not Ogg or WAVE".into()));
        }
        let decoder =
            Decoder::new(Cursor::new(bytes)).map_err(|e| AudioError::Unsupported(e.to_string()))?;
        self.music.stop();
        self.music = Player::connect_new(self.stream.mixer());
        self.music.set_volume(self.music_volume);
        if looped {
            self.music.append(decoder.repeat_infinite());
        } else {
            self.music.append(decoder);
        }
        Ok(())
    }

    /// Stop the music.
    pub fn stop_music(&mut self) {
        self.music.stop();
    }

    /// Play a one-shot effect.
    pub fn play_effect(&mut self, bytes: Vec<u8>) -> Result<(), AudioError> {
        if sniff(&bytes) == Container::Unknown {
            return Err(AudioError::Unsupported("not Ogg or WAVE".into()));
        }
        let decoder =
            Decoder::new(Cursor::new(bytes)).map_err(|e| AudioError::Unsupported(e.to_string()))?;
        self.effects.retain(|s| !s.empty());
        let sink = Player::connect_new(self.stream.mixer());
        sink.set_volume(self.effects_volume);
        sink.append(decoder);
        self.effects.push(sink);
        Ok(())
    }

    /// Music volume `0.0..=1.0`.
    pub fn set_music_volume(&mut self, v: f32) {
        self.music_volume = v.clamp(0.0, 1.0);
        self.music.set_volume(self.music_volume);
    }

    /// Effects volume `0.0..=1.0`.
    pub fn set_effects_volume(&mut self, v: f32) {
        self.effects_volume = v.clamp(0.0, 1.0);
        for s in &self.effects {
            s.set_volume(self.effects_volume);
        }
    }

    /// Whether music is playing.
    #[must_use]
    pub fn music_playing(&self) -> bool {
        !self.music.empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_containers() {
        assert_eq!(sniff(b"OggS\0\x02"), Container::Ogg);
        assert_eq!(sniff(b"RIFF\x10\0\0\0WAVEfmt "), Container::Wave);
        assert_eq!(sniff(b"RIFF\x10\0\0\0AVI "), Container::Unknown);
        assert_eq!(sniff(b""), Container::Unknown);
    }
}
