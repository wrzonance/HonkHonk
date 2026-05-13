# Mic Passthrough Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a toggleable mic passthrough control to the Audio settings section, persisted in config and wired to the PipeWire link layer.

**Architecture:** `RegistryGuard` gains an `apply_passthrough(bool, core)` method that either drops mic→sink PipeWire links (off) or recreates them from current registry state (on). A shared `Rc<Cell<bool>>` gates the listener closure so re-appearing PW globals don't re-create links while passthrough is disabled. Level slider is stored in config and shown in UI but has no PW audio effect (deferred to issue #29).

**Tech Stack:** Rust, Iced 0.13, pipewire-rs 0.8, serde_json

**Spec:** `docs/superpowers/specs/2026-05-12-mic-passthrough-toggle-design.md`
**Branch:** `feat/mic-passthrough-toggle` (create from `main` before starting)

---

## File Map

| File | Change |
|------|--------|
| `src/state/config.rs` | Add `mic_passthrough: bool`, `mic_passthrough_level: f32`, serde defaults |
| `src/audio/engine.rs` | Add `AudioCommand::SetMicPassthrough(bool)`, `SetMicPassthroughLevel(f32)`, update `spawn` signature, handle new commands |
| `src/audio/registry.rs` | Split links vec, add `mic_passthrough: Rc<Cell<bool>>` gate, `apply_passthrough` method on `RegistryGuard` |
| `src/settings/mod.rs` | Add `MicPassthrough` and `MicPassthroughLevel` entries to `SETTINGS_REGISTRY`, update tests |
| `src/ui/settings.rs` | Add Toggle and Slider match arms in `render_setting_row`, wire `get_setting_value` and `setting_message` |
| `src/app.rs` | Add `Message::MicPassthroughChanged(bool)` and `MicPassthroughLevelChanged(f32)`, wire `update()` handlers, update `spawn` call |

---

## Task 1: Config fields

**Files:**
- Modify: `src/state/config.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block inside `src/state/config.rs`:

```rust
#[test]
fn default_mic_passthrough_is_true() {
    assert!(AppConfig::default().mic_passthrough);
}

#[test]
fn default_mic_passthrough_level_is_one() {
    let eps = 1e-6_f32;
    assert!((AppConfig::default().mic_passthrough_level - 1.0).abs() < eps);
}

#[test]
fn mic_passthrough_false_round_trips_json() {
    let config = AppConfig {
        mic_passthrough: false,
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(!back.mic_passthrough);
}

#[test]
fn mic_passthrough_level_round_trips_json() {
    let config = AppConfig {
        mic_passthrough_level: 0.42,
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    let eps = 1e-5_f32;
    assert!((back.mic_passthrough_level - 0.42).abs() < eps);
}

#[test]
fn missing_mic_passthrough_field_deserializes_to_default() {
    // Simulates loading a config written before this field existed.
    let json = r#"{"sound_directories":[],"volume":0.85,"window_width":900,"window_height":600,"theme":"dark","density":"regular"}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert!(config.mic_passthrough);
    let eps = 1e-6_f32;
    assert!((config.mic_passthrough_level - 1.0).abs() < eps);
}
```

- [ ] **Step 2: Run tests — confirm they fail to compile**

```bash
cargo test -p honkhonk --lib state::config 2>&1 | head -30
```

Expected: compile error — field `mic_passthrough` not found on `AppConfig`; struct literal missing fields.

- [ ] **Step 3: Add fields and serde helpers**

In `src/state/config.rs`, add two private serde default helpers just before the `DEFAULT_VOLUME` constant:

```rust
fn default_true() -> bool {
    true
}

fn default_level() -> f32 {
    1.0
}
```

Add the two new fields to `AppConfig` (after `density`):

```rust
#[serde(default = "default_true")]
pub mic_passthrough: bool,
#[serde(default = "default_level")]
pub mic_passthrough_level: f32,
```

Add the fields to the `Default` impl (inside `Self { ... }`):

```rust
mic_passthrough: true,
mic_passthrough_level: 1.0,
```

- [ ] **Step 4: Fix existing tests that construct AppConfig directly**

Two existing tests construct `AppConfig` with named fields. Both will now fail to compile without the new fields. Update them to use struct update syntax:

`round_trip_serialize_deserialize` (around line 220):
```rust
let config = AppConfig {
    sound_directories: vec![PathBuf::from("/tmp/sounds")],
    volume: 0.5,
    window_width: 1024,
    window_height: 768,
    theme: Theme::Dark,
    density: Density::Compact,
    mic_passthrough: true,
    mic_passthrough_level: 0.75,
};
```

`save_and_load_from_path` (around line 240):
```rust
let config = AppConfig {
    sound_directories: vec![PathBuf::from("/home/user/sounds")],
    volume: 0.7,
    window_width: 800,
    window_height: 500,
    theme: Theme::Dark,
    density: Density::Comfy,
    mic_passthrough: false,
    mic_passthrough_level: 0.5,
};
```

- [ ] **Step 5: Run tests — confirm green**

```bash
cargo test -p honkhonk --lib state::config 2>&1
```

Expected: all config tests pass, zero warnings.

- [ ] **Step 6: Clippy check**

```bash
cargo clippy -- -D warnings 2>&1 | head -40
```

Expected: zero warnings. (New fields have no behavior yet — no unused-field warnings expected since `pub` fields are always considered used.)

- [ ] **Step 7: Commit**

```bash
git add src/state/config.rs
git commit -m "feat(state): add mic_passthrough and mic_passthrough_level to AppConfig"
```

---

## Task 2: AudioCommand variants + spawn signature

**Files:**
- Modify: `src/audio/engine.rs`

- [ ] **Step 1: Add new AudioCommand variants**

In `src/audio/engine.rs`, extend the `AudioCommand` enum (after `SetVolume`):

```rust
SetMicPassthrough(bool),
SetMicPassthroughLevel(f32),
```

Full updated enum:
```rust
#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play {
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
    },
    Stop,
    SetVolume(f32),
    SetMicPassthrough(bool),
    SetMicPassthroughLevel(f32),
    Shutdown,
}
```

- [ ] **Step 2: Update spawn signature**

Change `pub fn spawn() -> Result<AudioHandle, AudioError>` to:

```rust
pub fn spawn(initial_passthrough: bool) -> Result<AudioHandle, AudioError> {
```

Inside `spawn`, update the thread spawn closure to pass `initial_passthrough` to `run_engine`:

```rust
std::thread::Builder::new()
    .name("honkhonk-pw".into())
    .spawn(move || {
        let default_source = query_default_source_name();
        if let Err(e) = run_engine(cmd_rx, evt_tx.clone(), default_source, initial_passthrough) {
            let _ = evt_tx.send(AudioEvent::Error(e.to_string()));
        }
    })
    .map_err(AudioError::ThreadSpawn)?;
```

Update `run_engine` signature to accept the new parameter (but don't implement the body changes yet — add a `let _ = initial_passthrough;` stub so it compiles):

```rust
fn run_engine(
    cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
    default_source: Option<String>,
    initial_passthrough: bool,
) -> Result<(), AudioError> {
    let _ = initial_passthrough; // wired in Task 4
    // ... rest of existing body unchanged ...
```

- [ ] **Step 3: Fix the match in the command handler for new variants**

In the `cmd_rx.attach` closure's `match cmd { ... }`, add stubs so the match is exhaustive:

```rust
AudioCommand::SetMicPassthrough(_) => {}
AudioCommand::SetMicPassthroughLevel(_) => {}
```

Place these before the `AudioCommand::Shutdown` arm.

- [ ] **Step 4: Clippy check**

```bash
cargo clippy -- -D warnings 2>&1 | head -40
```

Expected: zero warnings. The `let _ = initial_passthrough;` suppresses unused-variable. The empty match arms are fine.

- [ ] **Step 5: Commit**

```bash
git add src/audio/engine.rs
git commit -m "feat(audio): add SetMicPassthrough/SetMicPassthroughLevel commands, update spawn signature"
```

---

## Task 3: Registry refactor — split links + apply_passthrough

**Files:**
- Modify: `src/audio/registry.rs`

This task refactors the registry module to:
1. Accept `mic_passthrough: Rc<Cell<bool>>` so the listener gates mic link creation
2. Keep mic links in a separate `Rc<RefCell<Vec<Link>>>` so they can be dropped independently
3. Add `apply_passthrough` method to `RegistryGuard`

- [ ] **Step 1: Add Cell import**

At the top of `src/audio/registry.rs`, verify `Cell` is imported (it already is via `std::cell::{Cell, RefCell}`). No change needed here.

- [ ] **Step 2: Update setup_registry_listener signature**

Change:
```rust
pub fn setup_registry_listener(
    core: &pipewire::core::CoreRc,
    shared_sink_id: Rc<Cell<Option<u32>>>,
    default_source_name: Option<String>,
) -> Result<RegistryGuard<'_>, AudioError> {
```

To:
```rust
pub fn setup_registry_listener(
    core: &pipewire::core::CoreRc,
    shared_sink_id: Rc<Cell<Option<u32>>>,
    default_source_name: Option<String>,
    mic_passthrough: Rc<Cell<bool>>,
) -> Result<RegistryGuard<'_>, AudioError> {
```

- [ ] **Step 3: Split the links vecs**

Inside `setup_registry_listener`, replace:
```rust
let all_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));
```

With:
```rust
let mic_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));
let other_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));
```

- [ ] **Step 4: Update the listener closure**

Replace:
```rust
let state_ref = state.clone();
let links_ref = all_links.clone();
let core_ref = core.clone();
let listener = registry
    .add_listener_local()
    .global(move |global| {
        let mut s = state_ref.borrow_mut();
        handle_registry_global(global, &mut s);
        if let Some(id) = s.sink_node_id {
            shared_sink_id.set(Some(id));
        }
        let mut link_store = links_ref.borrow_mut();
        try_create_mic_links(&mut s, &core_ref, &mut link_store);
        try_create_monitor_links(&mut s, &core_ref, &mut link_store);
    })
    .register();
```

With:
```rust
let state_ref = state.clone();
let mic_links_ref = mic_links.clone();
let other_links_ref = other_links.clone();
let mic_passthrough_ref = mic_passthrough.clone();
let core_ref = core.clone();
let listener = registry
    .add_listener_local()
    .global(move |global| {
        let mut s = state_ref.borrow_mut();
        handle_registry_global(global, &mut s);
        if let Some(id) = s.sink_node_id {
            shared_sink_id.set(Some(id));
        }
        if mic_passthrough_ref.get() {
            let mut ml = mic_links_ref.borrow_mut();
            try_create_mic_links(&mut s, &core_ref, &mut ml);
        }
        let mut ol = other_links_ref.borrow_mut();
        try_create_monitor_links(&mut s, &core_ref, &mut ol);
    })
    .register();
```

- [ ] **Step 5: Update RegistryGuard struct**

Replace:
```rust
pub struct RegistryGuard<'a> {
    _registry: pipewire::registry::RegistryBox<'a>,
    _listener: pipewire::registry::Listener,
    _links: Rc<RefCell<Vec<pipewire::link::Link>>>,
}
```

With:
```rust
pub struct RegistryGuard<'a> {
    _registry: pipewire::registry::RegistryBox<'a>,
    _listener: pipewire::registry::Listener,
    _other_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
    mic_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
    state: Rc<RefCell<RegistryState>>,
    mic_passthrough: Rc<Cell<bool>>,
}
```

- [ ] **Step 6: Update the Ok(...) return at the end of setup_registry_listener**

Replace:
```rust
Ok(RegistryGuard {
    _registry: registry,
    _listener: listener,
    _links: all_links,
})
```

With:
```rust
Ok(RegistryGuard {
    _registry: registry,
    _listener: listener,
    _other_links: other_links,
    mic_links,
    state,
    mic_passthrough,
})
```

- [ ] **Step 7: Add apply_passthrough method**

After the `RegistryGuard` struct definition, add:

```rust
impl<'a> RegistryGuard<'a> {
    pub fn apply_passthrough(&self, enabled: bool, core: &pipewire::core::CoreRc) {
        self.mic_passthrough.set(enabled);
        if enabled {
            let mut s = self.state.borrow_mut();
            let mut links = self.mic_links.borrow_mut();
            try_create_mic_links(&mut s, core, &mut links);
        } else {
            self.mic_links.borrow_mut().clear();
        }
    }
}
```

- [ ] **Step 8: Clippy + build check**

```bash
cargo clippy -- -D warnings 2>&1 | head -50
```

Expected: zero warnings. The compile will fail at `engine.rs` because `setup_registry_listener` call there hasn't been updated yet — that's fine for now. Focus on registry.rs being internally correct. If there are compile errors in engine.rs, note them and continue; they'll be fixed in Task 4.

Actually run this to check just registry.rs compiles in context:
```bash
cargo build 2>&1 | grep "registry\|error" | head -30
```

Expected: errors only in engine.rs (wrong number of args to `setup_registry_listener`). No errors in registry.rs itself.

- [ ] **Step 9: Commit**

```bash
git add src/audio/registry.rs
git commit -m "feat(audio): split mic_links in RegistryGuard, add apply_passthrough method"
```

---

## Task 4: Engine — wire passthrough into run_engine + command handlers

**Files:**
- Modify: `src/audio/engine.rs`

- [ ] **Step 1: Create mic_passthrough Rc in run_engine**

In `run_engine`, after the `default_source` param, remove the `let _ = initial_passthrough;` stub and add:

```rust
let mic_passthrough: Rc<Cell<bool>> = Rc::new(Cell::new(initial_passthrough));
```

Place this before the `let registry_sink_id` line.

- [ ] **Step 2: Pass mic_passthrough to setup_registry_listener**

Change:
```rust
let _registry_guard = setup_registry_listener(&core, registry_sink_id.clone(), default_source)?;
```

To:
```rust
let registry_guard = setup_registry_listener(
    &core,
    registry_sink_id.clone(),
    default_source,
    mic_passthrough,
)?;
```

Note: variable renamed from `_registry_guard` to `registry_guard` so it's accessible in the command closure.

- [ ] **Step 3: Update the command handler closure to move registry_guard in**

The existing command handler closure looks like:

```rust
let mainloop_quit = mainloop.clone();
let _cmd_listener = cmd_rx.attach(mainloop.loop_(), move |cmd| match cmd {
    AudioCommand::Play { ... } => { handle_play(&ctx, ...) }
    AudioCommand::Stop => { ... }
    AudioCommand::SetVolume(v) => { ... }
    AudioCommand::SetMicPassthrough(_) => {}
    AudioCommand::SetMicPassthroughLevel(_) => {}
    AudioCommand::Shutdown => { ... mainloop_quit.quit() }
});
```

Add a `core_for_passthrough` clone before the closure (needed because `ctx` already owns a `core` clone and the closure moves ctx):

```rust
let core_for_passthrough = core.clone();
```

Then update the `SetMicPassthrough` arm:

```rust
AudioCommand::SetMicPassthrough(v) => {
    registry_guard.apply_passthrough(v, &core_for_passthrough);
}
```

Keep `SetMicPassthroughLevel` as a no-op:

```rust
AudioCommand::SetMicPassthroughLevel(_) => {}
```

Move `registry_guard` into the closure by ensuring it's captured (the `move` closure will capture it automatically since it's referenced inside).

The full updated closure signature area:

```rust
let core_for_passthrough = core.clone();
let mainloop_quit = mainloop.clone();
let _cmd_listener = cmd_rx.attach(mainloop.loop_(), move |cmd| match cmd {
    AudioCommand::Play {
        sound_id,
        samples,
        sample_rate,
        channels,
    } => {
        handle_play(&ctx, sound_id, samples, sample_rate, channels);
    }
    AudioCommand::Stop => {
        let prev = ctx.active.borrow_mut().take();
        if let Some(ap) = prev {
            let _ = ctx.evt_tx.send(AudioEvent::PlaybackFinished {
                sound_id: ap.sound_id,
            });
        }
    }
    AudioCommand::SetVolume(v) => {
        ctx.engine_volume.set(v.clamp(0.0, 1.0));
        if let Some(ref ap) = *ctx.active.borrow() {
            ap.sink_state.borrow_mut().set_volume(v);
            ap.monitor_state.borrow_mut().set_volume(v);
        }
    }
    AudioCommand::SetMicPassthrough(v) => {
        registry_guard.apply_passthrough(v, &core_for_passthrough);
    }
    AudioCommand::SetMicPassthroughLevel(_) => {}
    AudioCommand::Shutdown => {
        let _ = ctx.active.borrow_mut().take();
        mainloop_quit.quit();
    }
});
```

- [ ] **Step 4: Fix the call site in app.rs (temporary)**

`spawn` now takes `initial_passthrough: bool`. The call in `app.rs` currently passes no arguments. Add a temporary `true` until Task 7 wires the config value:

Find the `audio::spawn()` call in `src/app.rs` (inside `HonkHonk::new` or equivalent) and change it to:

```bash
grep -n "audio::spawn" src/app.rs
```

Then update that line to `audio::spawn(true)`. (Task 7 will change this to `config.mic_passthrough`.)

- [ ] **Step 5: Build check**

```bash
cargo build 2>&1 | head -40
```

Expected: clean build, zero errors.

- [ ] **Step 6: Clippy check**

```bash
cargo clippy -- -D warnings 2>&1 | head -40
```

Expected: zero warnings.

- [ ] **Step 7: Run tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: all existing tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/audio/engine.rs src/app.rs
git commit -m "feat(audio): wire mic passthrough toggle into PipeWire engine"
```

---

## Task 5: Settings registry entries

**Files:**
- Modify: `src/settings/mod.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block in `src/settings/mod.rs`:

```rust
#[test]
fn mic_passthrough_entry_exists_in_audio_category() {
    let def = SETTINGS_REGISTRY
        .iter()
        .find(|d| matches!(d.id, SettingId::MicPassthrough))
        .expect("MicPassthrough must be in SETTINGS_REGISTRY");
    assert!(matches!(def.category, SettingCategory::Audio));
}

#[test]
fn mic_passthrough_control_is_toggle() {
    let def = SETTINGS_REGISTRY
        .iter()
        .find(|d| matches!(d.id, SettingId::MicPassthrough))
        .expect("MicPassthrough must be in SETTINGS_REGISTRY");
    assert!(matches!(def.control, ControlType::Toggle));
}

#[test]
fn mic_passthrough_level_entry_exists_in_audio_category() {
    let def = SETTINGS_REGISTRY
        .iter()
        .find(|d| matches!(d.id, SettingId::MicPassthroughLevel))
        .expect("MicPassthroughLevel must be in SETTINGS_REGISTRY");
    assert!(matches!(def.category, SettingCategory::Audio));
}

#[test]
fn mic_passthrough_level_control_is_slider() {
    let def = SETTINGS_REGISTRY
        .iter()
        .find(|d| matches!(d.id, SettingId::MicPassthroughLevel))
        .expect("MicPassthroughLevel must be in SETTINGS_REGISTRY");
    assert!(matches!(def.control, ControlType::Slider { .. }));
}

#[test]
fn audio_category_has_two_entries() {
    let count = SETTINGS_REGISTRY
        .iter()
        .filter(|d| matches!(d.category, SettingCategory::Audio))
        .count();
    assert_eq!(count, 2, "Audio section must have MicPassthrough + MicPassthroughLevel");
}
```

- [ ] **Step 2: Remove the old conflicting test**

Find and delete the existing test `audio_category_has_no_phase2_entries` (it asserts `count == 0`, which will now be wrong). Remove the entire `#[test]` block for it.

- [ ] **Step 3: Run tests — confirm new tests fail**

```bash
cargo test -p honkhonk --lib settings 2>&1 | tail -20
```

Expected: `mic_passthrough_entry_exists_in_audio_category` fails with "MicPassthrough must be in SETTINGS_REGISTRY".

- [ ] **Step 4: Add registry entries**

In `SETTINGS_REGISTRY` inside `src/settings/mod.rs`, add after the `Density` entry and before the `RescanLibrary` entry:

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

- [ ] **Step 5: Run tests — confirm green**

```bash
cargo test -p honkhonk --lib settings 2>&1 | tail -20
```

Expected: all settings tests pass.

- [ ] **Step 6: Clippy check**

```bash
cargo clippy -- -D warnings 2>&1 | head -20
```

Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add src/settings/mod.rs
git commit -m "feat(settings): add MicPassthrough and MicPassthroughLevel to SETTINGS_REGISTRY"
```

---

## Task 6: UI — Toggle and Slider rendering

**Files:**
- Modify: `src/ui/settings.rs`

- [ ] **Step 1: Add Toggle rendering arm to render_setting_row**

In `render_setting_row`, the `let control: Element<'_, Message> = match (&def.control, value) { ... }` block currently handles `Button` and `Radio`. Add a `Toggle` arm **before** the catch-all `_ =>`:

```rust
(ControlType::Toggle, SettingValue::Bool(v)) => {
    let id = def.id;
    row![
        button(
            text("On")
                .size(theme::font::BODY)
                .color(if v { t.bg() } else { t.ink() }),
        )
        .on_press(setting_message(id, SettingValue::Bool(true)))
        .padding([6.0, 14.0])
        .style(move |_t, _s| button::Style {
            background: Some(theme::bg_color(if v { t.ink() } else { t.panel() })),
            border: theme::tile_border(t.hairline2(), 1.0),
            ..Default::default()
        }),
        button(
            text("Off")
                .size(theme::font::BODY)
                .color(if !v { t.bg() } else { t.ink() }),
        )
        .on_press(setting_message(id, SettingValue::Bool(false)))
        .padding([6.0, 14.0])
        .style(move |_t, _s| button::Style {
            background: Some(theme::bg_color(if !v { t.ink() } else { t.panel() })),
            border: theme::tile_border(t.hairline2(), 1.0),
            ..Default::default()
        }),
    ]
    .spacing(theme::space::XS)
    .into()
}
```

- [ ] **Step 2: Add Slider rendering arm**

Add the `Slider` arm immediately after the `Toggle` arm (still before the catch-all):

```rust
(ControlType::Slider { min, max, step }, SettingValue::F32(v)) => {
    let id = def.id;
    row![
        iced::widget::slider((*min)..=(*max), v, move |x| {
            setting_message(id, SettingValue::F32(x))
        })
        .step(*step)
        .width(Length::Fixed(200.0)),
        text(format!("{:.0}%", v * 100.0))
            .size(theme::font::LABEL)
            .color(t.ink_dim())
            .font(iced::Font {
                family: iced::font::Family::Monospace,
                ..Default::default()
            }),
    ]
    .spacing(theme::space::SM)
    .align_y(Alignment::Center)
    .into()
}
```

- [ ] **Step 3: Wire get_setting_value**

In `get_setting_value`, add two arms before the `_ => SettingValue::None` catch-all:

```rust
SettingId::MicPassthrough => SettingValue::Bool(state.config.mic_passthrough),
SettingId::MicPassthroughLevel => SettingValue::F32(state.config.mic_passthrough_level),
```

- [ ] **Step 4: Wire setting_message**

In `setting_message`, add two arms before the `other =>` catch-all:

```rust
(SettingId::MicPassthrough, SettingValue::Bool(v)) => Message::MicPassthroughChanged(v),
(SettingId::MicPassthroughLevel, SettingValue::F32(v)) => Message::MicPassthroughLevelChanged(v),
```

These will cause compile errors until Task 7 adds the Message variants. Continue to Task 7 before running cargo build.

- [ ] **Step 5: Check render_setting_row line count**

```bash
grep -n "pub fn render_setting_row" src/ui/settings.rs
```

Count lines from that function to its closing brace. If it exceeds 50 lines (clippy threshold), extract helpers. If needed, add these two private functions before `render_setting_row`:

```rust
fn render_toggle<'a>(id: SettingId, v: bool, t: Theme) -> Element<'a, Message> {
    row![
        button(text("On").size(theme::font::BODY).color(if v { t.bg() } else { t.ink() }))
            .on_press(setting_message(id, SettingValue::Bool(true)))
            .padding([6.0, 14.0])
            .style(move |_t, _s| button::Style {
                background: Some(theme::bg_color(if v { t.ink() } else { t.panel() })),
                border: theme::tile_border(t.hairline2(), 1.0),
                ..Default::default()
            }),
        button(text("Off").size(theme::font::BODY).color(if !v { t.bg() } else { t.ink() }))
            .on_press(setting_message(id, SettingValue::Bool(false)))
            .padding([6.0, 14.0])
            .style(move |_t, _s| button::Style {
                background: Some(theme::bg_color(if !v { t.ink() } else { t.panel() })),
                border: theme::tile_border(t.hairline2(), 1.0),
                ..Default::default()
            }),
    ]
    .spacing(theme::space::XS)
    .into()
}

fn render_slider<'a>(id: SettingId, v: f32, min: f32, max: f32, step: f32, t: Theme) -> Element<'a, Message> {
    row![
        iced::widget::slider(min..=max, v, move |x| setting_message(id, SettingValue::F32(x)))
            .step(step)
            .width(Length::Fixed(200.0)),
        text(format!("{:.0}%", v * 100.0))
            .size(theme::font::LABEL)
            .color(t.ink_dim())
            .font(iced::Font { family: iced::font::Family::Monospace, ..Default::default() }),
    ]
    .spacing(theme::space::SM)
    .align_y(Alignment::Center)
    .into()
}
```

And replace the inline arms with calls:

```rust
(ControlType::Toggle, SettingValue::Bool(v)) => render_toggle(def.id, v, t),
(ControlType::Slider { min, max, step }, SettingValue::F32(v)) => {
    render_slider(def.id, v, *min, *max, *step, t)
}
```

- [ ] **Step 6: Commit (partial — will not compile until Task 7)**

```bash
git add src/ui/settings.rs
git commit -m "feat(ui): add Toggle and Slider rendering to settings panel"
```

---

## Task 7: App messages + update handlers

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing test**

Add to the `#[cfg(test)]` block in `src/app.rs` (or create one if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::config::AppConfig;

    #[test]
    fn mic_passthrough_changed_false_updates_config() {
        // Verify the Message variant exists and carries the right type.
        // Full integration test of update() is skipped (requires PW init).
        let msg = Message::MicPassthroughChanged(false);
        assert!(matches!(msg, Message::MicPassthroughChanged(false)));
    }

    #[test]
    fn mic_passthrough_level_changed_carries_f32() {
        let msg = Message::MicPassthroughLevelChanged(0.5);
        assert!(matches!(msg, Message::MicPassthroughLevelChanged(_)));
    }
}
```

- [ ] **Step 2: Run — confirm compile error**

```bash
cargo test -p honkhonk --lib app 2>&1 | head -20
```

Expected: error — `MicPassthroughChanged` not found on `Message`.

- [ ] **Step 3: Add Message variants**

In the `Message` enum in `src/app.rs`, add after `DensityChanged`:

```rust
// Audio
MicPassthroughChanged(bool),
MicPassthroughLevelChanged(f32),
```

- [ ] **Step 4: Add update() handlers**

Find the `update` function in `src/app.rs` and locate the `Message::DensityChanged(d)` arm. Add the two new arms after it:

```rust
Message::MicPassthroughChanged(v) => {
    let config = AppConfig {
        mic_passthrough: v,
        ..self.config.clone()
    };
    if let Err(e) = config.save() {
        eprintln!("honkhonk: failed to save config: {e}");
    }
    self.config = config;
    if let Some(ref audio) = self.audio {
        audio.send(AudioCommand::SetMicPassthrough(v));
    }
    Task::none()
}
Message::MicPassthroughLevelChanged(v) => {
    let config = AppConfig {
        mic_passthrough_level: v.clamp(0.0, 1.0),
        ..self.config.clone()
    };
    if let Err(e) = config.save() {
        eprintln!("honkhonk: failed to save config: {e}");
    }
    self.config = config;
    if let Some(ref audio) = self.audio {
        audio.send(AudioCommand::SetMicPassthroughLevel(v));
    }
    Task::none()
}
```

- [ ] **Step 5: Wire spawn with config value**

Find the `audio::spawn(true)` line added in Task 4 and update it to use the actual config:

```rust
audio::spawn(config.mic_passthrough)
```

(The exact location depends on where `spawn` is called — typically in `HonkHonk::new` or similar. Check with `grep -n "audio::spawn" src/app.rs`.)

- [ ] **Step 6: Build**

```bash
cargo build 2>&1 | head -40
```

Expected: clean build.

- [ ] **Step 7: Run all tests**

```bash
cargo test 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 8: Clippy**

```bash
cargo clippy -- -D warnings 2>&1 | head -40
```

Expected: zero warnings.

- [ ] **Step 9: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): wire MicPassthroughChanged and MicPassthroughLevelChanged handlers"
```

---

## Task 8: Final verification + PR

- [ ] **Step 1: Full test + clippy run**

```bash
cargo test 2>&1 | tail -20
cargo clippy -- -D warnings 2>&1
```

Expected: all tests green, zero warnings.

- [ ] **Step 2: Release build**

```bash
cargo build --release 2>&1 | tail -10
```

Expected: clean build.

- [ ] **Step 3: Smoke test (manual)**

```
1. cargo run
2. Open Settings → Audio
3. Confirm "Mic passthrough" toggle shows On/Off buttons, "On" active
4. Click "Off" — confirm config file (~/.config/honkhonk/config.json) now has "mic_passthrough": false
5. Click "On" — confirm config returns to "mic_passthrough": true
6. Confirm "Passthrough level" slider renders at 100%, dragging changes the % label and updates config
7. Restart app — confirm settings persist from saved config
```

- [ ] **Step 4: Push branch**

```bash
git push -u origin feat/mic-passthrough-toggle
```

- [ ] **Step 5: Create PR**

```bash
gh pr create \
  --title "feat(audio,ui): mic passthrough toggle + level slider (#71)" \
  --body "$(cat <<'EOF'
## Summary

- Toggleable mic passthrough in Audio settings (On/Off pill buttons)
- Passthrough level slider in Audio settings (0–100%, stored in config)
- PipeWire mic→sink links created/dropped immediately on toggle
- Level slider wired to config only — audio gain effect deferred to #29
- Existing configs deserialize cleanly via \`#[serde(default)]\`

## How it works

\`RegistryGuard::apply_passthrough(bool, core)\` either clears the \`mic_links\` vec (dropping PW links) or calls \`try_create_mic_links\` against current registry state. A shared \`Rc<Cell<bool>>\` gates the registry listener so PW global re-appearances don't recreate links while passthrough is off.

## Test plan

- [ ] \`cargo test\` — all tests green
- [ ] \`cargo clippy -- -D warnings\` — zero warnings
- [ ] \`cargo build --release\` — clean
- [ ] Manual: open Settings → Audio, toggle On/Off, verify config.json updates
- [ ] Manual: drag level slider, verify % label updates and config persists on restart
- [ ] Manual: toggle Off, speak into mic while playing sound — voice should not pass through

Closes #71
EOF
)"
```
