use crate::audio::processing::{ChannelLayout, SoundProcessing};
use std::sync::Arc;

/// Upper bound on the per-sound volume multiplier (boost), mirroring the
/// per-sound volume domain in `state::sound_meta` (0.0..=2.0). The pre-#151
/// path scaled samples by this factor uncapped, so a user's above-unity boost
/// must survive here too — clamping to 1.0 would silently quiet boosted sounds.
const MAX_PER_SOUND_GAIN: f32 = 2.0;

pub struct PlaybackState {
    sound_id: Option<String>,
    samples: Option<Arc<Vec<f32>>>,
    cursor: usize,
    volume: f32,
    gain: f32,
    sample_rate: u32,
    source_channels: u16,
    layout: ChannelLayout,
    active: bool,
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            sound_id: None,
            samples: None,
            cursor: 0,
            volume: 1.0,
            gain: 1.0,
            sample_rate: 48000,
            source_channels: 2,
            layout: ChannelLayout::new(2, SoundProcessing::default()),
            active: false,
        }
    }

    pub fn with_volume(volume: f32) -> Self {
        Self {
            volume: volume.clamp(0.0, 1.0),
            ..Self::new()
        }
    }

    // Six args (one over the `too-many-arguments-threshold = 5`) is the
    // canonical playback descriptor: identity, buffer, format (rate + channels),
    // and the per-sound gain. They are not separable into a meaningful sub-struct
    // here, and the engine + app both call `start` positionally with exactly
    // these (#151).
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        source_channels: u16,
        gain: f32,
    ) {
        self.sound_id = Some(sound_id);
        self.samples = Some(samples);
        self.cursor = 0;
        self.gain = gain.clamp(0.0, MAX_PER_SOUND_GAIN);
        self.sample_rate = sample_rate;
        self.source_channels = source_channels;
        self.layout = ChannelLayout::new(source_channels, SoundProcessing::default());
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.sound_id = None;
        self.samples = None;
        self.cursor = 0;
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn sound_id(&self) -> Option<&str> {
        self.sound_id.as_deref()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.layout.output_channels()
    }

    pub fn set_channel_processing(&mut self, settings: SoundProcessing) {
        self.layout = ChannelLayout::new(self.source_channels, settings);
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn progress(&self) -> f32 {
        match &self.samples {
            Some(s) if !s.is_empty() => self.cursor as f32 / s.len() as f32,
            _ => 0.0,
        }
    }

    pub fn fill_buffer(&mut self, buf: &mut [f32]) -> usize {
        let samples = match &self.samples {
            Some(s) if self.active => s,
            _ => return 0,
        };

        let (consumed, written) =
            self.layout
                .fill(&samples[self.cursor..], buf, self.volume * self.gain);
        self.cursor += consumed;
        if samples.len().saturating_sub(self.cursor) < self.source_channels.max(1) as usize {
            self.active = false;
        }
        written
    }
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
