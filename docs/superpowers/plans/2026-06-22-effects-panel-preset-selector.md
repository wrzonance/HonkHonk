# Effects Panel + Preset Selector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a voice-effects UI (preset selector, master bypass, wet/dry, per-effect parameter sliders) that drives the existing index-addressed `AudioCommand::SetEffect*` commands through a stable, shared effect-chain layout.

**Architecture:** Three layers. (1) A new `src/audio/effects/layout.rs` defines `EffectSlot` — a fixed, ordered enum giving each concrete effect a stable chain index and its parameter names; both the engine (to populate the chain once at startup) and the UI (to address commands) reference it, eliminating the "empty chain / meaningless index" gap. (2) A new `src/ui/effects_panel.rs` is a pure `Element<Message>` view plus a small `EffectsUiState` holding the current preset/bypass/wet-dry/param values, and a free function mapping a preset to the list of `AudioCommand`s it implies. (3) `app.rs` gains thin `Message` variants whose `update()` arms delegate to the panel module to compute commands and to `EffectsUiState` to update display state — no business logic in `app.rs`.

**Tech Stack:** Rust, Iced 0.14 (Elm/MVU; UI = `Element<Message>` functions), existing `EffectChain`/`AudioEffect`/`AudioCommand` audio infrastructure, PipeWire.

## Global Constraints

- File size 400 lines max; functions ≤50 lines. `src/app.rs` is ~2,491 lines and OVER budget — DO NOT add logic to it; only thin `Message` variants + delegating `update()` arms.
- `cargo clippy --all-targets -- -D warnings` must pass clean. clippy.toml: cognitive-complexity 10, too-many-arguments 5, too-many-lines 50, type-complexity 200. Run `cargo fmt`.
- No `.unwrap()` / `panic!()` in non-test code. `thiserror` typed enums per module; `anyhow` `.context(...)` at glue. No `String` errors across boundaries.
- TDD mandatory: failing test first. 80% coverage target. Do NOT test Iced view rendering — test preset→command mapping and state-update logic at the module boundary.
- Reuse existing effect types and `AudioCommand::SetEffect{Bypass,Param,WetDry,ChainBypass}` — do NOT reinvent them.
- No new crates. If Cargo.lock changes, regenerate `packaging/flatpak/cargo-sources.json` (it will NOT change here — no new deps).
- Commit format: Conventional Commits; trailer `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Branch: `feat/issue-33` only. Never commit to main.

## Existing API facts (verified, do not re-derive)

- `AudioHandle::send(AudioCommand)` — fire-and-forget to the audio thread. `app.rs` holds `audio: Option<AudioHandle>`.
- `AudioCommand` variants already exist: `SetEffectBypass { index: usize, bypass: bool }`, `SetEffectParam { index: usize, param: String, value: f32 }`, `SetEffectWetDry(f32)`, `SetEffectChainBypass(bool)`.
- `EffectChain::push_effect(Box<dyn AudioEffect>, block_size) -> Result<(), EffectsError>`. Mixer block size at engine init is `4096` (engine.rs:467 `Mixer::new(4096)`).
- Effect constructors: `RingModEffect::new(block_size: usize)`, `BandpassFilterEffect::new(block_size: usize)` (block ignored), `PitchShiftEffect::new(sample_rate: u32)`.
- Param vocab: Pitch → `"semitones"`, `"pitch_factor"`. RingMod → `"carrier"`. Bandpass → `"center"`, `"bandwidth"`, `"noise"`.
- Engine handler arms for `SetEffect*` already call `ctx.mixer.borrow_mut().chain_mut().{set_bypass,set_param,set_wet_dry,set_chain_bypass}` (engine.rs:545-580). The chain is currently NEVER populated — this plan populates it.
- Theme helpers: `crate::ui::theme::{self, Hh, Theme}`; `theme::space::{XS,SM,MD,LG,XL}`, `theme::font::{LABEL,BODY,TITLE}`, `theme::radius::{SM,MD,PILL}`, `theme::bg_color(Color)`, `Theme::{bg,panel,ink,ink_dim,ink_faint,hairline,hairline2,accent,good}`.
- Iced slider pattern (from settings.rs): `iced::widget::slider(min..=max, value, move |x| Message::...).step(s).width(Length::Fixed(w))`. Buttons: `button(...).on_press(Message::...)`. Chips use `button` with custom `style`.

---

### Task 1: Effect-chain layout contract (`EffectSlot`)

Defines the stable index ↔ effect ↔ param mapping shared by engine + UI. This is the keystone: without it, UI commands address an empty chain.

**Files:**
- Create: `src/audio/effects/layout.rs`
- Modify: `src/audio/effects/mod.rs` (add `pub mod layout;` + re-export)
- Test: inline `#[cfg(test)]` in `layout.rs`

**Interfaces:**
- Consumes: `AudioEffect` trait, `RingModEffect`, `BandpassFilterEffect`, `PitchShiftEffect` from `super`.
- Produces:
  - `pub enum EffectSlot { Pitch, RingMod, Bandpass }` (derives `Debug, Clone, Copy, PartialEq, Eq`).
  - `EffectSlot::ORDER: [EffectSlot; 3]` const — chain order = `[Pitch, RingMod, Bandpass]`.
  - `fn index(self) -> usize` — position in `ORDER`.
  - `fn label(self) -> &'static str` — `"Pitch"`, `"Ring Mod"`, `"Radio"`.
  - `fn build(self, block_size: usize, sample_rate: u32) -> Box<dyn AudioEffect>`.
  - `fn default_chain(block_size: usize, sample_rate: u32) -> Vec<Box<dyn AudioEffect>>` — built in `ORDER`, every effect starts bypassed.

- [ ] **Step 1: Write the failing test**

```rust
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
        assert!(chain.iter().all(|e| e.bypass()), "all effects start bypassed");
    }

    #[test]
    fn labels_are_human_readable() {
        assert_eq!(EffectSlot::Pitch.label(), "Pitch");
        assert_eq!(EffectSlot::RingMod.label(), "Ring Mod");
        assert_eq!(EffectSlot::Bandpass.label(), "Radio");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p honkhonk effects::layout 2>&1 | tail -20` (use the crate name from Cargo.toml `[package] name`; if unsure run `cargo test layout::tests`)
Expected: FAIL — `layout` module / `EffectSlot` not found.

- [ ] **Step 3: Write minimal implementation**

```rust
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
    pub fn index(self) -> usize {
        Self::ORDER
            .iter()
            .position(|&s| s == self)
            .unwrap_or_default()
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
```

Then in `src/audio/effects/mod.rs`, add after the other `pub mod` lines:
```rust
pub mod layout;
```
and after the other `pub use` lines:
```rust
pub use layout::{default_chain, EffectSlot};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test layout::tests 2>&1 | tail -20`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/audio/effects/layout.rs src/audio/effects/mod.rs
git commit -m "feat(effects): fixed EffectSlot chain layout shared by engine and UI

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Engine populates the default chain at startup

Wire `default_chain` into the mixer so UI commands hit real effects.

**Files:**
- Modify: `src/audio/mixer.rs` (add a method to populate the chain)
- Modify: `src/audio/engine.rs:467` area (call it after constructing the mixer)
- Test: inline `#[cfg(test)]` in `mixer.rs`

**Interfaces:**
- Consumes: `crate::audio::effects::default_chain`, `EffectSlot` from Task 1; `EffectChain::push_effect`.
- Produces: `Mixer::install_default_chain(&mut self, sample_rate: u32)` — pushes every `default_chain` effect into `self.chain` using the mixer's block size.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn install_default_chain_populates_all_slots_bypassed() {
    use crate::audio::effects::EffectSlot;
    let mut mixer = Mixer::new(4096);
    mixer.install_default_chain(48_000);
    let chain = mixer.chain_mut();
    assert_eq!(chain.len(), EffectSlot::ORDER.len());
    // All bypassed → passthrough.
    let input = vec![0.3_f32; 64];
    let out = mixer.process_block(&input, 48_000);
    assert_eq!(out, input.as_slice());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test mixer::tests::install_default_chain 2>&1 | tail -20`
Expected: FAIL — `install_default_chain` not found.

- [ ] **Step 3: Write minimal implementation**

In `src/audio/mixer.rs`, add a field for the block size if not present (it is `output_capacity` initial; store the construction block size). Add import at top:
```rust
use crate::audio::effects::{default_chain, EffectChain};
```
Add a method on `impl Mixer` (after `chain_mut`):
```rust
    /// Populate the chain with the fixed default layout (all effects bypassed).
    ///
    /// Cold path — call once at engine startup before the audio callback runs.
    /// Silently keeps any effects that fail to push (chain-too-long is not
    /// reachable here: the default layout is far under `MAX_CHAIN_LEN`).
    pub fn install_default_chain(&mut self, sample_rate: u32) {
        let block = self.output_capacity;
        for effect in default_chain(block, sample_rate) {
            // push_effect only errors on ChainTooLong; the fixed layout is small.
            let _ = self.chain.push_effect(effect, block);
        }
    }
```
> Note: `output_capacity` is set to the constructor's `initial_block_size`, so it is the correct block size here.

In `src/audio/engine.rs`, immediately after `let mixer = Rc::new(RefCell::new(super::mixer::Mixer::new(4096)));` (line ~467), add:
```rust
    mixer.borrow_mut().install_default_chain(48_000);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test mixer::tests 2>&1 | tail -20`
Expected: PASS (all mixer tests, including the new one).

- [ ] **Step 5: Commit**

```bash
git add src/audio/mixer.rs src/audio/engine.rs
git commit -m "feat(audio): populate default effect chain at engine startup

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Preset definitions + preset→commands mapping (panel core, no view yet)

The testable heart of the feature: presets and the function that turns a preset (or a param change) into the list of `AudioCommand`s, plus `EffectsUiState`.

**Files:**
- Create: `src/ui/effects_panel.rs` (logic + state only this task; view added Task 4)
- Modify: `src/ui/mod.rs` (add `pub mod effects_panel;`)
- Test: inline `#[cfg(test)]` in `effects_panel.rs`

**Interfaces:**
- Consumes: `crate::audio::{AudioCommand}`, `crate::audio::effects::EffectSlot`.
- Produces:
  - `pub enum PresetId { Robot, Radio, Deep, Chipmunk, Custom }` (`Debug, Clone, Copy, PartialEq, Eq`).
  - `PresetId::ALL: [PresetId; 5]`, `fn label(self) -> &'static str`, `fn description(self) -> &'static str`, `fn glyph(self) -> &'static str`.
  - `pub struct EffectsUiState { pub preset: PresetId, pub chain_bypass: bool, pub wet_dry: f32, pub pitch_semitones: f32, pub carrier_hz: f32, pub center_hz: f32, pub bandwidth_hz: f32, pub noise: f32 }` with `Default` (Custom, not bypassed=false→bypass true? see below), and `fn apply_preset(&mut self, preset: PresetId)`.
  - `fn preset_commands(preset: PresetId) -> Vec<AudioCommand>` — full set of bypass+param commands realizing the preset.
  - `fn param_command(slot: EffectSlot, param: &'static str, value: f32) -> AudioCommand`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{effects::EffectSlot, AudioCommand};

    fn has_unbypass(cmds: &[AudioCommand], slot: EffectSlot) -> bool {
        cmds.iter().any(|c| matches!(c,
            AudioCommand::SetEffectBypass { index, bypass: false } if *index == slot.index()))
    }
    fn has_bypass(cmds: &[AudioCommand], slot: EffectSlot) -> bool {
        cmds.iter().any(|c| matches!(c,
            AudioCommand::SetEffectBypass { index, bypass: true } if *index == slot.index()))
    }
    fn param_value(cmds: &[AudioCommand], slot: EffectSlot, p: &str) -> Option<f32> {
        cmds.iter().find_map(|c| match c {
            AudioCommand::SetEffectParam { index, param, value }
                if *index == slot.index() && param == p => Some(*value),
            _ => None,
        })
    }

    #[test]
    fn robot_enables_only_ring_mod_at_150hz() {
        let cmds = preset_commands(PresetId::Robot);
        assert!(has_unbypass(&cmds, EffectSlot::RingMod));
        assert!(has_bypass(&cmds, EffectSlot::Pitch));
        assert!(has_bypass(&cmds, EffectSlot::Bandpass));
        assert_eq!(param_value(&cmds, EffectSlot::RingMod, "carrier"), Some(150.0));
    }

    #[test]
    fn radio_enables_bandpass_center_1500_with_noise() {
        let cmds = preset_commands(PresetId::Radio);
        assert!(has_unbypass(&cmds, EffectSlot::Bandpass));
        assert!(has_bypass(&cmds, EffectSlot::RingMod));
        assert_eq!(param_value(&cmds, EffectSlot::Bandpass, "center"), Some(1500.0));
        assert_eq!(param_value(&cmds, EffectSlot::Bandpass, "noise"), Some(0.1));
    }

    #[test]
    fn deep_enables_pitch_down_only() {
        let cmds = preset_commands(PresetId::Deep);
        assert!(has_unbypass(&cmds, EffectSlot::Pitch));
        assert!(has_bypass(&cmds, EffectSlot::RingMod));
        assert!(has_bypass(&cmds, EffectSlot::Bandpass));
        let semis = param_value(&cmds, EffectSlot::Pitch, "semitones").unwrap();
        assert!(semis < 0.0, "deep voice shifts pitch down, got {semis}");
    }

    #[test]
    fn chipmunk_enables_pitch_up_only() {
        let cmds = preset_commands(PresetId::Chipmunk);
        let semis = param_value(&cmds, EffectSlot::Pitch, "semitones").unwrap();
        assert!(semis > 0.0, "chipmunk shifts pitch up, got {semis}");
    }

    #[test]
    fn custom_bypasses_all_effects() {
        let cmds = preset_commands(PresetId::Custom);
        for slot in EffectSlot::ORDER {
            assert!(has_bypass(&cmds, slot), "custom starts with {slot:?} bypassed");
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
    fn all_presets_listed_with_labels() {
        assert_eq!(PresetId::ALL.len(), 5);
        for p in PresetId::ALL {
            assert!(!p.label().is_empty());
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test effects_panel::tests 2>&1 | tail -20`
Expected: FAIL — module/symbols not found.

- [ ] **Step 3: Write minimal implementation**

```rust
//! Voice-effects panel: preset selector, master bypass, wet/dry, and per-effect
//! parameter controls. The *logic* here (presets → `AudioCommand`s, UI state) is
//! unit-tested at the module boundary; the Iced view (`view_effects_panel`) is a
//! thin rendering of this state and is intentionally not unit-tested.

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
            PresetId::Robot => "\u{1F916}",     // robot
            PresetId::Radio => "\u{1F4FB}",     // radio
            PresetId::Deep => "\u{1F30A}",      // wave
            PresetId::Chipmunk => "\u{1F43F}",  // chipmunk
            PresetId::Custom => "\u{1F39B}",    // control knobs
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
pub fn param_command(slot: EffectSlot, param: &'static str, value: f32) -> AudioCommand {
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

/// Full command set realizing `preset`: bypass every slot, then unbypass +
/// parameterize the ones the preset uses.
pub fn preset_commands(preset: PresetId) -> Vec<AudioCommand> {
    // Start from all-bypassed, then enable what the preset needs.
    let mut cmds: Vec<AudioCommand> = EffectSlot::ORDER
        .iter()
        .map(|&slot| bypass_command(slot, true))
        .collect();

    let enable = |cmds: &mut Vec<AudioCommand>, slot: EffectSlot| {
        cmds.push(bypass_command(slot, false));
    };

    match preset {
        PresetId::Robot => {
            enable(&mut cmds, EffectSlot::RingMod);
            cmds.push(param_command(EffectSlot::RingMod, "carrier", ROBOT_CARRIER_HZ));
        }
        PresetId::Radio => {
            enable(&mut cmds, EffectSlot::Bandpass);
            cmds.push(param_command(EffectSlot::Bandpass, "center", RADIO_CENTER_HZ));
            cmds.push(param_command(EffectSlot::Bandpass, "bandwidth", RADIO_BANDWIDTH_HZ));
            cmds.push(param_command(EffectSlot::Bandpass, "noise", RADIO_NOISE));
        }
        PresetId::Deep => {
            enable(&mut cmds, EffectSlot::Pitch);
            cmds.push(param_command(EffectSlot::Pitch, "semitones", DEEP_SEMITONES));
        }
        PresetId::Chipmunk => {
            enable(&mut cmds, EffectSlot::Pitch);
            cmds.push(param_command(EffectSlot::Pitch, "semitones", CHIPMUNK_SEMITONES));
        }
        PresetId::Custom => {} // all bypassed; user unlocks via sliders
    }
    cmds
}
```

Add to `src/ui/mod.rs`:
```rust
pub mod effects_panel;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test effects_panel::tests 2>&1 | tail -25`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add src/ui/effects_panel.rs src/ui/mod.rs
git commit -m "feat(ui): effect presets and preset->command mapping

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Effects panel view (Iced `Element`)

The Confetti-styled view. Pure rendering of `EffectsUiState`; not unit-tested per the no-view-test rule, but MUST compile and pass clippy. Keep each `fn` ≤50 lines — split into `view_preset_chips`, `view_master_row`, `view_param_sliders`.

**Files:**
- Modify: `src/ui/effects_panel.rs` (add view functions)
- Test: a single compile-smoke test allowed (constructs the element, asserts it builds) — this is the one exception, matching `app.rs`'s `view_builds_*` pattern; OR omit and rely on the app-level build test in Task 5. Prefer: add a build-smoke test here.

**Interfaces:**
- Consumes: `EffectsUiState`, `PresetId`, `crate::app::Message` (variants added in Task 5 — this task depends on Task 5's `Message` variants existing, so SEQUENCE Task 5 BEFORE Task 4's view body if building strictly; to avoid a circular wait, define the `Message` variants in Task 5 first). **Implementer note:** do Task 5 (message + state wiring) and Task 4 (view) in either order, but the view references `Message::{SelectEffectPreset, SetEffectBypassUi, SetWetDryMix, SetEffectParamUi}` which Task 5 defines. If doing Task 4 first, add those `Message` variants as part of this task instead and skip re-adding them in Task 5.
- Produces: `pub fn view_effects_panel<'a>(state: &'a EffectsUiState, t: Theme) -> Element<'a, Message>`.

- [ ] **Step 1: Write the build-smoke test**

```rust
#[test]
fn effects_panel_view_builds_for_each_preset() {
    use crate::ui::theme::Theme;
    for p in PresetId::ALL {
        let mut state = EffectsUiState::default();
        state.apply_preset(p);
        let _el = view_effects_panel(&state, Theme::Dark);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test effects_panel::tests::effects_panel_view_builds 2>&1 | tail -20`
Expected: FAIL — `view_effects_panel` not found.

- [ ] **Step 3: Write minimal implementation**

Add imports at top of `effects_panel.rs`:
```rust
use iced::widget::{button, column, container, row, slider, text, Column, Row, Space};
use iced::{Alignment, Border, Element, Length};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};
```
Add view functions (each ≤50 lines):
```rust
/// Top-level effects panel view.
pub fn view_effects_panel<'a>(state: &'a EffectsUiState, t: Theme) -> Element<'a, Message> {
    let body = column![
        view_master_row(state, t),
        view_preset_chips(state.preset, t),
        view_param_sliders(state, t),
    ]
    .spacing(theme::space::LG)
    .width(Length::Fill);

    container(body)
        .width(Length::Fill)
        .padding(theme::space::LG)
        .style(move |_th| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: theme::radius::MD,
            },
            ..Default::default()
        })
        .into()
}

fn view_master_row(state: &EffectsUiState, t: Theme) -> Element<'_, Message> {
    let bypass_label = if state.chain_bypass { "Effects: OFF" } else { "Effects: ON" };
    let toggle = button(text(bypass_label).size(theme::font::LABEL).color(t.ink()))
        .on_press(Message::SetEffectBypassUi(!state.chain_bypass))
        .style(move |_th, _status| chip_style(t, !state.chain_bypass));

    let wet = slider(0.0..=1.0, state.wet_dry, Message::SetWetDryMix)
        .step(0.01)
        .width(Length::Fixed(160.0));
    let wet_label = text(format!("{}%", (state.wet_dry * 100.0).round() as i32))
        .size(theme::font::LABEL)
        .color(t.ink_dim());

    row![toggle, Space::new().width(Length::Fill), text("Mix").size(theme::font::LABEL).color(t.ink_dim()), wet, wet_label]
        .spacing(theme::space::SM)
        .align_y(Alignment::Center)
        .into()
}

fn view_preset_chips(active: PresetId, t: Theme) -> Element<'static, Message> {
    let mut chips = Row::new().spacing(theme::space::SM);
    for p in PresetId::ALL {
        let selected = p == active;
        let label = format!("{} {}", p.glyph(), p.label());
        let chip = button(text(label).size(theme::font::LABEL).color(t.ink()))
            .on_press(Message::SelectEffectPreset(p))
            .padding([theme::space::XS, theme::space::MD])
            .style(move |_th, _status| chip_style(t, selected));
        chips = chips.push(chip);
    }
    chips.into()
}

fn view_param_sliders(state: &EffectsUiState, t: Theme) -> Element<'_, Message> {
    use crate::audio::effects::EffectSlot;
    column![
        labeled_slider("Pitch (semitones)", -12.0..=12.0, state.pitch_semitones, 0.5,
            move |v| param_msg(EffectSlot::Pitch, "semitones", v), t),
        labeled_slider("Carrier (Hz)", 20.0..=2000.0, state.carrier_hz, 1.0,
            move |v| param_msg(EffectSlot::RingMod, "carrier", v), t),
        labeled_slider("Center (Hz)", 200.0..=4000.0, state.center_hz, 1.0,
            move |v| param_msg(EffectSlot::Bandpass, "center", v), t),
        labeled_slider("Bandwidth (Hz)", 100.0..=4000.0, state.bandwidth_hz, 1.0,
            move |v| param_msg(EffectSlot::Bandpass, "bandwidth", v), t),
        labeled_slider("Noise", 0.0..=1.0, state.noise, 0.01,
            move |v| param_msg(EffectSlot::Bandpass, "noise", v), t),
    ]
    .spacing(theme::space::SM)
    .into()
}

fn param_msg(slot: crate::audio::effects::EffectSlot, param: &'static str, value: f32) -> Message {
    Message::SetEffectParamUi { slot, param, value }
}

fn labeled_slider<'a>(
    label: &'static str,
    range: std::ops::RangeInclusive<f32>,
    value: f32,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
    t: Theme,
) -> Element<'a, Message> {
    row![
        text(label).size(theme::font::LABEL).color(t.ink_dim()).width(Length::Fixed(130.0)),
        slider(range, value, on_change).step(step).width(Length::Fill),
    ]
    .spacing(theme::space::SM)
    .align_y(Alignment::Center)
    .into()
}

fn chip_style(t: Theme, selected: bool) -> button::Style {
    let bg = if selected { t.accent() } else { t.panel() };
    button::Style {
        background: Some(theme::bg_color(bg)),
        text_color: t.ink(),
        border: Border { color: t.hairline(), width: 1.0, radius: theme::radius::PILL },
        ..Default::default()
    }
}
```
> If `labeled_slider` trips clippy `too-many-arguments` (limit 5), it has 6 — refactor to take a small `SliderSpec { label, range, value, step }` struct + `(on_change, t)`. Implementer: prefer the struct to stay under the limit.

- [ ] **Step 4: Run build + test + clippy**

Run: `cargo test effects_panel::tests 2>&1 | tail -20 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -20`
Expected: tests PASS; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/ui/effects_panel.rs
git commit -m "feat(ui): Confetti-styled effects panel view

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Wire panel into app.rs (Message variants, thin update arms, state, integration into view)

Add `Message` variants, `EffectsUiState` field, delegating `update()` arms, and render the panel. All `update()` arms MUST be thin: call `preset_commands` / `param_command` / `EffectsUiState` methods and `audio.send(...)` — no logic.

**Files:**
- Modify: `src/app.rs` — `Message` enum (+4 variants), `HonkHonk` struct (+`effects_ui: EffectsUiState`), `new`/`new_for_test` (init field), `update()` (+4 thin arms), `view_main` (render panel below now_playing OR in a collapsible bottom section), imports.
- Test: inline `#[cfg(test)]` in `app.rs` — assert state updates (NOT command emission, which is covered in Task 3).

**Interfaces:**
- Consumes: `crate::ui::effects_panel::{EffectsUiState, PresetId, preset_commands, param_command, view_effects_panel}`, `crate::audio::effects::EffectSlot`, `AudioCommand`.
- Produces: `Message::{SelectEffectPreset(PresetId), SetEffectBypassUi(bool), SetWetDryMix(f32), SetEffectParamUi { slot: EffectSlot, param: &'static str, value: f32 }}`.

> **Note on `Message` deriving `PartialEq`:** `Message` derives `Clone, PartialEq`. `PresetId`, `EffectSlot` are `Copy + PartialEq`; `&'static str` and `f32` are `PartialEq`. So the new variants keep `Message: PartialEq`. Good.

- [ ] **Step 1: Write the failing test** (in `app.rs` tests module)

```rust
#[test]
fn select_effect_preset_updates_ui_state() {
    use crate::ui::effects_panel::PresetId;
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SelectEffectPreset(PresetId::Robot));
    assert_eq!(app.effects_ui_preset(), PresetId::Robot);
}

#[test]
fn set_wet_dry_updates_ui_state() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SetWetDryMix(0.4));
    assert!((app.effects_ui_wet_dry() - 0.4).abs() < 1e-6);
}

#[test]
fn set_effect_bypass_updates_ui_state() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SetEffectBypassUi(true));
    assert!(app.effects_ui_chain_bypass());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test app::tests::select_effect_preset 2>&1 | tail -20`
Expected: FAIL — variants/accessors not found.

- [ ] **Step 3: Write minimal implementation**

Imports (top of `app.rs`): extend the `use crate::ui::...` and `use crate::audio::...` lines:
```rust
use crate::audio::effects::EffectSlot;
use crate::ui::effects_panel::{self, EffectsUiState, PresetId};
```
Add `Message` variants (in the enum, near the Audio group):
```rust
    // Voice effects
    SelectEffectPreset(PresetId),
    SetEffectBypassUi(bool),
    SetWetDryMix(f32),
    SetEffectParamUi { slot: EffectSlot, param: &'static str, value: f32 },
```
Struct field (in `pub struct HonkHonk`):
```rust
    effects_ui: EffectsUiState,
```
Initialize in BOTH `new(...)` and `new_for_test()` (find each struct literal and add):
```rust
            effects_ui: EffectsUiState::default(),
```
Test accessors (in `impl HonkHonk`, near other small getters — keep with the non-test impl so tests can call them; mark `#[cfg(test)]` is fine since only tests use them, but they may also help future code — use plain `pub(crate)`):
```rust
    pub(crate) fn effects_ui_preset(&self) -> PresetId { self.effects_ui.preset }
    pub(crate) fn effects_ui_wet_dry(&self) -> f32 { self.effects_ui.wet_dry }
    pub(crate) fn effects_ui_chain_bypass(&self) -> bool { self.effects_ui.chain_bypass }
```
Thin `update()` arms (add to the `match message { ... }`):
```rust
            Message::SelectEffectPreset(preset) => {
                self.effects_ui.apply_preset(preset);
                if let Some(audio) = &self.audio {
                    for cmd in effects_panel::preset_commands(preset) {
                        audio.send(cmd);
                    }
                }
            }
            Message::SetEffectBypassUi(bypass) => {
                self.effects_ui.chain_bypass = bypass;
                if let Some(audio) = &self.audio {
                    audio.send(AudioCommand::SetEffectChainBypass(bypass));
                }
            }
            Message::SetWetDryMix(mix) => {
                self.effects_ui.wet_dry = mix;
                if let Some(audio) = &self.audio {
                    audio.send(AudioCommand::SetEffectWetDry(mix));
                }
            }
            Message::SetEffectParamUi { slot, param, value } => {
                self.effects_ui.preset = PresetId::Custom;
                store_effect_param(&mut self.effects_ui, slot, param, value);
                if let Some(audio) = &self.audio {
                    audio.send(effects_panel::param_command(slot, param, value));
                }
            }
```
Add a small free helper near the bottom of `app.rs` (NOT a method — keeps the match arm thin and avoids growing impl logic) — OR place it in `effects_panel.rs` and call it. Prefer `effects_panel.rs` to keep app.rs logic-free:

In `effects_panel.rs` add:
```rust
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
```
and import it in `app.rs` via `effects_panel::store_effect_param` (call as `effects_panel::store_effect_param(&mut self.effects_ui, slot, param, value);` — adjust the arm above accordingly).

Render the panel in `view_main`: insert it as a top-level item BEFORE `now_playing` in the `items` vec so it sits above the now-playing bar:
```rust
        let effects = effects_panel::view_effects_panel(&self.effects_ui, t);
        let items: Vec<Element<'_, Message>> = vec![
            top.into(),
            chips,
            scrollable(grid).height(Length::Fill).into(),
            effects,
            now_playing,
        ];
```

- [ ] **Step 4: Run test + build + clippy + fmt**

Run:
```bash
cargo test app::tests 2>&1 | tail -25
cargo build 2>&1 | tail -10
cargo clippy --all-targets -- -D warnings 2>&1 | tail -20
cargo fmt --check
```
Expected: tests PASS; build OK; clippy clean; fmt clean.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/ui/effects_panel.rs
git commit -m "feat(ui): integrate effects panel into main window

Adds thin Message variants + delegating update arms; panel renders above
the now-playing bar and drives AudioCommand::SetEffect* on change.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Full verification gate

**Files:** none (verification only).

- [ ] **Step 1: Full build**

Run: `cargo build 2>&1 | tail -15`
Expected: compiles, no warnings.

- [ ] **Step 2: All tests**

Run: `cargo test 2>&1 | tail -30`
Expected: all green.

- [ ] **Step 3: Clippy (all targets, deny warnings)**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -20`
Expected: clean.

- [ ] **Step 4: Format check + file-size check**

Run:
```bash
cargo fmt --check
wc -l src/ui/effects_panel.rs src/audio/effects/layout.rs
```
Expected: fmt clean; `effects_panel.rs` < 400 lines, `layout.rs` < 400 lines.

- [ ] **Step 5: Confirm app.rs did not absorb logic**

Run: `wc -l src/app.rs` and eyeball the new update arms.
Expected: arms are thin (delegate-only); app.rs growth is just variants + arms + field + 3 accessors + 1 view insert.

---

## Self-Review

**Spec coverage:**
- `src/ui/effects_panel.rs` Iced view component → Tasks 3+4. ✓
- Preset selector (chip bar, named presets Robot/Radio/Deep/Chipmunk/Custom) → Task 4 `view_preset_chips`, Task 3 `PresetId`. ✓
- Bypass toggle (master on/off) → `SetEffectBypassUi` → `AudioCommand::SetEffectChainBypass`. ✓
- Wet/dry slider → `SetWetDryMix` → `AudioCommand::SetEffectWetDry`. ✓
- Per-effect parameter controls (pitch, carrier, bandwidth) → Task 4 `view_param_sliders`. ✓
- Visual feedback (active highlighted, bypass state clear) → `chip_style(selected)` + master label "Effects: ON/OFF". ✓
- Confetti design language → reuses `theme` tokens (panel/hairline/accent/pill radius), glyphs. ✓
- Integrates into main window → Task 5 inserts above now-playing bar. ✓ (Design decision: bottom-of-main, not settings — documented in PR.)
- `EffectsCommand`/`AudioCommand` sent on change → Task 5 update arms. ✓
- Preset display: name + description + glyph → `PresetId::{label,description,glyph}`. ✓
- Robot→ring mod 150Hz / Radio→bandpass 1.5kHz + noise / Deep→pitch down / Custom unlocked → Task 3 `preset_commands` + tests. ✓ (Deep uses pitch shift, not 0.7x factor literally — semitones is the effect's native param; -5 semitones ≈ 0.75x, matching PitchPreset::Deep. Documented in PR.)
- Message types from issue (`SelectEffectPreset`, `SetEffectBypass`, `SetWetDryMix`, `SetEffectParam`) → mapped to `SelectEffectPreset`, `SetEffectBypassUi`, `SetWetDryMix`, `SetEffectParamUi`. Renamed `*Ui` suffix on bypass/param to avoid colliding with intent and to signal "UI origin"; documented in PR. ✓

**Placeholder scan:** none — all steps carry real code/commands.

**Type consistency:** `EffectSlot::index()`, `PresetId::ALL/label/description/glyph`, `EffectsUiState` fields, `preset_commands`, `param_command`, `store_effect_param`, `view_effects_panel` names match across tasks. `Message` variant names match between Task 4 (view) and Task 5 (definitions): `SelectEffectPreset`, `SetEffectBypassUi`, `SetWetDryMix`, `SetEffectParamUi`. ✓

**Gap addressed:** Original empty-chain/undefined-index problem solved by Tasks 1–2 (the only "extra" beyond pure-UI scope; minimal and necessary for the UI's commands to do anything).
