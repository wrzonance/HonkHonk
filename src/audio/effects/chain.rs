use super::AudioEffect;
use crate::audio::error::EffectsError;

/// Maximum number of effects in a chain. Prevents unbounded growth.
pub const MAX_CHAIN_LEN: usize = 16;

/// A composable, ordered pipeline of [`AudioEffect`]s.
///
/// # Real-time Safety
/// `process()` is real-time safe: no allocation, no locking, no syscalls.
/// The internal scratch buffer is pre-allocated on `new()` and resized only
/// in `push_effect()` (cold path).
pub struct EffectChain {
    effects: Vec<Box<dyn AudioEffect>>,
    /// Pre-allocated scratch buffer. Sized to `scratch_capacity` f32 values.
    /// Used as intermediate buffer between effect stages.
    scratch: Vec<f32>,
    scratch_capacity: usize,
    /// Chain-level wet/dry: 0.0 = dry (input passthrough), 1.0 = fully wet.
    wet_dry: f32,
    /// When `true`, the entire chain is bypassed regardless of per-effect bypass.
    chain_bypass: bool,
}

impl EffectChain {
    /// Create a new empty `EffectChain` with a scratch buffer sized for
    /// `initial_block_size` mono samples.
    pub fn new(initial_block_size: usize) -> Self {
        Self {
            effects: Vec::new(),
            scratch: vec![0.0_f32; initial_block_size],
            scratch_capacity: initial_block_size,
            wet_dry: 1.0,
            chain_bypass: false,
        }
    }

    /// Add an effect to the end of the chain.
    ///
    /// Grows the scratch buffer if needed (cold path, may allocate).
    /// Returns `Err(EffectsError::ChainTooLong)` if chain is at capacity.
    pub fn push_effect(
        &mut self,
        effect: Box<dyn AudioEffect>,
        block_size: usize,
    ) -> Result<(), EffectsError> {
        if self.effects.len() >= MAX_CHAIN_LEN {
            return Err(EffectsError::ChainTooLong {
                max: MAX_CHAIN_LEN,
                got: self.effects.len() + 1,
            });
        }
        self.effects.push(effect);
        if block_size > self.scratch_capacity {
            self.scratch.resize(block_size, 0.0);
            self.scratch_capacity = block_size;
        }
        Ok(())
    }

    /// Remove the effect at `index`.
    ///
    /// Returns `Err(EffectsError::IndexOutOfRange)` if index is out of bounds.
    pub fn remove_effect(&mut self, index: usize) -> Result<(), EffectsError> {
        if index >= self.effects.len() {
            return Err(EffectsError::IndexOutOfRange {
                index,
                len: self.effects.len(),
            });
        }
        self.effects.remove(index);
        Ok(())
    }

    /// Set the chain-level wet/dry mix. Clamped to `[0.0, 1.0]`.
    pub fn set_wet_dry(&mut self, wet_dry: f32) {
        self.wet_dry = wet_dry.clamp(0.0, 1.0);
    }

    /// Returns the current wet/dry ratio.
    pub fn wet_dry(&self) -> f32 {
        self.wet_dry
    }

    /// Set the chain-level bypass. When `true`, all effects are skipped.
    pub fn set_chain_bypass(&mut self, bypass: bool) {
        self.chain_bypass = bypass;
    }

    /// Returns `true` if the chain is bypassed (all effects skipped).
    pub fn chain_bypass(&self) -> bool {
        self.chain_bypass
    }

    /// Returns `true` if all individual effects are bypassed (or chain is empty).
    pub fn all_effects_bypassed(&self) -> bool {
        self.effects.is_empty() || self.effects.iter().all(|e| e.bypass())
    }

    /// Returns the number of effects in the chain.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Returns `true` if the chain has no effects.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Total algorithmic latency of all non-bypassed effects, in samples.
    pub fn total_latency_samples(&self) -> u32 {
        if self.chain_bypass {
            return 0;
        }
        self.effects
            .iter()
            .filter(|e| !e.bypass())
            .map(|e| e.latency_samples())
            .fold(0u32, |acc, l| acc.saturating_add(l))
    }

    /// Set bypass on the effect at `index`.
    pub fn set_bypass(&mut self, index: usize, bypass: bool) -> Result<(), EffectsError> {
        let len = self.effects.len();
        self.effects
            .get_mut(index)
            .ok_or(EffectsError::IndexOutOfRange { index, len })
            .map(|e| e.set_bypass(bypass))
    }

    /// Set a parameter on the effect at `index`.
    ///
    /// Returns `Err(EffectsError::IndexOutOfRange)` if `index` is out of bounds,
    /// or `Err(EffectsError::ParamUnknown)` if the effect rejects the parameter.
    pub fn set_param(&mut self, index: usize, param: &str, value: f32) -> Result<(), EffectsError> {
        let len = self.effects.len();
        let effect = self
            .effects
            .get_mut(index)
            .ok_or(EffectsError::IndexOutOfRange { index, len })?;
        effect.set_param(param, value)
    }

    /// Process a block of audio.
    ///
    /// If the chain is bypassed, all effects are bypassed, or there are no
    /// effects, copies `input` to `output` directly (passthrough).
    ///
    /// Otherwise, runs each non-bypassed effect in sequence using a ping-pong
    /// strategy between `output` and the pre-allocated scratch buffer.
    /// Applies the wet/dry mix at the end.
    ///
    /// `input` and `output` must have equal length. Does not allocate.
    pub fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());

        if self.chain_bypass || self.all_effects_bypassed() {
            output.copy_from_slice(input);
            return;
        }

        let n = input.len();
        // Guard: if block is larger than pre-allocated scratch, fall back to
        // passthrough rather than allocating on the RT thread. Callers must
        // call push_effect() (or resize via a cold-path API) before processing
        // larger blocks. This upholds the real-time safety contract.
        if n > self.scratch_capacity {
            output.copy_from_slice(input);
            return;
        }

        // Copy input → scratch as the initial working buffer.
        self.scratch[..n].copy_from_slice(&input[..n]);

        for effect in &mut self.effects {
            if effect.bypass() {
                continue;
            }
            // Copy scratch (current result) → output, then run effect:
            // effect reads from output, writes back to scratch.
            // This ping-pong pattern avoids any allocation.
            output[..n].copy_from_slice(&self.scratch[..n]);
            effect.process(&output[..n], &mut self.scratch[..n], sample_rate);
        }

        // Final result lives in scratch — copy to output.
        output[..n].copy_from_slice(&self.scratch[..n]);

        // Apply wet/dry mix if not fully wet.
        if (self.wet_dry - 1.0_f32).abs() > f32::EPSILON {
            let wet = self.wet_dry;
            let dry = 1.0 - wet;
            for (i, out) in output[..n].iter_mut().enumerate() {
                *out = dry * input[i] + wet * (*out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock effect that scales all samples by `gain`.
    struct GainEffect {
        gain: f32,
        bypassed: bool,
    }

    impl GainEffect {
        fn new(gain: f32) -> Box<Self> {
            Box::new(Self {
                gain,
                bypassed: false,
            })
        }
    }

    impl AudioEffect for GainEffect {
        fn process(&mut self, input: &[f32], output: &mut [f32], _sample_rate: u32) {
            for (o, &i) in output.iter_mut().zip(input.iter()) {
                *o = i * self.gain;
            }
        }
        fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
            if param == "gain" {
                self.gain = value;
                Ok(())
            } else {
                Err(EffectsError::ParamUnknown {
                    param: param.to_owned(),
                })
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

    /// Mock effect with 512-sample latency.
    struct LatencyEffect {
        bypassed: bool,
    }

    impl AudioEffect for LatencyEffect {
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
            512
        }
    }

    fn make_chain() -> EffectChain {
        EffectChain::new(1024)
    }

    #[test]
    fn empty_chain_passes_through() {
        let mut chain = make_chain();
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        assert_eq!(output, input);
    }

    #[test]
    fn single_gain_effect_doubles_signal() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(2.0), 64).unwrap();
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        for &s in &output {
            assert!((s - 1.0).abs() < 1e-6, "expected 1.0, got {s}");
        }
    }

    #[test]
    fn chained_gains_multiply() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(2.0), 64).unwrap();
        chain.push_effect(GainEffect::new(3.0), 64).unwrap();
        let input = vec![1.0_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        for &s in &output {
            assert!((s - 6.0).abs() < 1e-5, "expected 6.0, got {s}");
        }
    }

    #[test]
    fn bypassed_effect_is_skipped() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(2.0), 64).unwrap();
        chain.set_bypass(0, true).unwrap();
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        assert_eq!(output, input);
    }

    #[test]
    fn chain_bypass_skips_all_effects() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(100.0), 64).unwrap();
        chain.set_chain_bypass(true);
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        assert_eq!(output, input);
    }

    #[test]
    fn wet_dry_zero_is_fully_dry() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(100.0), 64).unwrap();
        chain.set_wet_dry(0.0);
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        for (o, i) in output.iter().zip(input.iter()) {
            assert!((o - i).abs() < 1e-5, "expected {i}, got {o}");
        }
    }

    #[test]
    fn wet_dry_half_mixes_equally() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(3.0), 64).unwrap();
        chain.set_wet_dry(0.5);
        let input = vec![1.0_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        // 0.5 * 1.0 (dry) + 0.5 * 3.0 (wet) = 2.0
        for &s in &output {
            assert!((s - 2.0).abs() < 1e-5, "expected 2.0, got {s}");
        }
    }

    #[test]
    fn chain_too_long_returns_error() {
        let mut chain = make_chain();
        for _ in 0..MAX_CHAIN_LEN {
            chain.push_effect(GainEffect::new(1.0), 64).unwrap();
        }
        let result = chain.push_effect(GainEffect::new(1.0), 64);
        assert!(matches!(result, Err(EffectsError::ChainTooLong { .. })));
    }

    #[test]
    fn remove_effect_out_of_range_returns_error() {
        let mut chain = make_chain();
        let result = chain.remove_effect(0);
        assert!(matches!(result, Err(EffectsError::IndexOutOfRange { .. })));
    }

    #[test]
    fn remove_effect_shrinks_chain() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(2.0), 64).unwrap();
        chain.push_effect(GainEffect::new(3.0), 64).unwrap();
        chain.remove_effect(0).unwrap();
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn total_latency_sums_non_bypassed() {
        let mut chain = make_chain();
        chain
            .push_effect(Box::new(LatencyEffect { bypassed: false }), 64)
            .unwrap();
        chain
            .push_effect(Box::new(LatencyEffect { bypassed: true }), 64)
            .unwrap();
        assert_eq!(chain.total_latency_samples(), 512);
    }

    #[test]
    fn total_latency_zero_when_chain_bypassed() {
        let mut chain = make_chain();
        chain
            .push_effect(Box::new(LatencyEffect { bypassed: false }), 64)
            .unwrap();
        chain.set_chain_bypass(true);
        assert_eq!(chain.total_latency_samples(), 0);
    }

    #[test]
    fn all_effects_bypassed_true_when_empty() {
        let chain = make_chain();
        assert!(chain.all_effects_bypassed());
    }

    #[test]
    fn all_effects_bypassed_false_when_one_active() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(1.0), 64).unwrap();
        assert!(!chain.all_effects_bypassed());
    }

    #[test]
    fn set_param_out_of_range_returns_error() {
        let mut chain = make_chain();
        let result = chain.set_param(0, "gain", 1.0);
        assert!(matches!(result, Err(EffectsError::IndexOutOfRange { .. })));
    }
}
