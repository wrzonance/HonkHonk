use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::audio::effects::{EffectChain, EffectSettings};
use crate::audio::playback::PlaybackState;
use crate::audio::processing::{self, Dynamics, DynamicsSettings, VoiceProcessing};

pub const MAX_VOICES: usize = 16;
const DEFAULT_MIX_SCRATCH: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedVoice {
    pub voice_id: u64,
    pub sound_id: String,
    pub generation: u64,
}

impl FinishedVoice {
    pub fn new(voice_id: u64, sound_id: impl Into<String>, generation: u64) -> Self {
        Self {
            voice_id,
            sound_id: sound_id.into(),
            generation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixTarget {
    Sink,
    Monitor,
}

pub struct MixScratch {
    dry: Vec<f32>,
    wet: Vec<f32>,
}

impl MixScratch {
    pub fn new(capacity: usize) -> Self {
        // Keep room for one complete frame of any supported PipeWire format.
        let capacity = capacity.max(pipewire::spa::param::audio::MAX_CHANNELS);
        Self {
            dry: vec![0.0; capacity],
            wet: vec![0.0; capacity],
        }
    }

    fn capacity(&self) -> usize {
        self.dry.len()
    }
}

impl Default for MixScratch {
    fn default() -> Self {
        Self::new(DEFAULT_MIX_SCRATCH)
    }
}

pub struct VoiceSpec {
    pub processing: VoiceProcessing,
    pub id: u64,
    pub sound_id: String,
    pub generation: u64,
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub channels: u16,
    pub gain: f32,
    pub master_volume: f32,
    pub effects: EffectSettings,
    pub monitor_enabled: bool,
}

pub struct Voice {
    processing: VoiceProcessing,
    sink_dynamics: Dynamics,
    monitor_dynamics: Dynamics,
    channels: u16,
    pub id: u64,
    pub sound_id: String,
    pub generation: u64,
    pub sink_state: Rc<RefCell<PlaybackState>>,
    pub monitor_state: Rc<RefCell<PlaybackState>>,
    sink_effects: EffectChain,
    monitor_effects: EffectChain,
    sink_tail_remaining: usize,
    monitor_tail_remaining: usize,
}

impl Voice {
    fn from_spec(spec: VoiceSpec) -> Self {
        let sink_state = Self::started_state(&spec);
        let monitor_state = if spec.monitor_enabled {
            Self::started_state(&spec)
        } else {
            Rc::new(RefCell::new(PlaybackState::with_volume(spec.master_volume)))
        };
        let sink_effects = spec
            .effects
            .build_chain(DEFAULT_MIX_SCRATCH, spec.sample_rate);
        let monitor_effects = spec
            .effects
            .build_chain(DEFAULT_MIX_SCRATCH, spec.sample_rate);
        let tail = sink_effects.total_latency_samples() as usize;
        let monitor_tail = if spec.monitor_enabled {
            monitor_effects.total_latency_samples() as usize
        } else {
            0
        };
        Self {
            id: spec.id,
            processing: spec.processing,
            sink_dynamics: Dynamics::default(),
            monitor_dynamics: Dynamics::default(),
            channels: processing::ChannelLayout::new(spec.channels, spec.processing.sound)
                .output_channels(),
            sound_id: spec.sound_id,
            generation: spec.generation,
            sink_state,
            monitor_state,
            sink_effects,
            monitor_effects,
            sink_tail_remaining: tail,
            monitor_tail_remaining: monitor_tail,
        }
    }

    fn started_state(spec: &VoiceSpec) -> Rc<RefCell<PlaybackState>> {
        let mut state = PlaybackState::with_volume(spec.master_volume);
        state.start(
            spec.sound_id.clone(),
            Arc::clone(&spec.samples),
            spec.sample_rate,
            spec.channels,
            spec.gain,
        );
        state.set_channel_processing(spec.processing.sound);
        Rc::new(RefCell::new(state))
    }

    fn finished(&self) -> FinishedVoice {
        FinishedVoice::new(self.id, self.sound_id.clone(), self.generation)
    }

    fn is_done(&self) -> bool {
        !self.sink_state.borrow().is_active()
            && !self.monitor_state.borrow().is_active()
            && self.sink_tail_remaining == 0
            && self.monitor_tail_remaining == 0
    }

    fn mix_into(
        &mut self,
        target: MixTarget,
        output: &mut [f32],
        scratch: &mut MixScratch,
        sample_rate: u32,
    ) {
        let n = output.len();
        let wrote = self.fill_target(target, n, scratch);
        if wrote == 0 && self.tail_remaining(target) == 0 {
            return;
        }
        self.process_target(target, n, scratch, sample_rate);
        self.mix_processed(output, &scratch.wet[..n]);
        if wrote < n {
            self.consume_tail(target, n - wrote);
        }
    }

    fn fill_target(&mut self, target: MixTarget, n: usize, scratch: &mut MixScratch) -> usize {
        scratch.dry[..n].fill(0.0);
        match target {
            MixTarget::Sink => self
                .sink_state
                .borrow_mut()
                .fill_buffer(&mut scratch.dry[..n]),
            MixTarget::Monitor => self
                .monitor_state
                .borrow_mut()
                .fill_buffer(&mut scratch.dry[..n]),
        }
    }

    fn process_target(
        &mut self,
        target: MixTarget,
        n: usize,
        scratch: &mut MixScratch,
        sample_rate: u32,
    ) {
        self.process_dry(target, &mut scratch.dry[..n], sample_rate);
        match target {
            MixTarget::Sink => {
                self.sink_effects
                    .process(&scratch.dry[..n], &mut scratch.wet[..n], sample_rate)
            }
            MixTarget::Monitor => {
                self.monitor_effects
                    .process(&scratch.dry[..n], &mut scratch.wet[..n], sample_rate)
            }
        }
    }

    fn process_dry(&mut self, target: MixTarget, samples: &mut [f32], rate: u32) {
        let gain = self.processing.normalization_gain;
        let gain = if gain.is_finite() {
            gain.clamp(0.0, 8.0)
        } else {
            1.0
        };
        for sample in samples.iter_mut() {
            *sample *= gain;
        }
        processing::pan(samples, self.channels, self.processing.sound.pan);
        let dynamics = match target {
            MixTarget::Sink => &mut self.sink_dynamics,
            MixTarget::Monitor => &mut self.monitor_dynamics,
        };
        dynamics.process(
            samples,
            (rate, self.channels),
            self.processing.sound.dynamics,
            false,
        );
    }

    fn mix_processed(&self, output: &mut [f32], processed: &[f32]) {
        for (dst, sample) in output.iter_mut().zip(processed.iter()) {
            *dst += *sample;
        }
    }

    fn tail_remaining(&self, target: MixTarget) -> usize {
        match target {
            MixTarget::Sink => self.sink_tail_remaining,
            MixTarget::Monitor => self.monitor_tail_remaining,
        }
    }

    fn consume_tail(&mut self, target: MixTarget, samples: usize) {
        match target {
            MixTarget::Sink => {
                self.sink_tail_remaining = self.sink_tail_remaining.saturating_sub(samples);
            }
            MixTarget::Monitor => {
                self.monitor_tail_remaining = self.monitor_tail_remaining.saturating_sub(samples);
            }
        }
    }

    fn set_master_volume(&self, volume: f32) {
        self.sink_state.borrow_mut().set_volume(volume);
        self.monitor_state.borrow_mut().set_volume(volume);
    }

    fn stop_monitor(&mut self) {
        self.monitor_state.borrow_mut().stop();
        self.monitor_tail_remaining = 0;
    }
}

pub struct VoicePool {
    dynamics: DynamicsSettings,
    sink_dynamics: Dynamics,
    monitor_dynamics: Dynamics,
    voices: Vec<Voice>,
    max_voices: usize,
}

impl VoicePool {
    pub fn new() -> Self {
        Self::with_max_voices(MAX_VOICES)
    }

    pub fn with_max_voices(max_voices: usize) -> Self {
        Self {
            voices: Vec::new(),
            dynamics: DynamicsSettings::default(),
            sink_dynamics: Dynamics::default(),
            monitor_dynamics: Dynamics::default(),
            max_voices: max_voices.max(1),
        }
    }

    pub fn push(&mut self, spec: VoiceSpec) -> Vec<FinishedVoice> {
        let mut finished = Vec::new();
        if self.voices.len() >= self.max_voices {
            finished.push(self.voices.remove(0).finished());
        }
        self.voices.push(Voice::from_spec(spec));
        finished
    }

    pub fn stop_voice(&mut self, voice_id: u64) -> Vec<FinishedVoice> {
        if let Some(index) = self.voices.iter().position(|voice| voice.id == voice_id) {
            return vec![self.voices.remove(index).finished()];
        }
        Vec::new()
    }

    pub fn stop_all(&mut self) -> Vec<FinishedVoice> {
        self.voices
            .drain(..)
            .map(|voice| voice.finished())
            .collect()
    }

    pub fn drain_finished(&mut self) -> Vec<FinishedVoice> {
        let mut finished = Vec::new();
        let mut i = 0;
        while i < self.voices.len() {
            if self.voices[i].is_done() {
                finished.push(self.voices.remove(i).finished());
            } else {
                i += 1;
            }
        }
        finished
    }

    pub fn mix(
        &mut self,
        target: MixTarget,
        output: &mut [f32],
        scratch: &mut MixScratch,
        sample_rate: u32,
    ) {
        let channels = self.voices.first().map_or(2, |v| v.channels).max(1);
        let capacity = (scratch.capacity() / channels as usize).max(1) * channels as usize;
        for chunk in output.chunks_mut(capacity.min(scratch.capacity())) {
            chunk.fill(0.0);
            for voice in &mut self.voices {
                voice.mix_into(target, chunk, scratch, sample_rate);
            }
            let dynamics = match target {
                MixTarget::Sink => &mut self.sink_dynamics,
                MixTarget::Monitor => &mut self.monitor_dynamics,
            };
            dynamics.process(chunk, (sample_rate, channels), self.dynamics, true);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.voices.is_empty()
    }

    pub fn set_dynamics(&mut self, settings: DynamicsSettings) {
        self.dynamics = settings.sanitized();
    }

    pub fn voice_ids(&self) -> Vec<u64> {
        self.voices.iter().map(|voice| voice.id).collect()
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        for voice in &self.voices {
            voice.set_master_volume(volume);
        }
    }

    pub fn stop_all_monitors(&mut self) {
        for voice in &mut self.voices {
            voice.stop_monitor();
        }
    }

    pub fn progress(&self) -> Option<f32> {
        self.voices
            .last()
            .map(|voice| voice.sink_state.borrow().progress())
    }
}

impl Default for VoicePool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
