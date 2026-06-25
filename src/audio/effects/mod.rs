pub mod chain;
pub mod chorus;
pub mod commands;
pub mod filter;
pub mod flanger;
pub mod formant;
pub mod formant_dsp;
pub mod formant_preset;
pub mod layout;
pub mod modulation;
pub mod pitch;
pub mod preset;
pub mod reverb;
pub mod settings;

use crate::audio::error::EffectsError;
pub use chain::EffectChain;
pub use chorus::Chorus;
pub use commands::{EffectsCommand, EffectsEvent};
pub use filter::BandpassFilterEffect;
pub use flanger::Flanger;
pub use formant::FormantPitchEffect;
pub use formant_preset::FormantPreset;
pub use layout::{EffectSlot, default_chain};
pub use modulation::RingModEffect;
pub use pitch::PitchShiftEffect;
pub use preset::PitchPreset;
pub use reverb::Reverb;
pub use settings::{
    BandpassEffectSettings, EffectSettings, PitchEffectSettings, RingModEffectSettings,
};

/// A real-time audio processing unit. All methods that run inside the PipeWire
/// process callback MUST be real-time safe: no allocation, no locks, no syscalls.
pub trait AudioEffect: Send {
    /// Process a block of audio. `input` and `output` have equal length.
    /// Called on the PipeWire thread — must be real-time safe.
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32);

    /// Set a named parameter. `value` is normalized to the parameter's natural range.
    /// Called from the command handler, not the process callback.
    ///
    /// Returns `Err(EffectsError::ParamUnknown)` if `param` is not recognised
    /// by this effect implementation.
    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError>;

    /// Returns `true` if this effect is currently bypassed.
    fn bypass(&self) -> bool;

    /// Enable or disable bypass for this effect.
    fn set_bypass(&mut self, bypass: bool);

    /// Algorithmic latency introduced by this effect, in samples.
    fn latency_samples(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PassThrough {
        bypassed: bool,
    }

    impl AudioEffect for PassThrough {
        fn process(&mut self, input: &[f32], output: &mut [f32], _sample_rate: u32) {
            output.copy_from_slice(input);
        }
        fn set_param(&mut self, _param: &str, _value: f32) -> Result<(), EffectsError> {
            Ok(())
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

    #[test]
    fn audio_effect_pass_through_copies_input() {
        let mut effect = PassThrough { bypassed: false };
        let input = vec![0.1_f32, 0.2, 0.3, 0.4];
        let mut output = vec![0.0_f32; 4];
        effect.process(&input, &mut output, 48000);
        assert_eq!(output, input);
    }

    #[test]
    fn audio_effect_bypass_toggle() {
        let mut effect = PassThrough { bypassed: false };
        assert!(!effect.bypass());
        effect.set_bypass(true);
        assert!(effect.bypass());
        effect.set_bypass(false);
        assert!(!effect.bypass());
    }

    #[test]
    fn audio_effect_latency_samples_default_zero() {
        let effect = PassThrough { bypassed: false };
        assert_eq!(effect.latency_samples(), 0);
    }

    /// Composability (issue #34): reverb, chorus and flanger must stack in an
    /// `EffectChain` and produce a finite, processed signal.
    #[test]
    fn time_effects_compose_in_chain() {
        let block = 1024usize;
        let mut chain = EffectChain::new(block);
        chain
            .push_effect(Box::new(Chorus::thick_voice()), block)
            .expect("push chorus");
        chain
            .push_effect(Box::new(Flanger::jet()), block)
            .expect("push flanger");
        chain
            .push_effect(Box::new(Reverb::cathedral()), block)
            .expect("push reverb");

        let input: Vec<f32> = (0..block)
            .map(|n| (std::f32::consts::TAU * 220.0 * n as f32 / 48_000.0).sin())
            .collect();
        let mut output = vec![0.0_f32; block];
        chain.process(&input, &mut output, 48_000);

        assert!(
            output.iter().all(|s| s.is_finite()),
            "stacked time-based effects must stay finite"
        );
        let diff = input
            .iter()
            .zip(output.iter())
            .map(|(i, o)| (i - o).abs())
            .fold(0.0_f32, f32::max);
        assert!(diff > 1e-3, "stacked effects should alter the signal");
    }
}
