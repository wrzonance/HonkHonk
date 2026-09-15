use super::*;
use crate::audio::effects::EffectSettings;
use crate::state::Step;
use crate::ui::effects_panel::{self, EffectsUiState, PresetId};
use crate::ui::macros::geometry::time_at;

#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    Add(PathBuf, u64),
    Move(usize, u64),
    Duplicate(usize),
    Remove(usize),
    Gain(usize, f32),
    Effects(usize, EffectSettings),
}

impl Edit {
    fn apply(self, steps: &mut Vec<Step>) {
        match self {
            Self::Add(path, time) => steps.push(Step::new(path, time)),
            Self::Move(i, time) => {
                if let Some(step) = steps.get_mut(i) {
                    step.start_offset_ms = time;
                }
            }
            Self::Duplicate(i) => {
                if let Some(step) = steps.get(i).cloned() {
                    steps.push(step);
                }
            }
            Self::Remove(i) => {
                if i < steps.len() {
                    steps.remove(i);
                }
            }
            Self::Gain(i, gain) => {
                if gain.is_finite()
                    && let Some(step) = steps.get_mut(i)
                {
                    step.gain = gain.clamp(0.0, 2.0);
                }
            }
            Self::Effects(i, effects) => {
                if let Some(step) = steps.get_mut(i) {
                    step.effects = effects;
                }
            }
        }
    }
}

impl HonkHonk {
    pub(super) fn edit_macro(&mut self, edit: Edit) {
        self.cancel_macro();
        let Some(id) = self.macro_editor.active.clone() else {
            return;
        };
        if matches!(edit, Edit::Remove(_)) {
            self.macro_editor.menu = None;
        }
        if let Some(value) = self.macros.get_mut(&id) {
            edit.apply(&mut value.steps);
        }
        self.persist_macros();
        self.tick_macro_preview(Instant::now());
    }

    pub(super) fn release_macro_drag(&mut self, point: Option<Point>) {
        let drag = self.macro_editor.dragging.take();
        self.macro_editor.pointer = None;
        let Some(point) = point else {
            return;
        };
        let scale = self.macro_editor.timeline.scale;
        let snap = self.macro_editor.snap;
        match drag {
            Some(Drag::Sound(path)) => {
                self.edit_macro(Edit::Add(path, time_at(point.x, 0.0, scale, snap)))
            }
            Some(Drag::Step { index, grab }) => {
                let Some(bar) = self.macro_editor.timeline.bars.get(index) else {
                    return;
                };
                let pressed_x = (bar.start as f64 * scale) as f32 + grab;
                // A click (including subpixel rounding noise) is not an edit.
                // Compare before snapping so off-grid recorded offsets survive.
                if (point.x - pressed_x).abs() > 0.5 {
                    self.edit_macro(Edit::Move(index, time_at(point.x, grab, scale, snap)));
                }
            }
            None => {}
        }
    }

    pub(super) fn open_step_menu(&mut self, index: usize) {
        let step = self
            .macro_editor
            .active
            .as_deref()
            .and_then(|id| self.macros.get(id))
            .and_then(|m| m.steps.get(index));
        let Some(step) = step else {
            return;
        };
        let e = step.effects;
        self.macro_editor.effects = EffectsUiState {
            preset: PresetId::Custom,
            chain_bypass: e.chain_bypass,
            wet_dry: e.wet_dry,
            pitch_semitones: e.pitch.semitones,
            pitch_bypass: e.pitch.bypass,
            carrier_hz: e.ring_mod.carrier_hz,
            ring_mod_bypass: e.ring_mod.bypass,
            center_hz: e.bandpass.center_hz,
            bandwidth_hz: e.bandpass.bandwidth_hz,
            noise: e.bandpass.noise,
            bandpass_bypass: e.bandpass.bypass,
        };
        self.macro_editor.menu = Some(index);
        self.macro_editor.dragging = None;
        self.macro_editor.pointer = None;
    }

    pub(super) fn edit_step_effects(&mut self, message: Message) {
        let Some(index) = self.macro_editor.menu else {
            return;
        };
        let state = &mut self.macro_editor.effects;
        match message {
            Message::SelectEffectPreset(preset) => state.apply_preset(preset),
            Message::SetEffectBypassUi(value) => state.chain_bypass = value,
            Message::SetWetDryMix(value) => state.wet_dry = value.clamp(0.0, 1.0),
            Message::SetEffectParamUi { slot, param, value } => {
                effects_panel::store_effect_param(state, slot, param, value)
            }
            _ => return,
        }
        let settings = state.to_effect_settings();
        self.edit_macro(Edit::Effects(index, settings));
    }
}
