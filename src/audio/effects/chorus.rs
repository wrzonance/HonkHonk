//! Chorus effect (issue #34), built on fundsp's mono `chorus` node.
//!
//! fundsp's `chorus(seed, separation, variation, mod_frequency)` is a delay
//! line with LFO-modulated taps whose output *already includes the dry
//! signal*. We expose `rate` (LFO speed), `depth` (modulation amount) and
//! `mix` (blend between the original input and the chorused signal).
//!
//! # Real-time safety
//! The fundsp node and its delay line are allocated in [`Chorus::new`] /
//! [`Chorus::set_param`] (cold paths). [`process`](AudioEffect::process) only
//! ticks the graph and does a scalar blend — no allocation, locks or syscalls.

use crate::audio::effects::AudioEffect;
use crate::audio::error::EffectsError;
use fundsp::prelude32::*;

/// Sample rate assumed before the engine reports the real one.
const DEFAULT_SR: f32 = 48_000.0;
/// Deterministic seed so chorus voicing is reproducible across runs/tests.
const SEED: u64 = 0;
/// Base tap separation (seconds) — the centre delay of the chorus voices.
const SEPARATION: f32 = 0.015;
/// `rate` (0..=1) maps onto this LFO frequency range, in Hz.
const MIN_RATE_HZ: f32 = 0.1;
const MAX_RATE_HZ: f32 = 5.0;
/// `depth` (0..=1) maps onto this delay-variation range, in seconds.
const MIN_VARIATION: f32 = 0.0005;
const MAX_VARIATION: f32 = 0.010;

/// A mono chorus with `rate`, `depth` and `mix` controls.
pub struct Chorus {
    node: Box<dyn AudioUnit>,
    rate: f32,
    depth: f32,
    mix: f32,
    sample_rate: f32,
    bypassed: bool,
}

impl Chorus {
    /// Build a chorus. `rate`, `depth`, `mix` are normalized `0.0..=1.0`.
    pub fn new(rate: f32, depth: f32, mix: f32) -> Self {
        let rate = rate.clamp(0.0, 1.0);
        let depth = depth.clamp(0.0, 1.0);
        let mix = mix.clamp(0.0, 1.0);
        let node = build_node(rate, depth, DEFAULT_SR);
        Self {
            node,
            rate,
            depth,
            mix,
            sample_rate: DEFAULT_SR,
            bypassed: false,
        }
    }

    /// "Thick Voice" preset: lush, wide chorus for vocal thickening.
    pub fn thick_voice() -> Self {
        Self::new(0.3, 0.7, 0.6)
    }

    /// Rebuild the fundsp node from current params. Cold path only.
    fn rebuild(&mut self) {
        self.node = build_node(self.rate, self.depth, self.sample_rate);
    }
}

/// Build the mono chorus graph. Allocates the delay line — off RT thread.
fn build_node(rate: f32, depth: f32, sample_rate: f32) -> Box<dyn AudioUnit> {
    let mod_hz = MIN_RATE_HZ + rate * (MAX_RATE_HZ - MIN_RATE_HZ);
    let variation = MIN_VARIATION + depth * (MAX_VARIATION - MIN_VARIATION);
    let mut node = Box::new(chorus(SEED, SEPARATION, variation, mod_hz)) as Box<dyn AudioUnit>;
    node.set_sample_rate(sample_rate as f64);
    node
}

impl AudioEffect for Chorus {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        debug_assert_eq!(sample_rate as f32, self.sample_rate);
        let wet = self.mix;
        let dry = 1.0 - wet;
        let mut frame_in = [0.0_f32; 1];
        let mut frame_out = [0.0_f32; 1];
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            frame_in[0] = i;
            self.node.tick(&frame_in, &mut frame_out);
            *o = dry * i + wet * frame_out[0];
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "rate" => {
                self.rate = value.clamp(0.0, 1.0);
                self.rebuild();
                Ok(())
            }
            "depth" => {
                self.depth = value.clamp(0.0, 1.0);
                self.rebuild();
                Ok(())
            }
            "mix" => {
                self.mix = value.clamp(0.0, 1.0);
                Ok(())
            }
            "sample_rate" => {
                self.sample_rate = value.max(1.0);
                self.rebuild();
                Ok(())
            }
            other => Err(EffectsError::ParamUnknown {
                param: other.to_owned(),
            }),
        }
    }

    fn bypass(&self) -> bool {
        self.bypassed
    }

    fn set_bypass(&mut self, bypass: bool) {
        self.bypassed = bypass;
    }

    fn latency_samples(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(len: usize, hz: f32, sr: f32) -> Vec<f32> {
        (0..len)
            .map(|n| (std::f32::consts::TAU * hz * n as f32 / sr).sin())
            .collect()
    }

    fn is_finite(buf: &[f32]) -> bool {
        buf.iter().all(|s| s.is_finite())
    }

    fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f32::max)
    }

    #[test]
    fn chorus_output_is_finite() {
        let mut fx = Chorus::new(0.3, 0.7, 0.6);
        let input = sine(1024, 220.0, 48_000.0);
        let mut output = vec![0.0_f32; 1024];
        fx.process(&input, &mut output, 48_000);
        assert!(is_finite(&output));
    }

    #[test]
    fn chorus_alters_signal_when_wet() {
        let mut fx = Chorus::new(0.5, 0.8, 1.0);
        let input = sine(2048, 220.0, 48_000.0);
        let mut output = vec![0.0_f32; 2048];
        fx.process(&input, &mut output, 48_000);
        // A fully-wet chorus must change the waveform vs. the dry input.
        assert!(
            max_abs_diff(&input, &output) > 1e-3,
            "wet chorus should differ from dry input"
        );
    }

    #[test]
    fn chorus_mix_zero_is_passthrough() {
        let mut fx = Chorus::new(0.5, 0.8, 0.0);
        let input = sine(512, 330.0, 48_000.0);
        let mut output = vec![0.0_f32; 512];
        fx.process(&input, &mut output, 48_000);
        assert!(
            max_abs_diff(&input, &output) < 1e-6,
            "mix=0 should pass the dry signal through unchanged"
        );
    }

    #[test]
    fn chorus_bypass_is_passthrough() {
        let mut fx = Chorus::new(0.5, 0.8, 1.0);
        fx.set_bypass(true);
        let input = sine(256, 440.0, 48_000.0);
        let mut output = vec![0.0_f32; 256];
        fx.process(&input, &mut output, 48_000);
        assert_eq!(output, input);
    }

    #[test]
    fn chorus_set_params_ok() {
        let mut fx = Chorus::new(0.3, 0.3, 0.3);
        assert!(fx.set_param("rate", 0.9).is_ok());
        assert!(fx.set_param("depth", 0.1).is_ok());
        assert!(fx.set_param("mix", 0.5).is_ok());
        assert!((fx.rate - 0.9).abs() < 1e-6);
        assert!((fx.depth - 0.1).abs() < 1e-6);
        assert!((fx.mix - 0.5).abs() < 1e-6);
    }

    #[test]
    fn chorus_set_param_unknown_rejected() {
        let mut fx = Chorus::new(0.3, 0.3, 0.3);
        let err = fx.set_param("bogus", 1.0);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn chorus_thick_voice_preset_constructs() {
        let fx = Chorus::thick_voice();
        assert_eq!(fx.latency_samples(), 0);
    }
}
