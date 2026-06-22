//! Formant-aware pitch shift (issue #35, PR 6 of 6 of voice effects #18).
//!
//! Shifts pitch while independently controlling the spectral formant envelope,
//! so a voice can be lowered/raised without the "chipmunk" or "barrel" artefact
//! a naive resampling pitch shift produces. This is the Tier-3 quality tier; the
//! Tier-1 [`PitchShiftEffect`](super::PitchShiftEffect) is the A/B baseline.
//!
//! # Approach (fundsp `resynth`)
//! fundsp's `resynth` opcode runs an overlap-4 Hann-windowed STFT/IFFT and calls
//! a user closure once per FFT window with the input spectrum, expecting the
//! output spectrum back. Per window we: extract a smoothed spectral envelope
//! (the formants), divide it out to get the excitation, resample the excitation
//! by `pitch_ratio`, then re-impose the envelope resampled by an independent
//! `formant_ratio`. See the crate-level design notes for the maths.
//!
//! # Real-time safety
//! The fundsp node and every scratch buffer are allocated in [`Self::new`]. The
//! closure that runs on the PipeWire thread reads its live parameters through
//! lock-free [`Shared`] atomics and writes only into pre-sized buffers — no
//! allocation, locking, or syscalls. [`AudioEffect::process`] feeds samples one
//! at a time through the node's `tick`, which is allocation-free.

use super::formant_preset::FormantPreset;
use super::AudioEffect;
use crate::audio::error::EffectsError;
use fundsp::prelude32::*;

/// FFT window length (power of two). 1024 samples ≈ 21.3 ms at 48 kHz, within
/// the <30 ms latency budget (issue #35). Also the effect's algorithmic latency.
const WINDOW: usize = 1024;

/// Linear pitch/formant ratio clamp range (one octave each way).
const MIN_RATIO: f32 = 0.5;
const MAX_RATIO: f32 = 2.0;

/// Semitone clamp range, matching `MIN_RATIO..MAX_RATIO`.
const MIN_SEMITONES: f32 = -12.0;
const MAX_SEMITONES: f32 = 12.0;

/// The resynth node's concrete type is unnameable (it embeds the closure type),
/// so we erase it behind fundsp's dynamic `AudioUnit` interface. `AudioUnit` is
/// `Send`, satisfying `AudioEffect: Send`.
type Node = Box<dyn AudioUnit>;

/// Formant-aware pitch shift effect. See module docs.
pub struct FormantPitchEffect {
    node: Node,
    /// Live pitch multiplier read by the RT closure.
    pitch: Shared,
    /// Live formant multiplier read by the RT closure.
    formant: Shared,
    sample_rate: u32,
    bypassed: bool,
}

impl FormantPitchEffect {
    /// Create a formant-pitch effect at `sample_rate`, no shift (ratios = 1.0),
    /// not bypassed. Allocates the fundsp node and all scratch up front.
    pub fn new(sample_rate: u32) -> Self {
        let pitch = shared(1.0);
        let formant = shared(1.0);
        let node: Node = Box::new(build_node(WINDOW, pitch.clone(), formant.clone()));
        let mut effect = Self {
            node,
            pitch,
            formant,
            sample_rate,
            bypassed: false,
        };
        effect.node.set_sample_rate(f64::from(sample_rate));
        effect
    }

    /// Current linear pitch multiplier.
    pub fn pitch_ratio(&self) -> f32 {
        self.pitch.value()
    }

    /// Current linear formant multiplier.
    pub fn formant_ratio(&self) -> f32 {
        self.formant.value()
    }

    /// Set the pitch multiplier (clamped to `[0.5, 2.0]`).
    pub fn set_pitch_ratio(&mut self, ratio: f32) {
        self.pitch.set(ratio.clamp(MIN_RATIO, MAX_RATIO));
    }

    /// Set the formant multiplier (clamped to `[0.5, 2.0]`).
    pub fn set_formant_ratio(&mut self, ratio: f32) {
        self.formant.set(ratio.clamp(MIN_RATIO, MAX_RATIO));
    }

    /// Set pitch by semitones (clamped to `[-12, 12]`), converted to a ratio.
    pub fn set_semitones(&mut self, semitones: f32) {
        let st = semitones.clamp(MIN_SEMITONES, MAX_SEMITONES);
        self.pitch.set(2.0_f32.powf(st / 12.0));
    }

    /// Apply a [`FormantPreset`], overwriting both ratios.
    pub fn apply_preset(&mut self, preset: FormantPreset) {
        self.set_pitch_ratio(preset.pitch_ratio());
        self.set_formant_ratio(preset.formant_ratio());
    }
}

/// Build the fundsp resynth node. The closure runs on the RT thread; it captures
/// `pitch`/`formant` `Shared` handles (lock-free reads) and pre-sized scratch
/// buffers (overwritten per window, never grown).
///
/// In this scaffold the closure forwards the input spectrum unchanged; Task 3
/// replaces the body with the formant-pitch DSP.
fn build_node(window: usize, _pitch: Shared, _formant: Shared) -> impl AudioUnit {
    resynth::<U1, U1, _>(window, move |fft| {
        for i in 0..fft.bins() {
            fft.set(0, i, fft.at(0, i));
        }
    })
}

impl AudioEffect for FormantPitchEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());
        if sample_rate != self.sample_rate {
            self.node.set_sample_rate(f64::from(sample_rate));
            self.sample_rate = sample_rate;
        }
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        let mut frame_out = [0.0_f32; 1];
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            self.node.tick(&[i], &mut frame_out);
            *o = frame_out[0];
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "pitch_ratio" | "pitch_factor" => {
                self.set_pitch_ratio(value);
                Ok(())
            }
            "semitones" => {
                self.set_semitones(value);
                Ok(())
            }
            "formant_ratio" | "formant_shift" => {
                self.set_formant_ratio(value);
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
        // Flush FFT/overlap state on any transition so audio captured before a
        // bypass window cannot leak out after it (mirrors PitchShiftEffect).
        if self.bypassed != bypass {
            self.node.reset();
        }
        self.bypassed = bypass;
    }

    fn latency_samples(&self) -> u32 {
        // resynth latency == one window length (see resynth.rs `latency()`).
        WINDOW as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn effect() -> FormantPitchEffect {
        FormantPitchEffect::new(SR)
    }

    #[test]
    fn new_defaults_to_unity_ratios() {
        let fx = effect();
        assert!((fx.pitch_ratio() - 1.0).abs() < 1e-6);
        assert!((fx.formant_ratio() - 1.0).abs() < 1e-6);
        assert!(!fx.bypass());
    }

    #[test]
    fn set_ratios_clamp_to_octave() {
        let mut fx = effect();
        fx.set_pitch_ratio(10.0);
        assert!((fx.pitch_ratio() - MAX_RATIO).abs() < 1e-6);
        fx.set_pitch_ratio(0.01);
        assert!((fx.pitch_ratio() - MIN_RATIO).abs() < 1e-6);
        fx.set_formant_ratio(10.0);
        assert!((fx.formant_ratio() - MAX_RATIO).abs() < 1e-6);
    }

    #[test]
    fn semitones_convert_to_ratio() {
        let mut fx = effect();
        fx.set_semitones(12.0);
        assert!((fx.pitch_ratio() - 2.0).abs() < 1e-5);
        fx.set_semitones(-12.0);
        assert!((fx.pitch_ratio() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn set_param_known_and_unknown() {
        let mut fx = effect();
        assert!(fx.set_param("pitch_ratio", 1.5).is_ok());
        assert!(fx.set_param("formant_ratio", 1.2).is_ok());
        assert!(fx.set_param("semitones", 7.0).is_ok());
        let err = fx.set_param("reverb", 0.5);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn bypass_is_exact_passthrough() {
        let mut fx = effect();
        fx.set_semitones(7.0);
        fx.set_bypass(true);
        let input: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut output = vec![0.0_f32; input.len()];
        fx.process(&input, &mut output, SR);
        assert_eq!(output, input);
    }

    #[test]
    fn latency_is_one_window() {
        assert_eq!(effect().latency_samples(), WINDOW as u32);
        // Budget check: < 30 ms at 48 kHz.
        assert!((WINDOW as f32 / SR as f32) < 0.030);
    }

    #[test]
    fn apply_preset_sets_both_ratios() {
        let mut fx = effect();
        fx.apply_preset(FormantPreset::GenderSwap);
        assert!((fx.pitch_ratio() - FormantPreset::GenderSwap.pitch_ratio()).abs() < 1e-6);
        assert!((fx.formant_ratio() - FormantPreset::GenderSwap.formant_ratio()).abs() < 1e-6);
    }

    #[test]
    fn process_output_is_finite_and_length_stable() {
        let mut fx = effect();
        fx.set_semitones(5.0);
        let input = vec![0.1_f32; 2048];
        let mut output = vec![0.0_f32; 2048];
        fx.process(&input, &mut output, SR);
        assert_eq!(output.len(), input.len());
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn composes_inside_effect_chain() {
        use crate::audio::effects::EffectChain;
        let block = 1024;
        let mut chain = EffectChain::new(block);
        let mut fx = effect();
        fx.set_semitones(7.0);
        chain.push_effect(Box::new(fx), block).unwrap();
        let input: Vec<f32> = (0..block)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / SR as f32).sin())
            .collect();
        let mut output = vec![0.0_f32; block];
        chain.process(&input, &mut output, SR);
        assert_eq!(output.len(), input.len());
        assert!(output.iter().all(|s| s.is_finite()));
    }
}
