# Mic Passthrough Toggle — Design Spec

**Date:** 2026-05-12
**Issue:** #71
**Branch:** `feat/mic-passthrough-toggle`

## Scope

Wire a mic passthrough on/off toggle into the Audio settings section. Add a level slider to config and UI — audio effect deferred to issue #29 (per-source volume mixer), since PipeWire link-level gain requires a filter node beyond this PR's scope.

**Out of scope:**
- Actual PipeWire gain/volume control on the passthrough stream (#29)
- Monitor output device selection (#72)
- Renderer selection (#73)

## Architecture

### Config (`src/state/config.rs`)

Two new fields on `AppConfig`:

```rust
#[serde(default = "default_true")]
pub mic_passthrough: bool,

#[serde(default = "default_level")]
pub mic_passthrough_level: f32,
```

Serde default helpers:
```rust
fn default_true() -> bool { true }
fn default_level() -> f32 { 1.0 }
```

Defaults: passthrough `true`, level `1.0`. Existing configs deserialize cleanly via `#[serde(default)]`.

### Audio engine — registry (`src/audio/registry.rs`)

**`RegistryState`** gains `mic_passthrough: Rc<Cell<bool>>` — shared ref, not a plain bool.

**Link storage split:**
- `mic_links: Rc<RefCell<Vec<Link>>>` — mic→sink links only, exposed publicly on `RegistryGuard`
- `other_links` — monitor→vsource links, internal

`try_create_mic_links` gates on `state.mic_passthrough.get()` before creating any link.

**`setup_registry_listener`** signature:
```rust
pub fn setup_registry_listener(
    core: &CoreRc,
    shared_sink_id: Rc<Cell<Option<u32>>>,
    default_source_name: Option<String>,
    mic_passthrough: Rc<Cell<bool>>,
) -> Result<RegistryGuard<'_>, AudioError>
```

**`RegistryGuard`** new public fields:
```rust
pub mic_passthrough: Rc<Cell<bool>>,
pub state: Rc<RefCell<RegistryState>>,
pub mic_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
```

**`RegistryGuard::apply_passthrough`** method:
```rust
pub fn apply_passthrough(&self, enabled: bool, core: &pipewire::core::CoreRc) {
    self.mic_passthrough.set(enabled);
    if enabled {
        let mut s = self.state.borrow_mut();
        let mut links = self.mic_links.borrow_mut();
        try_create_mic_links(&mut s, core, &mut links);
    } else {
        self.mic_links.borrow_mut().clear(); // drops links → PW tears down connections
    }
}
```

### Audio engine — engine (`src/audio/engine.rs`)

**New `AudioCommand` variants:**
```rust
SetMicPassthrough(bool),
SetMicPassthroughLevel(f32),  // stored only, no PW effect until #29
```

**`spawn` signature change:**
```rust
pub fn spawn(initial_passthrough: bool) -> Result<AudioHandle, AudioError>
```

Creates `mic_passthrough: Rc<Cell<bool>>` from `initial_passthrough`, passes to `setup_registry_listener`.

**Command handler additions:**
```rust
AudioCommand::SetMicPassthrough(v) => {
    registry_guard.apply_passthrough(v, &core);
}
AudioCommand::SetMicPassthroughLevel(_v) => {
    // no-op: PW gain control deferred to #29
}
```

### Settings registry (`src/settings/mod.rs`)

Two new entries added to `SETTINGS_REGISTRY`:

```rust
SettingDef {
    id: SettingId::MicPassthrough,
    category: SettingCategory::Audio,
    label: "Mic passthrough",
    hint: "Mix your real mic into the virtual mic.",
    control: ControlType::Toggle,
},
SettingDef {
    id: SettingId::MicPassthroughLevel,
    category: SettingCategory::Audio,
    label: "Passthrough level",
    hint: "Mic gain into virtual mic. Audio effect lands in issue #29.",
    control: ControlType::Slider { min: 0.0, max: 1.0, step: 0.01 },
},
```

### UI — settings renderer (`src/ui/settings.rs`)

**`render_setting_row` new match arms:**

`ControlType::Toggle` + `SettingValue::Bool(v)`:
- Two pill buttons "On" / "Off" using same active/inactive style as `Radio`
- "On" active when `v == true`, "Off" active when `v == false`
- Messages: `setting_message(id, SettingValue::Bool(true/false))`

`ControlType::Slider { min, max, step }` + `SettingValue::F32(v)`:
- `iced::widget::slider(min..=max, v, move |x| setting_message(id, SettingValue::F32(x)))`
- Width `Fixed(200.0)`, step `step`
- Monospace value label beside it: `format!("{:.0}%", v * 100.0)`

**`get_setting_value` additions:**
```rust
SettingId::MicPassthrough => SettingValue::Bool(state.config.mic_passthrough),
SettingId::MicPassthroughLevel => SettingValue::F32(state.config.mic_passthrough_level),
```

**`setting_message` additions:**
```rust
(SettingId::MicPassthrough, SettingValue::Bool(v)) => Message::MicPassthroughChanged(v),
(SettingId::MicPassthroughLevel, SettingValue::F32(v)) => Message::MicPassthroughLevelChanged(v),
```

### App (`src/app.rs`)

**New `Message` variants:**
```rust
MicPassthroughChanged(bool),
MicPassthroughLevelChanged(f32),
```

**`update()` handlers (immutable config pattern):**
```rust
Message::MicPassthroughChanged(v) => {
    let config = AppConfig { mic_passthrough: v, ..self.config.clone() };
    if let Err(e) = config.save() { /* surface error */ }
    self.config = config;
    if let Some(ref audio) = self.audio {
        audio.send(AudioCommand::SetMicPassthrough(v));
    }
    Task::none()
}
Message::MicPassthroughLevelChanged(v) => {
    let config = AppConfig { mic_passthrough_level: v, ..self.config.clone() };
    if let Err(e) = config.save() { /* surface error */ }
    self.config = config;
    if let Some(ref audio) = self.audio {
        audio.send(AudioCommand::SetMicPassthroughLevel(v));
    }
    Task::none()
}
```

`spawn` call updated: `audio::spawn(config.mic_passthrough)`.

## Data Flow

```
User toggles "On/Off" in Audio settings
  → Message::MicPassthroughChanged(bool)
  → update(): config saved immutably, AudioCommand sent
  → PW thread: registry_guard.apply_passthrough(v, &core)
      if false: mic_links.clear() → PW drops mic→sink connections
      if true:  try_create_mic_links() → PW creates mic→sink connections
```

## Error Handling

`apply_passthrough` when toggling on: `try_create_mic_links` already logs failures to stderr per existing pattern. No new error surface needed — failure to re-create links is visible as silence (mic not passing through).

## Tests

### `src/state/config.rs`
- `default_mic_passthrough_is_true`
- `default_mic_passthrough_level_is_one`
- `mic_passthrough_round_trips_json`
- Update `round_trip_serialize_deserialize` to include new fields

### `src/settings/mod.rs`
- Rename `audio_category_has_no_phase2_entries` → `audio_category_has_two_entries` (assert `count == 2`)
- `mic_passthrough_is_toggle_control`
- `mic_passthrough_level_is_slider_control`

### `src/app.rs`
- `mic_passthrough_changed_updates_config` — assert config field updated correctly

## Build Verification

```bash
cargo clippy -- -D warnings   # must pass zero warnings
cargo test                     # all tests green
cargo build --release          # binary builds
```

Clippy limits: functions ≤50 lines, files ≤400 lines. If `render_setting_row` grows too long, extract `render_toggle` and `render_slider` helpers.
