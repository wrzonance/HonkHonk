pub mod chain;
pub mod commands;

pub use chain::EffectChain;
pub use commands::{EffectsCommand, EffectsEvent};

/// A real-time audio processing unit. All methods that run inside the PipeWire
/// process callback MUST be real-time safe: no allocation, no locks, no syscalls.
pub trait AudioEffect: Send {
    /// Process a block of audio. `input` and `output` have equal length.
    /// Called on the PipeWire thread — must be real-time safe.
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32);

    /// Set a named parameter. `value` is normalized to the parameter's natural range.
    /// Called from the command handler, not the process callback.
    fn set_param(&mut self, param: &str, value: f32);

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
        fn set_param(&mut self, _param: &str, _value: f32) {}
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
}
