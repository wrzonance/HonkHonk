use serde::{Deserialize, Serialize};

use super::{EffectChain, EffectSlot, default_chain};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PitchEffectSettings {
    pub bypass: bool,
    pub semitones: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RingModEffectSettings {
    pub bypass: bool,
    pub carrier_hz: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BandpassEffectSettings {
    pub bypass: bool,
    pub center_hz: f32,
    pub bandwidth_hz: f32,
    pub noise: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EffectSettings {
    pub chain_bypass: bool,
    pub wet_dry: f32,
    pub pitch: PitchEffectSettings,
    pub ring_mod: RingModEffectSettings,
    pub bandpass: BandpassEffectSettings,
}

impl Default for EffectSettings {
    fn default() -> Self {
        Self {
            chain_bypass: false,
            wet_dry: 1.0,
            pitch: PitchEffectSettings {
                bypass: true,
                semitones: 0.0,
            },
            ring_mod: RingModEffectSettings {
                bypass: true,
                carrier_hz: 150.0,
            },
            bandpass: BandpassEffectSettings {
                bypass: true,
                center_hz: 1500.0,
                bandwidth_hz: 1200.0,
                noise: 0.1,
            },
        }
    }
}

impl EffectSettings {
    pub fn build_chain(self, block_size: usize, sample_rate: u32) -> EffectChain {
        let mut chain = EffectChain::new(block_size);
        for effect in default_chain(block_size, sample_rate) {
            if chain.push_effect(effect, block_size).is_err() {
                return chain;
            }
        }
        self.apply_to_chain(&mut chain);
        chain
    }

    fn apply_to_chain(self, chain: &mut EffectChain) {
        chain.set_chain_bypass(self.chain_bypass);
        chain.set_wet_dry(self.wet_dry);
        self.apply_pitch(chain);
        self.apply_ring_mod(chain);
        self.apply_bandpass(chain);
    }

    fn apply_pitch(self, chain: &mut EffectChain) {
        let index = EffectSlot::Pitch.index();
        let _ = chain.set_bypass(index, self.pitch.bypass);
        let _ = chain.set_param(index, "semitones", self.pitch.semitones);
    }

    fn apply_ring_mod(self, chain: &mut EffectChain) {
        let index = EffectSlot::RingMod.index();
        let _ = chain.set_bypass(index, self.ring_mod.bypass);
        let _ = chain.set_param(index, "carrier", self.ring_mod.carrier_hz);
    }

    fn apply_bandpass(self, chain: &mut EffectChain) {
        let index = EffectSlot::Bandpass.index();
        let _ = chain.set_bypass(index, self.bandpass.bypass);
        let _ = chain.set_param(index, "center", self.bandpass.center_hz);
        let _ = chain.set_param(index, "bandwidth", self.bandpass.bandwidth_hz);
        let _ = chain.set_param(index, "noise", self.bandpass.noise);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_build_bypassed_chain() {
        let chain = EffectSettings::default().build_chain(128, 48_000);
        assert_eq!(chain.len(), EffectSlot::ORDER.len());
        assert!(chain.all_effects_bypassed());
    }

    #[test]
    fn active_pitch_setting_unbypasses_pitch_slot_only() {
        let settings = EffectSettings {
            pitch: PitchEffectSettings {
                bypass: false,
                semitones: 7.0,
            },
            ..EffectSettings::default()
        };
        let chain = settings.build_chain(128, 48_000);
        assert!(!chain.all_effects_bypassed());
        assert_eq!(chain.total_latency_samples(), 1024);
    }
}
