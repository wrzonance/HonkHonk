//! Stereo reverb effect (issue #34), built on fundsp's `reverb2_stereo` FDN.
//!
//! # Real-time safety
//! The fundsp graph and its delay lines are allocated once in [`Reverb::new`]
//! and in [`Reverb::set_param`] (both off the PipeWire thread). The hot
//! [`process`](AudioEffect::process) path only calls `tick`, which uses
//! stack-allocated frames — no allocation, locking, or syscalls.

use crate::audio::effects::AudioEffect;
use crate::audio::error::EffectsError;
use fundsp::prelude32::*;

/// fundsp's `reverb2_stereo` expects `room_size` in meters (10..=30) and a
/// reverberation `time` in seconds. Our public parameters are normalized to
/// `0.0..=1.0` (matching the issue's preset table) and mapped onto those
/// physical ranges here.
const MIN_ROOM_METERS: f32 = 10.0;
const MAX_ROOM_METERS: f32 = 30.0;
/// Decay `1.0` maps to this many seconds of reverb tail (Cathedral-scale).
const MAX_DECAY_SECONDS: f32 = 8.0;
/// Floor so `decay = 0.0` still produces a (very short) audible tail rather
/// than a degenerate zero-length reverb.
const MIN_DECAY_SECONDS: f32 = 0.3;
/// Diffusion for the FDN — fixed; the issue exposes only room_size and decay.
const DIFFUSION: f32 = 0.5;
/// Loop-filter modulation speed (fundsp nominal 0..1).
const MODULATION_SPEED: f32 = 1.0;
/// Loop-filter cutoff (Hz) — gentle high-frequency damping of the tail.
const LOOP_FILTER_HZ: f32 = 8000.0;

/// A stereo reverb. Input is mono (HonkHonk's mic path); it is fed to both
/// reverb channels and the two wet outputs are averaged back to mono.
pub struct Reverb {
    node: Box<dyn AudioUnit>,
    room_size: f32,
    decay: f32,
    sample_rate: f32,
    bypassed: bool,
}

impl Reverb {
    /// Build a reverb. `room_size` and `decay` are normalized `0.0..=1.0`.
    pub fn new(room_size: f32, decay: f32) -> Self {
        let room_size = room_size.clamp(0.0, 1.0);
        let decay = decay.clamp(0.0, 1.0);
        let node = build_node(room_size, decay, DEFAULT_SR);
        Self {
            node,
            room_size,
            decay,
            sample_rate: DEFAULT_SR,
            bypassed: false,
        }
    }

    /// "Bathroom" preset: tight, reflective small room.
    pub fn bathroom() -> Self {
        Self::new(0.1, 0.3)
    }

    /// "Room" preset: natural-sounding room.
    pub fn room() -> Self {
        Self::new(0.3, 0.5)
    }

    /// "Hall" preset: concert hall.
    pub fn hall() -> Self {
        Self::new(0.6, 0.7)
    }

    /// "Cathedral" preset: massive space, long tail.
    pub fn cathedral() -> Self {
        Self::new(0.9, 0.9)
    }

    /// Rebuild the fundsp node from the current parameters at `sample_rate`.
    /// Called only from the cold path (`set_param`) — never from `process`.
    fn rebuild(&mut self) {
        self.node = build_node(self.room_size, self.decay, self.sample_rate);
    }
}

/// Sample rate assumed before the first `process` call configures the real one.
const DEFAULT_SR: f32 = 48_000.0;

/// Construct the stereo reverb graph for the given normalized parameters.
/// Allocates the FDN delay lines — must run off the RT thread.
fn build_node(room_size: f32, decay: f32, sample_rate: f32) -> Box<dyn AudioUnit> {
    let meters = MIN_ROOM_METERS + room_size * (MAX_ROOM_METERS - MIN_ROOM_METERS);
    let seconds = MIN_DECAY_SECONDS + decay * (MAX_DECAY_SECONDS - MIN_DECAY_SECONDS);
    let mut node = Box::new(reverb2_stereo(
        meters,
        seconds,
        DIFFUSION,
        MODULATION_SPEED,
        lowpole_hz(LOOP_FILTER_HZ),
    )) as Box<dyn AudioUnit>;
    node.set_sample_rate(sample_rate as f64);
    node
}

impl AudioEffect for Reverb {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        // Sample-rate changes are rare; reacting here would require a rebuild
        // (allocation) on the RT thread, which is forbidden. We instead assume
        // the engine's fixed rate; `process` only ticks the pre-built graph.
        debug_assert_eq!(sample_rate as f32, self.sample_rate);
        let mut frame_in = [0.0_f32; 2];
        let mut frame_out = [0.0_f32; 2];
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            frame_in[0] = i;
            frame_in[1] = i;
            self.node.tick(&frame_in, &mut frame_out);
            *o = 0.5 * (frame_out[0] + frame_out[1]);
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "room_size" => {
                self.room_size = value.clamp(0.0, 1.0);
                self.rebuild();
                Ok(())
            }
            "decay" => {
                self.decay = value.clamp(0.0, 1.0);
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
        // FDN reverb has no fixed pre-delay we report as latency; the wet tail
        // is decorrelated from the dry signal and not a pipeline delay.
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impulse(len: usize) -> Vec<f32> {
        let mut v = vec![0.0_f32; len];
        v[0] = 1.0;
        v
    }

    fn energy(buf: &[f32]) -> f32 {
        buf.iter().map(|s| s * s).sum()
    }

    fn is_finite(buf: &[f32]) -> bool {
        buf.iter().all(|s| s.is_finite())
    }

    #[test]
    fn reverb_produces_finite_output() {
        let mut fx = Reverb::new(0.5, 0.5);
        let input = impulse(256);
        let mut output = vec![0.0_f32; 256];
        fx.process(&input, &mut output, 48_000);
        assert!(is_finite(&output), "reverb output must be finite");
    }

    #[test]
    fn reverb_spreads_impulse_energy_over_time() {
        // A reverb of a single-sample impulse should smear energy well past the
        // impulse into a decorrelated tail. The FDN's wet tail only begins after
        // its shortest internal delay-line tap (10-30 m room => several hundred
        // samples of earliest wet-tail onset). This is internal FDN timing, not a
        // host-compensable pipeline latency (`latency_samples()` returns 0), so we
        // use a generously long buffer and measure energy in its back half.
        let len = 8192;
        let mut fx = Reverb::new(0.6, 0.7);
        let input = impulse(len);
        let mut output = vec![0.0_f32; len];
        fx.process(&input, &mut output, 48_000);
        let tail_energy: f32 = energy(&output[len / 2..]);
        assert!(
            tail_energy > 0.0,
            "reverb tail should carry energy after the impulse, got {tail_energy}"
        );
    }

    #[test]
    fn reverb_bypass_is_passthrough() {
        let mut fx = Reverb::new(0.9, 0.9);
        fx.set_bypass(true);
        let input = impulse(64);
        let mut output = vec![0.0_f32; 64];
        fx.process(&input, &mut output, 48_000);
        assert_eq!(output, input);
    }

    #[test]
    fn reverb_set_param_room_size_ok() {
        let mut fx = Reverb::new(0.5, 0.5);
        assert!(fx.set_param("room_size", 0.8).is_ok());
        assert!((fx.room_size - 0.8).abs() < 1e-6);
    }

    #[test]
    fn reverb_set_param_decay_ok() {
        let mut fx = Reverb::new(0.5, 0.5);
        assert!(fx.set_param("decay", 0.2).is_ok());
        assert!((fx.decay - 0.2).abs() < 1e-6);
    }

    #[test]
    fn reverb_set_param_clamps_out_of_range() {
        let mut fx = Reverb::new(0.5, 0.5);
        fx.set_param("room_size", 5.0).unwrap();
        assert!((fx.room_size - 1.0).abs() < 1e-6);
        fx.set_param("decay", -3.0).unwrap();
        assert!(fx.decay.abs() < 1e-6);
    }

    #[test]
    fn reverb_set_param_unknown_rejected() {
        let mut fx = Reverb::new(0.5, 0.5);
        let err = fx.set_param("nonsense", 1.0);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn reverb_cathedral_has_longer_tail_than_bathroom() {
        let len = 8192;
        let input = impulse(len);

        let mut cathedral = Reverb::cathedral();
        let mut cat_out = vec![0.0_f32; len];
        cathedral.process(&input, &mut cat_out, 48_000);

        let mut bathroom = Reverb::bathroom();
        let mut bath_out = vec![0.0_f32; len];
        bathroom.process(&input, &mut bath_out, 48_000);

        // Energy in the far tail (last quarter) should be greater for the
        // cathedral's long decay than the bathroom's tight one.
        let quarter = len * 3 / 4;
        let cat_tail = energy(&cat_out[quarter..]);
        let bath_tail = energy(&bath_out[quarter..]);
        assert!(
            cat_tail > bath_tail,
            "cathedral tail ({cat_tail}) should exceed bathroom tail ({bath_tail})"
        );
    }

    #[test]
    fn reverb_presets_construct() {
        let _ = Reverb::bathroom();
        let _ = Reverb::room();
        let _ = Reverb::hall();
        let _ = Reverb::cathedral();
    }

    #[test]
    fn reverb_latency_is_zero() {
        let fx = Reverb::new(0.5, 0.5);
        assert_eq!(fx.latency_samples(), 0);
    }
}
