//! Radio / walkie-talkie effect via a bandpass filter plus optional pink noise.
//!
//! A telephone/walkie-talkie sound comes from discarding the low and high ends
//! of the spectrum, leaving only a narrow mid-range "communications band".
//! Optional pink noise mixed on top adds the static/crackle of a weak radio
//! link.
//!
//! # fundsp graph
//! The signal path is built from fundsp primitives:
//! - [`bandpass_hz`] — a state-variable bandpass filter centered at `center_hz`
//!   with quality factor `q`.
//! - [`pink`] — pink noise, itself the operator-composed graph
//!   `white() >> pinkpass()`.
//!
//! Filtered signal and scaled noise are summed per sample. The noise level is a
//! plain runtime scalar (not baked into a graph), so changing it is allocation
//! free and real-time safe.
//!
//! # Bandwidth vs. Q
//! The issue specifies a *bandwidth* (~2 kHz), but `bandpass_hz` takes a *Q*
//! value. They relate by the standard definition `Q = center / bandwidth`, so
//! center 1500 Hz with bandwidth 2000 Hz yields `Q = 0.75`. This keeps the
//! public API in the intuitive units the issue asked for while feeding fundsp
//! what it expects.
//!
//! # Real-time safety
//! [`BandpassFilterEffect::process`] only calls `filter_mono` / `get_mono`
//! (pure arithmetic, no allocation) plus a guarded `set_sample_rate` that runs
//! only when the rate changes. Parameter changes use fundsp's real-time-safe
//! [`Setting::center_q`] — no graph rebuild, no allocation.

use crate::audio::effects::AudioEffect;
use crate::audio::error::EffectsError;
use fundsp::prelude32::*;

/// Default center frequency in Hz — the middle of the "telephone" band.
const DEFAULT_CENTER_HZ: f32 = 1500.0;
/// Default passband width in Hz.
const DEFAULT_BANDWIDTH_HZ: f32 = 2000.0;
/// Floor on bandwidth to avoid division by zero / runaway Q.
const MIN_BANDWIDTH_HZ: f32 = 1.0;

/// Radio / walkie-talkie bandpass effect with optional pink-noise crackle.
///
/// See the [module documentation](self) for the DSP and real-time-safety model.
pub struct BandpassFilterEffect {
    /// State-variable bandpass node. `center`/`Q` change via `set()` (RT-safe).
    filter: An<FixedSvf<f32, BandpassMode<f32>>>,
    /// Pink-noise generator: `white() >> pinkpass()`.
    noise: An<Pipe<Noise, Pinkpass<f32>>>,
    center_hz: f32,
    bandwidth_hz: f32,
    /// Pink-noise mix level in `[0, 1]`. `0` = no crackle.
    noise_level: f32,
    /// Last sample rate pushed into the nodes. `0` means "not yet set".
    sample_rate: u32,
    bypassed: bool,
}

impl BandpassFilterEffect {
    /// Create a `BandpassFilterEffect` with the default radio voicing and no
    /// noise.
    ///
    /// `_block_size` is accepted for signature parity with other effects; this
    /// effect processes sample-by-sample and needs no pre-sized buffer.
    #[must_use]
    pub fn new(_block_size: usize) -> Self {
        let q = DEFAULT_CENTER_HZ / DEFAULT_BANDWIDTH_HZ;
        Self {
            filter: bandpass_hz(DEFAULT_CENTER_HZ, q),
            noise: pink(),
            center_hz: DEFAULT_CENTER_HZ,
            bandwidth_hz: DEFAULT_BANDWIDTH_HZ,
            noise_level: 0.0,
            sample_rate: 0,
            bypassed: false,
        }
    }

    /// Quality factor derived from the current center and bandwidth.
    fn q(&self) -> f32 {
        self.center_hz / self.bandwidth_hz.max(MIN_BANDWIDTH_HZ)
    }

    /// Current center frequency in Hz.
    #[must_use]
    pub fn center_hz(&self) -> f32 {
        self.center_hz
    }

    /// Current passband width in Hz.
    #[must_use]
    pub fn bandwidth_hz(&self) -> f32 {
        self.bandwidth_hz
    }

    /// Current pink-noise mix level in `[0, 1]`.
    #[must_use]
    pub fn noise_level(&self) -> f32 {
        self.noise_level
    }
}

impl AudioEffect for BandpassFilterEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        if sample_rate != self.sample_rate {
            self.filter.set_sample_rate(f64::from(sample_rate));
            self.noise.set_sample_rate(f64::from(sample_rate));
            self.sample_rate = sample_rate;
        }
        let noise_level = self.noise_level;
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            let filtered = self.filter.filter_mono(i);
            let crackle = if noise_level > 0.0 {
                self.noise.get_mono() * noise_level
            } else {
                0.0
            };
            *o = filtered + crackle;
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "center" | "center_hz" => {
                self.center_hz = value.max(0.0);
                self.filter.set(Setting::center_q(self.center_hz, self.q()));
                Ok(())
            }
            "bandwidth" | "bandwidth_hz" => {
                self.bandwidth_hz = value.max(MIN_BANDWIDTH_HZ);
                self.filter.set(Setting::center_q(self.center_hz, self.q()));
                Ok(())
            }
            "noise" | "noise_level" => {
                self.noise_level = value.clamp(0.0, 1.0);
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

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn bandpass_defaults() {
        let effect = BandpassFilterEffect::new(1024);
        assert!((effect.center_hz() - 1500.0).abs() < 1e-3);
        assert!((effect.bandwidth_hz() - 2000.0).abs() < 1e-3);
        assert!(effect.noise_level().abs() < 1e-6);
    }

    #[test]
    fn bandpass_passes_in_band_attenuates_out_of_band() {
        let sr = 48_000.0f32;
        let n = 8192;
        let warmup = 2048; // skip filter transient

        let mut in_band = BandpassFilterEffect::new(n);
        let inb_input = sine_block(1500.0, sr, n);
        let mut inb_out = vec![0.0f32; n];
        in_band.process(&inb_input, &mut inb_out, sr as u32);

        let mut low = BandpassFilterEffect::new(n);
        let low_input = sine_block(100.0, sr, n);
        let mut low_out = vec![0.0f32; n];
        low.process(&low_input, &mut low_out, sr as u32);

        let inb_rms = rms(&inb_out[warmup..]);
        let low_rms = rms(&low_out[warmup..]);

        assert!(inb_rms > 0.2, "in-band tone should pass, got {inb_rms}");
        assert!(
            low_rms < inb_rms * 0.5,
            "out-of-band tone should be attenuated: low={low_rms} in_band={inb_rms}"
        );

        let center_bin = goertzel_mag(&inb_out[warmup..], 1500.0, sr);
        let low_bin = goertzel_mag(&low_out[warmup..], 100.0, sr);
        assert!(
            center_bin > low_bin,
            "center bin should exceed out-of-band bin: center={center_bin} low={low_bin}"
        );
    }

    #[test]
    fn bandpass_set_center_and_bandwidth() {
        let mut effect = BandpassFilterEffect::new(64);
        effect.set_param("center", 1000.0).unwrap();
        effect.set_param("bandwidth", 500.0).unwrap();
        assert!((effect.center_hz() - 1000.0).abs() < 1e-3);
        assert!((effect.bandwidth_hz() - 500.0).abs() < 1e-3);
    }

    #[test]
    fn bandpass_noise_adds_energy_to_silence() {
        let n = 2048;
        let mut effect = BandpassFilterEffect::new(n);
        effect.set_param("noise", 0.5).unwrap();
        let input = vec![0.0f32; n];
        let mut output = vec![0.0f32; n];
        effect.process(&input, &mut output, 48_000);
        let energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(
            energy > 0.0,
            "noise mix should add energy to a silent input"
        );
    }

    #[test]
    fn bandpass_zero_noise_silent_input_stays_silent() {
        let n = 512;
        let mut effect = BandpassFilterEffect::new(n);
        let input = vec![0.0f32; n];
        let mut output = vec![0.0f32; n];
        effect.process(&input, &mut output, 48_000);
        let energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(energy < 1e-9, "no noise + silence in => silence out");
    }

    #[test]
    fn bandpass_unknown_param_rejected() {
        let mut effect = BandpassFilterEffect::new(64);
        let err = effect.set_param("nope", 1.0);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn bandpass_bypass_passes_through() {
        let mut effect = BandpassFilterEffect::new(64);
        effect.set_bypass(true);
        let input = vec![0.1f32, -0.2, 0.3, -0.4];
        let mut output = vec![0.0f32; 4];
        effect.process(&input, &mut output, 48_000);
        assert_eq!(output, input);
    }

    #[test]
    fn bandpass_latency_is_zero() {
        let effect = BandpassFilterEffect::new(64);
        assert_eq!(effect.latency_samples(), 0);
    }

    #[test]
    fn both_effects_compose_in_chain() {
        use crate::audio::effects::chain::EffectChain;
        use crate::audio::effects::modulation::RingModEffect;

        let block = 256;
        let mut chain = EffectChain::new(block);
        chain
            .push_effect(Box::new(RingModEffect::new(block)), block)
            .unwrap();
        chain
            .push_effect(Box::new(BandpassFilterEffect::new(block)), block)
            .unwrap();

        let input: Vec<f32> = (0..block)
            .map(|i| (2.0 * std::f32::consts::PI * 1500.0 * i as f32 / 48_000.0).sin())
            .collect();
        let mut output = vec![0.0f32; block];
        chain.process(&input, &mut output, 48_000);

        assert!(output.iter().all(|s| s.is_finite()));
        assert!(
            output != input,
            "chained effects should transform the signal"
        );
    }
}
