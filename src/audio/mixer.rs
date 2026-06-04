//! Mixer: integration seam between mic capture and virtual sink write.
//!
//! In the current architecture (ADR-007: Links-Only routing), HonkHonk uses
//! PipeWire graph links for mic routing — there is no application-level mic
//! capture buffer. This module is the integration stub for Phase 4B voice
//! effects. When effects are wired in (PR 2+), a `pw_stream` process callback
//! will call [`Mixer::process_block`] on each captured mic buffer before writing
//! it to the virtual sink.
//!
//! Until then, `process_block` is a transparent passthrough.

use crate::audio::effects::EffectChain;

/// Holds the effect chain and applies it to mic audio blocks.
///
/// Instantiated once per audio engine session. The [`EffectChain`] inside
/// is populated by [`crate::audio::engine`] in response to `EffectsCommand`s.
pub struct Mixer {
    chain: EffectChain,
    /// Pre-allocated output buffer for `process_block`. Avoids per-call alloc.
    output_buf: Vec<f32>,
    output_capacity: usize,
}

impl Mixer {
    /// Create a new `Mixer` with an empty effect chain.
    pub fn new(initial_block_size: usize) -> Self {
        Self {
            chain: EffectChain::new(initial_block_size),
            output_buf: vec![0.0_f32; initial_block_size],
            output_capacity: initial_block_size,
        }
    }

    /// Returns a mutable reference to the effect chain.
    ///
    /// Used by `engine.rs` to apply `EffectsCommand`s.
    pub fn chain_mut(&mut self) -> &mut EffectChain {
        &mut self.chain
    }

    /// Process a block of mic audio through the effect chain.
    ///
    /// Returns a slice into the internal output buffer. Caller copies this
    /// to the virtual sink's input buffer.
    ///
    /// No-op passthrough when chain is empty or bypassed. Real-time safe.
    /// Prepare `Mixer` to handle blocks up to `required_capacity` samples.
    ///
    /// Call this on a cold (non-RT) path before the PipeWire process callback
    /// starts delivering blocks of the given size. Must not be called from the
    /// audio thread.
    pub fn ensure_output_capacity(&mut self, required_capacity: usize) {
        if required_capacity > self.output_capacity {
            self.output_buf.resize(required_capacity, 0.0);
            self.output_capacity = required_capacity;
        }
    }

    /// Process a block of mic audio through the effect chain.
    ///
    /// Returns a slice into the internal output buffer. Caller copies this
    /// to the virtual sink's input buffer.
    ///
    /// If the block size exceeds the pre-allocated capacity, resizes the output
    /// buffer (allocation). To avoid allocation on the RT thread, call
    /// [`ensure_output_capacity`] with the maximum expected block size before
    /// the audio callback starts delivering audio.
    pub fn process_block(&mut self, input: &[f32], sample_rate: u32) -> &[f32] {
        let n = input.len();
        if n > self.output_capacity {
            // Allocation fallback — avoid by calling ensure_output_capacity()
            // on a cold path before the audio callback runs.
            self.ensure_output_capacity(n);
        }
        self.chain
            .process(input, &mut self.output_buf[..n], sample_rate);
        &self.output_buf[..n]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_new_is_empty_passthrough() {
        let mut mixer = Mixer::new(64);
        let input = vec![0.25_f32; 64];
        let output = mixer.process_block(&input, 48000);
        assert_eq!(output, input.as_slice());
    }

    #[test]
    fn mixer_chain_mut_accessible() {
        let mut mixer = Mixer::new(64);
        let chain = mixer.chain_mut();
        assert!(chain.is_empty());
    }

    #[test]
    fn mixer_process_block_returns_correct_length() {
        let mut mixer = Mixer::new(64);
        let input = vec![1.0_f32; 32];
        let output = mixer.process_block(&input, 48000);
        assert_eq!(output.len(), 32);
    }
}
