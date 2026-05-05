use std::sync::Arc;

pub struct PlaybackState {
    sound_id: Option<String>,
    samples: Option<Arc<Vec<f32>>>,
    cursor: usize,
    volume: f32,
    sample_rate: u32,
    channels: u16,
    active: bool,
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            sound_id: None,
            samples: None,
            cursor: 0,
            volume: 1.0,
            sample_rate: 48000,
            channels: 2,
            active: false,
        }
    }

    pub fn start(
        &mut self,
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
    ) {
        self.sound_id = Some(sound_id);
        self.samples = Some(samples);
        self.cursor = 0;
        self.sample_rate = sample_rate;
        self.channels = channels;
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
        self.channels
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn fill_buffer(&mut self, buf: &mut [f32]) -> usize {
        let samples = match &self.samples {
            Some(s) if self.active => s,
            _ => return 0,
        };

        let remaining = samples.len().saturating_sub(self.cursor);
        let to_write = buf.len().min(remaining);

        if to_write == 0 {
            self.active = false;
            return 0;
        }

        let src = &samples[self.cursor..self.cursor + to_write];
        for (dst, &sample) in buf[..to_write].iter_mut().zip(src.iter()) {
            *dst = sample * self.volume;
        }

        self.cursor += to_write;

        if self.cursor >= samples.len() {
            self.active = false;
        }

        to_write
    }
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::new()
    }
}
