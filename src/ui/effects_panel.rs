//! Voice-effects panel logic: presets, the UI-side state mirror, and the
//! mapping from a preset (or a single param edit) to the `AudioCommand`s sent to
//! the audio thread. The Iced view lives in [`super::effects_panel_view`]; this
//! module holds only the testable, render-free logic.

use crate::audio::effects::EffectSlot;
use crate::audio::AudioCommand;

/// A named voice-effect preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetId {
    Robot,
    Radio,
    Deep,
    Chipmunk,
    Custom,
}

impl PresetId {
    /// All presets, in display order.
    pub const ALL: [PresetId; 5] = [
        PresetId::Robot,
        PresetId::Radio,
        PresetId::Deep,
        PresetId::Chipmunk,
        PresetId::Custom,
    ];

    /// Short display name.
    pub fn label(self) -> &'static str {
        match self {
            PresetId::Robot => "Robot",
            PresetId::Radio => "Radio",
            PresetId::Deep => "Deep",
            PresetId::Chipmunk => "Chipmunk",
            PresetId::Custom => "Custom",
        }
    }

    /// One-line description shown under the name.
    pub fn description(self) -> &'static str {
        match self {
            PresetId::Robot => "Metallic ring-mod carrier",
            PresetId::Radio => "Bandpass + crackle",
            PresetId::Deep => "Lowered, ominous voice",
            PresetId::Chipmunk => "High, fast voice",
            PresetId::Custom => "All controls unlocked",
        }
    }

    /// Confetti-style glyph for the chip.
    pub fn glyph(self) -> &'static str {
        match self {
            PresetId::Robot => "\u{1F916}",    // robot
            PresetId::Radio => "\u{1F4FB}",    // radio
            PresetId::Deep => "\u{1F30A}",     // wave
            PresetId::Chipmunk => "\u{1F43F}", // chipmunk
            PresetId::Custom => "\u{1F39B}",   // control knobs
        }
    }
}

// Preset parameter constants (single source of truth for both commands + state).
const ROBOT_CARRIER_HZ: f32 = 150.0;
const RADIO_CENTER_HZ: f32 = 1500.0;
const RADIO_BANDWIDTH_HZ: f32 = 1200.0;
const RADIO_NOISE: f32 = 0.1;
const DEEP_SEMITONES: f32 = -5.0;
const CHIPMUNK_SEMITONES: f32 = 7.0;

/// UI-side mirror of the effect chain's user-facing state. Drives the view and
/// is updated by `apply_preset` / param edits. Defaults to `Custom`, all off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectsUiState {
    pub preset: PresetId,
    pub chain_bypass: bool,
    pub wet_dry: f32,
    pub pitch_semitones: f32,
    pub carrier_hz: f32,
    pub center_hz: f32,
    pub bandwidth_hz: f32,
    pub noise: f32,
}

impl Default for EffectsUiState {
    fn default() -> Self {
        Self {
            preset: PresetId::Custom,
            chain_bypass: false,
            wet_dry: 1.0,
            pitch_semitones: 0.0,
            carrier_hz: ROBOT_CARRIER_HZ,
            center_hz: RADIO_CENTER_HZ,
            bandwidth_hz: RADIO_BANDWIDTH_HZ,
            noise: RADIO_NOISE,
        }
    }
}

impl EffectsUiState {
    /// Update the displayed parameter values to match `preset`.
    pub fn apply_preset(&mut self, preset: PresetId) {
        self.preset = preset;
        match preset {
            PresetId::Robot => self.carrier_hz = ROBOT_CARRIER_HZ,
            PresetId::Radio => {
                self.center_hz = RADIO_CENTER_HZ;
                self.bandwidth_hz = RADIO_BANDWIDTH_HZ;
                self.noise = RADIO_NOISE;
            }
            PresetId::Deep => self.pitch_semitones = DEEP_SEMITONES,
            PresetId::Chipmunk => self.pitch_semitones = CHIPMUNK_SEMITONES,
            PresetId::Custom => {}
        }
    }
}

/// Build a `SetEffectParam` command for `slot`/`param`/`value`.
pub fn param_command(slot: EffectSlot, param: &str, value: f32) -> AudioCommand {
    AudioCommand::SetEffectParam {
        index: slot.index(),
        param: param.to_owned(),
        value,
    }
}

fn bypass_command(slot: EffectSlot, bypass: bool) -> AudioCommand {
    AudioCommand::SetEffectBypass {
        index: slot.index(),
        bypass,
    }
}

/// Store an edited parameter value into the UI state mirror.
pub fn store_effect_param(state: &mut EffectsUiState, slot: EffectSlot, param: &str, value: f32) {
    match (slot, param) {
        (EffectSlot::Pitch, "semitones") => state.pitch_semitones = value,
        (EffectSlot::RingMod, "carrier") => state.carrier_hz = value,
        (EffectSlot::Bandpass, "center") => state.center_hz = value,
        (EffectSlot::Bandpass, "bandwidth") => state.bandwidth_hz = value,
        (EffectSlot::Bandpass, "noise") => state.noise = value,
        _ => {}
    }
}

/// Full command set realizing `preset`: bypass every slot, then unbypass +
/// parameterize the ones the preset uses.
pub fn preset_commands(preset: PresetId) -> Vec<AudioCommand> {
    // Start from all-bypassed, then enable what the preset needs.
    let mut cmds: Vec<AudioCommand> = EffectSlot::ORDER
        .iter()
        .map(|&slot| bypass_command(slot, true))
        .collect();

    match preset {
        PresetId::Robot => {
            cmds.push(bypass_command(EffectSlot::RingMod, false));
            cmds.push(param_command(
                EffectSlot::RingMod,
                "carrier",
                ROBOT_CARRIER_HZ,
            ));
        }
        PresetId::Radio => {
            cmds.push(bypass_command(EffectSlot::Bandpass, false));
            cmds.push(param_command(
                EffectSlot::Bandpass,
                "center",
                RADIO_CENTER_HZ,
            ));
            cmds.push(param_command(
                EffectSlot::Bandpass,
                "bandwidth",
                RADIO_BANDWIDTH_HZ,
            ));
            cmds.push(param_command(EffectSlot::Bandpass, "noise", RADIO_NOISE));
        }
        PresetId::Deep => {
            cmds.push(bypass_command(EffectSlot::Pitch, false));
            cmds.push(param_command(
                EffectSlot::Pitch,
                "semitones",
                DEEP_SEMITONES,
            ));
        }
        PresetId::Chipmunk => {
            cmds.push(bypass_command(EffectSlot::Pitch, false));
            cmds.push(param_command(
                EffectSlot::Pitch,
                "semitones",
                CHIPMUNK_SEMITONES,
            ));
        }
        PresetId::Custom => {} // all bypassed; user unlocks via sliders
    }
    cmds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{effects::EffectSlot, AudioCommand};

    fn has_unbypass(cmds: &[AudioCommand], slot: EffectSlot) -> bool {
        cmds.iter().any(|c| {
            matches!(c,
                AudioCommand::SetEffectBypass { index, bypass: false } if *index == slot.index())
        })
    }
    fn has_bypass(cmds: &[AudioCommand], slot: EffectSlot) -> bool {
        cmds.iter().any(|c| {
            matches!(c,
                AudioCommand::SetEffectBypass { index, bypass: true } if *index == slot.index())
        })
    }
    fn param_value(cmds: &[AudioCommand], slot: EffectSlot, p: &str) -> Option<f32> {
        cmds.iter().find_map(|c| match c {
            AudioCommand::SetEffectParam {
                index,
                param,
                value,
            } if *index == slot.index() && param == p => Some(*value),
            _ => None,
        })
    }

    #[test]
    fn robot_enables_only_ring_mod_at_150hz() {
        let cmds = preset_commands(PresetId::Robot);
        assert!(has_unbypass(&cmds, EffectSlot::RingMod));
        assert!(has_bypass(&cmds, EffectSlot::Pitch));
        assert!(has_bypass(&cmds, EffectSlot::Bandpass));
        assert_eq!(
            param_value(&cmds, EffectSlot::RingMod, "carrier"),
            Some(150.0)
        );
    }

    #[test]
    fn radio_enables_bandpass_center_1500_with_noise() {
        let cmds = preset_commands(PresetId::Radio);
        assert!(has_unbypass(&cmds, EffectSlot::Bandpass));
        assert!(has_bypass(&cmds, EffectSlot::RingMod));
        assert_eq!(
            param_value(&cmds, EffectSlot::Bandpass, "center"),
            Some(1500.0)
        );
        assert_eq!(param_value(&cmds, EffectSlot::Bandpass, "noise"), Some(0.1));
    }

    #[test]
    fn deep_enables_pitch_down_only() {
        let cmds = preset_commands(PresetId::Deep);
        assert!(has_unbypass(&cmds, EffectSlot::Pitch));
        assert!(has_bypass(&cmds, EffectSlot::RingMod));
        assert!(has_bypass(&cmds, EffectSlot::Bandpass));
        let semis = param_value(&cmds, EffectSlot::Pitch, "semitones").expect("semitones param");
        assert!(semis < 0.0, "deep voice shifts pitch down, got {semis}");
    }

    #[test]
    fn chipmunk_enables_pitch_up_only() {
        let cmds = preset_commands(PresetId::Chipmunk);
        let semis = param_value(&cmds, EffectSlot::Pitch, "semitones").expect("semitones param");
        assert!(semis > 0.0, "chipmunk shifts pitch up, got {semis}");
    }

    #[test]
    fn custom_bypasses_all_effects() {
        let cmds = preset_commands(PresetId::Custom);
        for slot in EffectSlot::ORDER {
            assert!(
                has_bypass(&cmds, slot),
                "custom starts with {slot:?} bypassed"
            );
        }
    }

    #[test]
    fn apply_preset_updates_state_fields() {
        let mut state = EffectsUiState::default();
        state.apply_preset(PresetId::Robot);
        assert_eq!(state.preset, PresetId::Robot);
        assert_eq!(state.carrier_hz, 150.0);
    }

    #[test]
    fn store_effect_param_mirrors_value() {
        let mut state = EffectsUiState::default();
        store_effect_param(&mut state, EffectSlot::Pitch, "semitones", -3.0);
        assert_eq!(state.pitch_semitones, -3.0);
        store_effect_param(&mut state, EffectSlot::Bandpass, "noise", 0.5);
        assert_eq!(state.noise, 0.5);
    }

    #[test]
    fn all_presets_listed_with_labels() {
        assert_eq!(PresetId::ALL.len(), 5);
        for p in PresetId::ALL {
            assert!(!p.label().is_empty());
            assert!(!p.description().is_empty());
            assert!(!p.glyph().is_empty());
        }
    }
}
