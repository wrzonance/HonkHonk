//! Fixed effect-chain layout shared by the audio engine and the effects UI.
//!
//! The runtime [`EffectChain`](super::EffectChain) addresses effects by index.
//! For UI-emitted commands (`AudioCommand::SetEffect*`) to be meaningful, the
//! engine and the UI must agree on which effect lives at which index. This
//! module is that single source of truth: [`EffectSlot::ORDER`] defines the
//! chain order, and [`default_chain`] builds it (all effects start bypassed so
//! a fresh session is a clean passthrough until the user enables an effect).

use super::{AudioEffect, BandpassFilterEffect, PitchShiftEffect, RingModEffect};

/// A fixed position in the effect chain. Index == position in [`EffectSlot::ORDER`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectSlot {
    /// Pitch shift (semitones / factor). Index 0.
    Pitch,
    /// Ring modulator (robotic carrier). Index 1.
    RingMod,
    /// Bandpass + crackle ("radio"). Index 2.
    Bandpass,
}

impl EffectSlot {
    /// Chain order. The slice index of each variant is its chain index.
    pub const ORDER: [EffectSlot; 3] =
        [EffectSlot::Pitch, EffectSlot::RingMod, EffectSlot::Bandpass];

    /// Stable chain index for this slot.
    ///
    /// An exhaustive `match` (not an `ORDER` lookup with a fallback): the
    /// compiler forces this to stay in sync with `ORDER`, so a future drift is
    /// a build error rather than a silent mis-route to index 0.
    pub fn index(self) -> usize {
        match self {
            EffectSlot::Pitch => 0,
            EffectSlot::RingMod => 1,
            EffectSlot::Bandpass => 2,
        }
    }

    /// Human-readable label for UI.
    pub fn label(self) -> &'static str {
        match self {
            EffectSlot::Pitch => "Pitch",
            EffectSlot::RingMod => "Ring Mod",
            EffectSlot::Bandpass => "Radio",
        }
    }

    /// Construct the concrete effect for this slot.
    pub fn build(self, block_size: usize, sample_rate: u32) -> Box<dyn AudioEffect> {
        match self {
            EffectSlot::Pitch => Box::new(PitchShiftEffect::new(sample_rate)),
            EffectSlot::RingMod => Box::new(RingModEffect::new(block_size)),
            EffectSlot::Bandpass => Box::new(BandpassFilterEffect::new(block_size)),
        }
    }
}

/// Build the full default chain in [`EffectSlot::ORDER`], every effect bypassed.
pub fn default_chain(block_size: usize, sample_rate: u32) -> Vec<Box<dyn AudioEffect>> {
    EffectSlot::ORDER
        .iter()
        .map(|slot| {
            let mut fx = slot.build(block_size, sample_rate);
            fx.set_bypass(true);
            fx
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_indices_are_stable_and_contiguous() {
        assert_eq!(EffectSlot::Pitch.index(), 0);
        assert_eq!(EffectSlot::RingMod.index(), 1);
        assert_eq!(EffectSlot::Bandpass.index(), 2);
        for (i, slot) in EffectSlot::ORDER.iter().enumerate() {
            assert_eq!(slot.index(), i);
        }
    }

    #[test]
    fn default_chain_has_one_effect_per_slot_all_bypassed() {
        let chain = default_chain(4096, 48_000);
        assert_eq!(chain.len(), EffectSlot::ORDER.len());
        assert!(
            chain.iter().all(|e| e.bypass()),
            "all effects start bypassed"
        );
    }

    #[test]
    fn labels_are_human_readable() {
        assert_eq!(EffectSlot::Pitch.label(), "Pitch");
        assert_eq!(EffectSlot::RingMod.label(), "Ring Mod");
        assert_eq!(EffectSlot::Bandpass.label(), "Radio");
    }
}
