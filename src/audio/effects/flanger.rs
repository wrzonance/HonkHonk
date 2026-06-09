//! Flanger effect (issue #34), built on fundsp's mono `flanger` node.
//!
//! fundsp's `flanger(feedback, min_delay, max_delay, delay_f)` is a short
//! feedback delay swept by an LFO; its output *includes the original signal*.
//! We expose `rate` (LFO sweep speed), `depth` (sweep range) and `feedback`.
//!
//! # Real-time safety
//! The node and its delay line are built in [`Flanger::new`] /
//! [`Flanger::set_param`] (cold paths). [`process`](AudioEffect::process) only
//! ticks the graph — no allocation, locks or syscalls.

use crate::audio::effects::AudioEffect;
use crate::audio::error::EffectsError;
use fundsp::prelude32::*;

/// Sample rate assumed before the engine reports the real one.
const DEFAULT_SR: f32 = 48_000.0;
/// Shortest delay the sweep ever reaches (seconds).
const BASE_MIN_DELAY: f32 = 0.0005;
/// `depth` (0..=1) scales the sweep span on top of `BASE_MIN_DELAY`, up to
/// this many seconds of maximum delay — the classic short flanger range.
const MAX_SWEEP_SPAN: f32 = 0.010;
/// `rate` (0..=1) maps onto this LFO sweep frequency range, in Hz.
const MIN_RATE_HZ: f32 = 0.05;
const MAX_RATE_HZ: f32 = 2.0;
/// `feedback` (0..=1) maps onto this signed feedback range. We keep it below
/// 1.0 to avoid a runaway/self-oscillating comb filter.
const MAX_FEEDBACK: f32 = 0.9;

/// A mono flanger with `rate`, `depth` and `feedback` controls.
pub struct Flanger {
    node: Box<dyn AudioUnit>,
    rate: f32,
    depth: f32,
    feedback: f32,
    sample_rate: f32,
    bypassed: bool,
}

impl Flanger {
    /// Build a flanger. `rate`, `depth`, `feedback` are normalized `0.0..=1.0`.
    pub fn new(rate: f32, depth: f32, feedback: f32) -> Self {
        let rate = rate.clamp(0.0, 1.0);
        let depth = depth.clamp(0.0, 1.0);
        let feedback = feedback.clamp(0.0, 1.0);
        let node = build_node(rate, depth, feedback, DEFAULT_SR);
        Self {
            node,
            rate,
            depth,
            feedback,
            sample_rate: DEFAULT_SR,
            bypassed: false,
        }
    }

    /// "Jet" preset: deep, resonant jet-plane sweep.
    pub fn jet() -> Self {
        Self::new(0.15, 1.0, 0.85)
    }

    /// Rebuild the fundsp node from current params. Cold path only.
    fn rebuild(&mut self) {
        self.node = build_node(self.rate, self.depth, self.feedback, self.sample_rate);
    }
}

/// Build the mono flanger graph. Allocates the delay line — off RT thread.
fn build_node(rate: f32, depth: f32, feedback: f32, sample_rate: f32) -> Box<dyn AudioUnit> {
    let rate_hz = MIN_RATE_HZ + rate * (MAX_RATE_HZ - MIN_RATE_HZ);
    let min_delay = BASE_MIN_DELAY;
    let max_delay = BASE_MIN_DELAY + depth * MAX_SWEEP_SPAN;
    let fb = feedback * MAX_FEEDBACK;
    // LFO sweeps the delay between min and max via a sine. `lerp11` maps the
    // sine's -1..1 output onto the delay range. Captured by value → Send+Sync.
    let delay_f = move |t: f32| lerp11(min_delay, max_delay, sin_hz(rate_hz, t));
    let mut node = Box::new(flanger(fb, min_delay, max_delay, delay_f)) as Box<dyn AudioUnit>;
    node.set_sample_rate(sample_rate as f64);
    node
}

impl AudioEffect for Flanger {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        debug_assert_eq!(sample_rate as f32, self.sample_rate);
        let mut frame_in = [0.0_f32; 1];
        let mut frame_out = [0.0_f32; 1];
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            frame_in[0] = i;
            self.node.tick(&frame_in, &mut frame_out);
            *o = frame_out[0];
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
            "feedback" => {
                self.feedback = value.clamp(0.0, 1.0);
                self.rebuild();
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
    fn flanger_output_is_finite() {
        let mut fx = Flanger::new(0.2, 0.8, 0.7);
        let input = sine(1024, 200.0, 48_000.0);
        let mut output = vec![0.0_f32; 1024];
        fx.process(&input, &mut output, 48_000);
        assert!(is_finite(&output));
    }

    #[test]
    fn flanger_alters_signal() {
        let mut fx = Flanger::new(0.3, 1.0, 0.8);
        let input = sine(2048, 200.0, 48_000.0);
        let mut output = vec![0.0_f32; 2048];
        fx.process(&input, &mut output, 48_000);
        assert!(
            max_abs_diff(&input, &output) > 1e-3,
            "flanger should change the waveform"
        );
    }

    #[test]
    fn flanger_bypass_is_passthrough() {
        let mut fx = Flanger::new(0.3, 1.0, 0.8);
        fx.set_bypass(true);
        let input = sine(256, 440.0, 48_000.0);
        let mut output = vec![0.0_f32; 256];
        fx.process(&input, &mut output, 48_000);
        assert_eq!(output, input);
    }

    #[test]
    fn flanger_high_feedback_stays_finite() {
        // Even with maximum feedback the comb filter must not blow up to NaN/inf.
        let mut fx = Flanger::new(0.1, 1.0, 1.0);
        let input = sine(8192, 150.0, 48_000.0);
        let mut output = vec![0.0_f32; 8192];
        fx.process(&input, &mut output, 48_000);
        assert!(is_finite(&output), "max-feedback flanger must stay bounded");
    }

    #[test]
    fn flanger_set_params_ok() {
        let mut fx = Flanger::new(0.3, 0.3, 0.3);
        assert!(fx.set_param("rate", 0.7).is_ok());
        assert!(fx.set_param("depth", 0.9).is_ok());
        assert!(fx.set_param("feedback", 0.5).is_ok());
        assert!((fx.rate - 0.7).abs() < 1e-6);
        assert!((fx.depth - 0.9).abs() < 1e-6);
        assert!((fx.feedback - 0.5).abs() < 1e-6);
    }

    #[test]
    fn flanger_set_param_unknown_rejected() {
        let mut fx = Flanger::new(0.3, 0.3, 0.3);
        let err = fx.set_param("nope", 1.0);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn flanger_jet_preset_constructs() {
        let fx = Flanger::jet();
        assert_eq!(fx.latency_samples(), 0);
    }
}
