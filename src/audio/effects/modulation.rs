//! Robot-voice effect via ring modulation.
//!
//! Ring modulation multiplies the input (voice) by an internal sine carrier,
//! producing the metallic, mechanical timbre of a classic "robot voice". The
//! output energy lands at the sum and difference frequencies (`f_in ± f_c`),
//! suppressing the original input frequency — the defining trait of true ring
//! modulation (as opposed to amplitude modulation, which keeps the carrier).
//!
//! # fundsp graph
//! The DSP graph is built with operator composition:
//! `pass() * sine_hz(carrier)` — `pass()` forwards the input unchanged while
//! `sine_hz` generates the carrier; the `*` operator multiplies them sample by
//! sample. The result is a single-input, single-output node.
//!
//! # Real-time safety
//! [`RingModEffect::process`] only calls `tick()` on the pre-built graph (pure
//! arithmetic, no allocation) plus a guarded `set_sample_rate` that runs only
//! when the rate actually changes. The graph is rebuilt only in
//! [`RingModEffect::set_param`], which the trait contract documents as running
//! on the command-handler (cold) path, never the audio callback.

use crate::audio::effects::AudioEffect;
use crate::audio::error::EffectsError;
use fundsp::prelude32::*;

/// Default carrier frequency in Hz. Low frequencies (~150 Hz) give the classic
/// "robot" growl; higher frequencies push toward bell-like, inharmonic tones.
const DEFAULT_CARRIER_HZ: f32 = 150.0;

/// Ring-modulation ("robot voice") effect.
///
/// See the [module documentation](self) for the DSP and real-time-safety model.
pub struct RingModEffect {
    /// Boxed fundsp graph `pass() * sine_hz(carrier)` (1 input, 1 output).
    /// Boxed so [`set_param`](RingModEffect::set_param) can swap it on a carrier
    /// change without naming the verbose concrete combinator type.
    graph: Box<dyn AudioUnit>,
    carrier_hz: f32,
    /// Last sample rate pushed into the graph. `0` means "not yet set".
    sample_rate: u32,
    bypassed: bool,
}

impl RingModEffect {
    /// Create a `RingModEffect` with the default carrier frequency.
    ///
    /// `_block_size` is accepted for signature parity with other effects; this
    /// effect processes sample-by-sample and needs no pre-sized buffer.
    #[must_use]
    pub fn new(_block_size: usize) -> Self {
        Self {
            graph: Self::build_graph(DEFAULT_CARRIER_HZ),
            carrier_hz: DEFAULT_CARRIER_HZ,
            sample_rate: 0,
            bypassed: false,
        }
    }

    /// Build the `pass() * sine_hz(carrier)` graph for a given carrier.
    fn build_graph(carrier_hz: f32) -> Box<dyn AudioUnit> {
        Box::new(pass() * sine_hz(carrier_hz))
    }

    /// Current carrier frequency in Hz.
    #[must_use]
    pub fn carrier_hz(&self) -> f32 {
        self.carrier_hz
    }
}

impl AudioEffect for RingModEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        if sample_rate != self.sample_rate {
            self.graph.set_sample_rate(f64::from(sample_rate));
            self.sample_rate = sample_rate;
        }
        let mut frame_in = [0.0f32; 1];
        let mut frame_out = [0.0f32; 1];
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            frame_in[0] = i;
            self.graph.tick(&frame_in, &mut frame_out);
            *o = frame_out[0];
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "carrier" | "carrier_hz" => {
                self.carrier_hz = value.max(0.0);
                self.graph = Self::build_graph(self.carrier_hz);
                if self.sample_rate != 0 {
                    self.graph.set_sample_rate(f64::from(self.sample_rate));
                }
                Ok(())
            }
            _ => Err(EffectsError::ParamUnknown {
                param: param.to_owned(),
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
    use crate::audio::error::EffectsError;

    fn goertzel_mag(samples: &[f32], freq: f32, sample_rate: f32) -> f32 {
        let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let coeff = 2.0 * omega.cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in samples {
            let s0 = x + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt()
    }

    fn sine_block(freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn ring_mod_default_carrier_is_150hz() {
        let effect = RingModEffect::new(1024);
        assert!((effect.carrier_hz() - 150.0).abs() < 1e-3);
    }

    #[test]
    fn ring_mod_produces_sidebands_and_suppresses_input() {
        let sr = 48_000.0f32;
        let f_in = 1000.0f32;
        let f_c = 150.0f32;
        let n = 4096;
        let mut effect = RingModEffect::new(n);
        let input = sine_block(f_in, sr, n);
        let mut output = vec![0.0f32; n];
        effect.process(&input, &mut output, sr as u32);

        let mag_in = goertzel_mag(&output, f_in, sr);
        let mag_upper = goertzel_mag(&output, f_in + f_c, sr);
        let mag_lower = goertzel_mag(&output, f_in - f_c, sr);

        assert!(
            mag_upper > mag_in * 5.0,
            "upper sideband should dominate input bin: upper={mag_upper} in={mag_in}"
        );
        assert!(
            mag_lower > mag_in * 5.0,
            "lower sideband should dominate input bin: lower={mag_lower} in={mag_in}"
        );
    }

    #[test]
    fn ring_mod_set_carrier_changes_value() {
        let mut effect = RingModEffect::new(64);
        effect.set_param("carrier", 300.0).unwrap();
        assert!((effect.carrier_hz() - 300.0).abs() < 1e-3);
    }

    #[test]
    fn ring_mod_unknown_param_rejected() {
        let mut effect = RingModEffect::new(64);
        let err = effect.set_param("nope", 1.0);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn ring_mod_bypass_passes_through() {
        let mut effect = RingModEffect::new(64);
        effect.set_bypass(true);
        let input = vec![0.1f32, -0.2, 0.3, -0.4];
        let mut output = vec![0.0f32; 4];
        effect.process(&input, &mut output, 48_000);
        assert_eq!(output, input);
    }

    #[test]
    fn ring_mod_latency_is_zero() {
        let effect = RingModEffect::new(64);
        assert_eq!(effect.latency_samples(), 0);
    }
}
