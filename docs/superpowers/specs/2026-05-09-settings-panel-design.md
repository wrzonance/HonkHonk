# Settings Panel — Design Spec
**Date:** 2026-05-09
**Issue:** #11 `feat(ui): settings panel (full-window swap)`
**Phase:** 2 (shell) with Phase 3/4 backend hooks

---

## Problem

HonkHonk has no settings UI. Config is file-only. As features grow, users need a discoverable, in-app way to tweak behavior. The panel must be extensible: adding a new setting should require no manual UI edits.

---

## Decisions

| Question | Decision |
|---|---|
| Registry location | Central `src/settings/mod.rs` (Option A) |
| Change signals | Specific `Message` per setting (not generic) |
| Unimplemented settings | Not shown at all until backend sub-MVP lands |
| Complex widgets | Custom view fragments (hybrid — not forced through registry) |
| Backend issues | Create and spec all 5 before writing any settings code |
| Empty sections | All 5 sections always visible in sidebar nav |

---

## Architecture

### Settings Registry (`src/settings/mod.rs`)

Pure data module — no UI, no audio logic. Defines all planned settings (including future ones) so the enum is stable. Only settings with working backends appear in `SETTINGS_REGISTRY`.

```rust
pub enum SettingId {
    // Library (ready — Phase 2)
    RescanLibrary,
    // Appearance (backend pending — Phase 3)
    Theme,
    Density,
    ViewMode,
    // Audio (backend pending — Phase 3/4)
    MicPassthrough,
    MicPassthroughLevel,
    MonitorDevice,
    // App (backend pending — Phase 3)
    Renderer,
}

pub enum SettingCategory {
    Audio,
    Library,
    Hotkeys,
    Appearance,
    About,
}

pub enum ControlType {
    Toggle,
    Radio(&'static [&'static str]),
    Slider { min: f32, max: f32, step: f32 },
    Button,
    Select,  // options loaded dynamically from state (used by MonitorDevice)
}

pub enum SettingValue {
    Bool(bool),
    Index(usize),
    F32(f32),
    None,
}

pub struct SettingDef {
    pub id: SettingId,
    pub category: SettingCategory,
    pub label: &'static str,
    pub hint: &'static str,
    pub control: ControlType,
}

/// Phase 2 shell: only RescanLibrary wired.
/// Each backend sub-MVP adds its SettingDef here when it lands.
pub static SETTINGS_REGISTRY: &[SettingDef] = &[
    SettingDef {
        id: SettingId::RescanLibrary,
        category: SettingCategory::Library,
        label: "Scan now",
        hint: "Force re-scan of all sound folders.",
        control: ControlType::Button,
    },
];
```

Two free functions handle value reading and message dispatch:

```rust
/// Read current value for a setting from app state.
pub fn get_setting_value(id: SettingId, state: &HonkHonk) -> SettingValue

/// Produce the specific Message for a setting change.
pub fn setting_message(id: SettingId, value: SettingValue) -> Message
```

**Extension contract:** adding a new setting requires:
1. Add variant to `SettingId`
2. Add `SettingDef` to `SETTINGS_REGISTRY`
3. Add arm to `get_setting_value`
4. Add `Message` variant to `app.rs`
5. Add arm to `setting_message`
6. Add handler in `update()`

UI auto-renders. No other UI changes needed.

---

### View Architecture (`src/ui/settings.rs`)

New `ViewMode::Settings` + `SettingsSection` in `app.rs`.

```rust
// app.rs additions
pub enum ViewMode { Main, SlotManager, Settings }

pub enum SettingsSection { Audio, Library, Hotkeys, Appearance, About }

// HonkHonk struct — new field
settings_section: SettingsSection,  // default: Audio, not persisted
```

Layout:

```
view_settings(state, theme)
  ├── settings_header()
  │     ├── "← Back to sounds" button → Message::ShowMain
  │     └── "Settings · tweak the honk" title
  └── row![
        settings_sidebar(active_section, theme)   220px fixed
          └── 5 nav items (always all 5, always visible)
        settings_content(state, theme)
          ├── view_audio_section()
          ├── view_library_section()
          ├── view_hotkeys_section()
          ├── view_appearance_section()
          └── view_about_section()
      ]
```

Each section view:

```
view_audio_section(state, theme)
  ├── section_header("Audio", subtitle)
  ├── custom fragment: PipeWire status badge (display only)
  └── registry rows for SettingCategory::Audio (empty Phase 2)

view_library_section(state, theme)
  ├── section_header("Library", subtitle)
  ├── registry rows for SettingCategory::Library
  │     └── RescanLibrary button row
  └── custom fragment: folder list
        ├── row per config.sound_directories entry (path + remove button)
        └── "+ Add a folder" button → Message::AddSoundDirectory

view_hotkeys_section(state, theme)
  ├── section_header("Hotkeys", subtitle)
  ├── custom fragment: portal status badge
  ├── custom fragment: slot bindings table (read-only, from slot_triggers)
  └── registry rows for SettingCategory::Hotkeys (empty Phase 2)

view_appearance_section(state, theme)
  ├── section_header("Appearance", subtitle)
  └── registry rows for SettingCategory::Appearance (empty until Phase 3)

view_about_section(state, theme)
  ├── custom fragment: logo block + version + tagline
  ├── custom fragment: license pill
  ├── custom fragment: credits list
  └── custom fragment: source link button
```

Shared helper:

```rust
fn render_setting_row(def: &SettingDef, value: SettingValue, t: Theme) -> Element<'_, Message>
```

Renders label + hint on left, appropriate control widget on right, separated by a hairline border — matching the `SettingsRow` pattern from the design reference.

---

### New Messages

```rust
// Navigation
Message::ShowSettings,
Message::ShowSettingsSection(SettingsSection),

// Library actions (Phase 2)
Message::RescanLibrary,
Message::AddSoundDirectory,          // spawns ashpd FileChooser task
Message::RemoveSoundDirectory(PathBuf),

// Future — defined when backend sub-MVPs land
// Message::ThemeChanged(Theme)
// Message::DensityChanged(Density)
// Message::MicPassthroughChanged(bool)
// Message::MicPassthroughLevelChanged(f32)
// Message::MonitorDeviceChanged(String)
// Message::RendererChanged(Renderer)
```

`update()` handlers:

| Message | Effect |
|---|---|
| `ShowSettings` | `view_mode = Settings`, `settings_section = Audio` |
| `ShowSettingsSection(s)` | `settings_section = s` |
| `RescanLibrary` | re-runs `Library::scan`, replaces `self.sounds`, resets `durations_loaded = false`, rebuilds `duration_scan_pairs` from new sound list — duration subscription re-fires automatically |
| `AddSoundDirectory` | spawns ashpd `FileChooser` task → appends path to `config.sound_directories`, saves, emits `RescanLibrary`. **Requires** adding `"file_chooser"` feature to `ashpd` in `Cargo.toml` |
| `RemoveSoundDirectory(p)` | removes from `config.sound_directories`, saves config, emits `RescanLibrary` |

---

## Backend Sub-MVP Issues (create before writing settings code)

### Issue A — `feat(state,ui): theme preference persistence + live switching`
**Phase 3**
- Derive `Serialize`/`Deserialize` on `ui::theme::Theme` enum
- Add `theme: Theme` field to `AppConfig` (default: `Dark`)
- Remove hardcoded `Theme::Dark` from `HonkHonk::theme()` — map from `config.theme`
- `Message::ThemeChanged(Theme)` → updates `self.config.theme`, saves config
- Adds `SettingId::Theme` to `SETTINGS_REGISTRY` with `Radio(&["Light", "Dark", "System"])`

### Issue B — `feat(state,ui): density system`
**Phase 3**
- New `Density` enum: `Compact | Regular | Comfy`
- Tile heights: Compact=156px / Regular=192px / Comfy=224px
- Grid columns: Compact=6 / Regular=5 / Comfy=4
- Add `density: Density` to `AppConfig`
- Thread `density` into `view_sound_grid` (currently hardcoded)
- `Message::DensityChanged(Density)` → saves config
- Adds `SettingId::Density` to registry with `Radio(&["compact", "regular", "comfy"])`

### Issue C — `feat(audio): mic passthrough toggle + level`
**Phase 4**
- Add `mic_passthrough: bool` (default: `true`) + `mic_passthrough_level: f32` (default: `1.0`) to `AppConfig`
- New `AudioCommand::SetMicPassthrough(bool)` + `AudioCommand::SetMicPassthroughLevel(f32)`
- Gate `try_create_mic_links()` in `registry.rs` on `RegistryState.mic_passthrough`
- `Message::MicPassthroughChanged(bool)` + `Message::MicPassthroughLevelChanged(f32)`
- Adds `SettingId::MicPassthrough` (Toggle) + `SettingId::MicPassthroughLevel` (Slider 0.0–1.0) to registry

### Issue D — `feat(audio): monitor output device selection`
**Phase 4**
- New `AudioCommand::QueryOutputDevices` → `AudioEvent::OutputDevices(Vec<(String, String)>)`
- App state field: `output_devices: Vec<(String, String)>` (node_name, display_name)
- Add `monitor_device: Option<String>` to `AppConfig` (None = PipeWire auto)
- `AudioCommand::SetMonitorTarget(String)` → rebuild monitor stream with `target.object` property
- `Message::MonitorDeviceChanged(String)` → saves config, sends command
- Adds `SettingId::MonitorDevice` with `ControlType::Select` to registry

### Issue E — `feat(app): renderer selection`
**Phase 3**
- New `Renderer` enum: `Wgpu | TinySkia`
- Add `renderer: Renderer` to `AppConfig` (default: `Wgpu`)
- Read `HONKHONK_RENDERER` env var in `main.rs` (precedence: env var > config > Wgpu)
- `Message::RendererChanged(Renderer)` → saves config, shows restart-required notice in UI
- Adds `SettingId::Renderer` with `Radio(&["wgpu", "tiny-skia"])` to registry

---

## File Structure

### New files
```
Cargo.toml               ashpd: add "file_chooser" feature flag

src/settings/mod.rs      SettingId, SettingCategory, ControlType, SettingValue,
                         SettingDef, SETTINGS_REGISTRY, get_setting_value(),
                         setting_message()

src/ui/settings.rs       view_settings(), settings_header(), settings_sidebar(),
                         settings_content(), view_audio_section(),
                         view_library_section(), view_hotkeys_section(),
                         view_appearance_section(), view_about_section(),
                         render_setting_row()
```

### Modified files
```
src/app.rs               ViewMode::Settings, SettingsSection enum,
                         settings_section field on HonkHonk,
                         ShowSettings / ShowSettingsSection / RescanLibrary /
                         AddSoundDirectory / RemoveSoundDirectory handlers

src/ui/mod.rs            pub mod settings

src/lib.rs               pub mod settings
```

---

## PR Scope (#11 — Phase 2 shell)

**Ships:**
- `src/settings/mod.rs` — registry framework, `SETTINGS_REGISTRY` with `RescanLibrary` only
- `src/ui/settings.rs` — full settings UI shell, all 5 sections
- Library section fully wired: folder list (add/remove) + rescan button
- Audio section: PipeWire status badge (display only — always running)
- Hotkeys section: portal status + slot bindings table (read-only)
- Appearance section: section header only
- About section: version, license, credits, source link

**Does NOT ship in #11:**
- Theme switching (Issue A)
- Density system (Issue B)
- Mic passthrough toggle (Issue C)
- Monitor output selection (Issue D)
- Renderer selection (Issue E)

---

## Issue Dependency Graph

```
#11 settings panel shell
  └── blocked-by (for future settings entries):
        Issue A  feat(state,ui): theme persistence
        Issue B  feat(state,ui): density system
        Issue C  feat(audio): mic passthrough toggle
        Issue D  feat(audio): monitor output selection
        Issue E  feat(app): renderer selection
```

---

## Out of Scope

- List view (Phase 3)
- Favorites / per-sound volume / overlap mode (Phase 3)
- Sound effects panel (Phase 4)
- App audio passthrough (Phase 4)
- Any canvas sticker tile work (Phase 3)
