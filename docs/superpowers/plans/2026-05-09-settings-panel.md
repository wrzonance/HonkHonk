# Settings Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Phase 2 settings panel shell — a full-window swap with 5-section sidebar nav, a dynamic settings registry framework, and fully-wired library management (folder add/remove/rescan).

**Architecture:** `src/settings/mod.rs` is pure data (SettingId, SettingDef, SETTINGS_REGISTRY — zero app.rs imports). `src/ui/settings.rs` renders from the registry using `state: &HonkHonk` passed top-down; `get_setting_value` and `setting_message` live here. New `ViewMode::Settings` + `SettingsSection` in `app.rs` control navigation.

**Tech Stack:** Rust, Iced 0.13, ashpd 0.13 (adding `file_chooser` feature), existing `theme::Hh` trait for all colors.

**Spec:** `docs/superpowers/specs/2026-05-09-settings-panel-design.md`

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src/settings/mod.rs` | **Create** | Pure registry: SettingId, SettingCategory, ControlType, SettingValue, SettingDef, SETTINGS_REGISTRY |
| `src/lib.rs` | **Modify** | Add `pub mod settings` |
| `src/app.rs` | **Modify** | ViewMode::Settings, SettingsSection enum, settings_section field, 6 new Message variants, update() handlers |
| `Cargo.toml` | **Modify** | Add `"file_chooser"` to ashpd features |
| `src/ui/settings.rs` | **Create** | All settings view functions, get_setting_value, setting_message |
| `src/ui/mod.rs` | **Modify** | Add `pub mod settings` |

---

## Task 1: Settings registry module

**Files:**
- Create: `src/settings/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1.1: Write failing tests in src/settings/mod.rs**

Create the file with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescan_library_entry_exists_in_registry() {
        let found = SETTINGS_REGISTRY
            .iter()
            .any(|d| matches!(d.id, SettingId::RescanLibrary));
        assert!(found, "RescanLibrary must be in SETTINGS_REGISTRY");
    }

    #[test]
    fn rescan_library_is_in_library_category() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::RescanLibrary))
            .expect("RescanLibrary must exist");
        assert!(matches!(def.category, SettingCategory::Library));
    }

    #[test]
    fn rescan_library_control_is_button() {
        let def = SETTINGS_REGISTRY
            .iter()
            .find(|d| matches!(d.id, SettingId::RescanLibrary))
            .expect("RescanLibrary must exist");
        assert!(matches!(def.control, ControlType::Button));
    }

    #[test]
    fn audio_category_has_no_phase2_entries() {
        let count = SETTINGS_REGISTRY
            .iter()
            .filter(|d| matches!(d.category, SettingCategory::Audio))
            .count();
        assert_eq!(count, 0, "No audio settings wired in Phase 2 shell");
    }
}
```

- [ ] **Step 1.2: Run — expect compile error**

```bash
cargo test --lib 2>&1 | head -20
```

Expected: compile error (types undefined).

- [ ] **Step 1.3: Implement the registry**

Replace the file with the full implementation:

```rust
/// Central settings registry — pure metadata, zero coupling to app state.
/// To add a new setting: add to SettingId, add SettingDef to SETTINGS_REGISTRY,
/// add arms to get_setting_value and setting_message in src/ui/settings.rs,
/// add Message variant + update() handler in src/app.rs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingId {
    // Library — wired Phase 2
    RescanLibrary,
    // Appearance — wired when backends land (issues #69, #70)
    Theme,
    Density,
    // Audio — wired when backends land (issues #71, #72)
    MicPassthrough,
    MicPassthroughLevel,
    MonitorDevice,
    // App — wired when backend lands (issue #73)
    Renderer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingCategory {
    Audio,
    Library,
    Hotkeys,
    Appearance,
    About,
}

#[derive(Debug, Clone, Copy)]
pub enum ControlType {
    Toggle,
    Radio(&'static [&'static str]),
    Slider { min: f32, max: f32, step: f32 },
    Button,
    Select, // options loaded dynamically from state (MonitorDevice)
}

#[derive(Debug, Clone, Copy)]
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

/// Phase 2: only RescanLibrary wired.
/// Add a SettingDef here when its backend sub-MVP lands.
pub static SETTINGS_REGISTRY: &[SettingDef] = &[SettingDef {
    id: SettingId::RescanLibrary,
    category: SettingCategory::Library,
    label: "Scan now",
    hint: "Force a re-scan of all sound folders.",
    control: ControlType::Button,
}];
```

- [ ] **Step 1.4: Wire into src/lib.rs**

Add after the existing `pub mod` declarations:

```rust
pub mod settings;
```

- [ ] **Step 1.5: Run tests — expect pass**

```bash
cargo test --lib settings 2>&1
```

Expected: 4 tests pass.

- [ ] **Step 1.6: Commit**

```bash
git add src/settings/mod.rs src/lib.rs
git commit -m "feat(settings): central registry — SettingId, SettingDef, SETTINGS_REGISTRY"
```

---

## Task 2: App state — ViewMode::Settings, SettingsSection, navigation messages

**Files:**
- Modify: `src/app.rs` (lines 16–20, 23–57, 68–121, 153–221, 706–720, test block at 723+)

- [ ] **Step 2.1: Write failing tests**

In `src/app.rs`, add to the `mod tests` block (after line 724):

```rust
#[test]
fn show_settings_sets_view_mode() {
    let mut app = HonkHonk::new_for_test();
    app.update(Message::ShowSettings);
    assert!(matches!(app.view_mode, ViewMode::Settings));
}

#[test]
fn show_settings_defaults_section_to_audio() {
    let mut app = HonkHonk::new_for_test();
    app.update(Message::ShowSettings);
    assert!(matches!(app.settings_section, SettingsSection::Audio));
}

#[test]
fn show_settings_section_updates_active_section() {
    let mut app = HonkHonk::new_for_test();
    app.update(Message::ShowSettingsSection(SettingsSection::Library));
    assert!(matches!(app.settings_section, SettingsSection::Library));
}

#[test]
fn show_main_from_settings_resets_view_mode() {
    let mut app = HonkHonk::new_for_test();
    app.update(Message::ShowSettings);
    app.update(Message::ShowMain);
    assert!(matches!(app.view_mode, ViewMode::Main));
}
```

- [ ] **Step 2.2: Run — expect compile error**

```bash
cargo test --lib 2>&1 | head -20
```

Expected: compile error (`ViewMode::Settings`, `SettingsSection` undefined).

- [ ] **Step 2.3: Add ViewMode::Settings**

Edit lines 16–20 (the ViewMode enum):

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Main,
    SlotManager,
    Settings,
}
```

- [ ] **Step 2.4: Add SettingsSection enum**

Insert directly after the ViewMode enum (after line 20):

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    Audio,
    Library,
    Hotkeys,
    Appearance,
    About,
}
```

- [ ] **Step 2.5: Make needed HonkHonk fields pub(crate)**

`src/ui/settings.rs` is a separate module — it can't read private fields on `HonkHonk`. Find the field declarations in the struct (lines 68–121) and add `pub(crate)` to these four fields:

```rust
pub(crate) config: AppConfig,
pub(crate) shortcuts_status: ShortcutsStatus,
pub(crate) slot_triggers: [Option<String>; 20],
pub(crate) sounds: Vec<SoundEntry>,
```

These are already private, so existing code (all inside `app.rs`) is unaffected. Only the new `settings.rs` module needs cross-module read access.

- [ ] **Step 2.6: Add settings_section field to HonkHonk struct**

Find the `selected_slot` field. Add after it:

```rust
pub(crate) settings_section: SettingsSection,
```

- [ ] **Step 2.7: Add settings_section to new()**

In `new()` struct literal (around line 167), add after `selected_slot: None`:

```rust
settings_section: SettingsSection::default(),
```

- [ ] **Step 2.8: Add settings_section to new_for_test()**

In `new_for_test()` struct literal (around line 196), add after `selected_slot: None`:

```rust
settings_section: SettingsSection::default(),
```

- [ ] **Step 2.9: Add Message variants**

In the `Message` enum (lines 23–57), add after the `// Navigation` section:

```rust
// Settings navigation
ShowSettings,
ShowSettingsSection(SettingsSection),
// Library management
RescanLibrary,
AddSoundDirectory,
SoundDirectoryPickResult(Option<std::path::PathBuf>),
RemoveSoundDirectory(std::path::PathBuf),
```

- [ ] **Step 2.10: Add update() handlers for navigation**

In `update()`, find the `Message::ShowSlots` arm and add after `Message::ShowMain`:

```rust
Message::ShowSettings => {
    self.view_mode = ViewMode::Settings;
    self.settings_section = SettingsSection::Audio;
    Task::none()
}
Message::ShowSettingsSection(section) => {
    self.settings_section = section;
    Task::none()
}
```

- [ ] **Step 2.10: Run tests — expect pass**

```bash
cargo test --lib 2>&1
```

Expected: 4 new tests pass, all existing tests pass.

- [ ] **Step 2.11: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): ViewMode::Settings + SettingsSection + navigation message handlers"
```

---

## Task 3: Library message handlers

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app.rs`

- [ ] **Step 3.1: Write failing tests**

Add to the test block in `src/app.rs`:

```rust
#[test]
fn rescan_library_resets_durations_loaded() {
    let mut app = HonkHonk::new_for_test();
    app.durations_loaded = true;
    app.update(Message::RescanLibrary);
    assert!(!app.durations_loaded, "RescanLibrary must reset durations_loaded");
}

#[test]
fn remove_sound_directory_removes_path() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/tmp/hh_test_sounds");
    app.config.sound_directories.push(path.clone());
    app.update(Message::RemoveSoundDirectory(path.clone()));
    assert!(!app.config.sound_directories.contains(&path));
}

#[test]
fn sound_directory_pick_some_appends_to_config() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/tmp/hh_new_sounds");
    let before = app.config.sound_directories.len();
    app.update(Message::SoundDirectoryPickResult(Some(path.clone())));
    assert_eq!(app.config.sound_directories.len(), before + 1);
    assert!(app.config.sound_directories.contains(&path));
}

#[test]
fn sound_directory_pick_none_is_noop() {
    let mut app = HonkHonk::new_for_test();
    let before = app.config.sound_directories.clone();
    app.update(Message::SoundDirectoryPickResult(None));
    assert_eq!(app.config.sound_directories, before);
}
```

- [ ] **Step 3.2: Run — expect compile error**

```bash
cargo test --lib 2>&1 | head -20
```

Expected: compile error (handlers not implemented yet).

- [ ] **Step 3.3: Add file_chooser feature to Cargo.toml**

Find the ashpd line:

```toml
ashpd = { version = "0.13", features = ["global_shortcuts", "raw-window-handle"] }
```

Change to:

```toml
ashpd = { version = "0.13", features = ["global_shortcuts", "raw-window-handle", "file_chooser"] }
```

- [ ] **Step 3.4: Add pick_directory async helper**

In `src/app.rs`, add this free function before the `impl HonkHonk` block (around line 152):

```rust
async fn pick_directory() -> Option<std::path::PathBuf> {
    use ashpd::desktop::file_chooser::OpenFileRequest;
    let response = OpenFileRequest::default()
        .title("Select Sound Folder")
        .directory(true)
        .send()
        .await
        .ok()?;
    let files = response.response().ok()?;
    files
        .uris()
        .first()
        .and_then(|uri| uri.to_file_path().ok())
}
```

- [ ] **Step 3.5: Add update() handlers for library messages**

In `update()`, after the `ShowSettingsSection` arm, add:

```rust
Message::RescanLibrary => {
    let new_sounds =
        crate::state::Library::scan(&self.config.sound_directories).unwrap_or_default();
    let pairs: Vec<(String, std::path::PathBuf)> = new_sounds
        .iter()
        .map(|s| (s.id.clone(), s.path.clone()))
        .collect();
    self.sounds = new_sounds;
    self.duration_scan_pairs = std::sync::Arc::new(pairs);
    self.durations_loaded = false;
    Task::none()
}
Message::AddSoundDirectory => {
    Task::perform(pick_directory(), Message::SoundDirectoryPickResult)
}
Message::SoundDirectoryPickResult(Some(path)) => {
    self.config.sound_directories.push(path);
    let _ = self.config.save();
    self.update(Message::RescanLibrary)
}
Message::SoundDirectoryPickResult(None) => Task::none(),
Message::RemoveSoundDirectory(path) => {
    self.config.sound_directories.retain(|p| p != &path);
    let _ = self.config.save();
    self.update(Message::RescanLibrary)
}
```

- [ ] **Step 3.6: Run tests — expect pass**

```bash
cargo test --lib 2>&1
```

Expected: all 4 new tests pass, all existing pass.

- [ ] **Step 3.7: Verify build**

```bash
cargo build 2>&1
```

Expected: clean build.

- [ ] **Step 3.8: Commit**

```bash
git add src/app.rs Cargo.toml Cargo.lock
git commit -m "feat(app): RescanLibrary + AddSoundDirectory + RemoveSoundDirectory handlers"
```

---

## Task 4: Settings UI skeleton

**Files:**
- Create: `src/ui/settings.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/app.rs` (view() at line 706, view_header() at line 496)

- [ ] **Step 4.1: Add pub mod settings to src/ui/mod.rs**

Add to `src/ui/mod.rs`:

```rust
pub mod settings;
```

- [ ] **Step 4.2: Create src/ui/settings.rs**

```rust
use iced::{
    widget::{button, column, container, row, scrollable, text, Space},
    Alignment, Element, Length,
};

use crate::app::{HonkHonk, Message, SettingsSection};
use crate::settings::{ControlType, SettingCategory, SettingDef, SettingId, SettingValue, SETTINGS_REGISTRY};
use crate::ui::theme::{self, Hh, Theme};
// Theme here is crate::ui::theme::Theme (Dark/Light enum with Hh trait),
// NOT iced::Theme. All color calls go through the Hh trait: t.ink(), t.bg(), etc.

/// Top-level settings view — full window swap.
/// Receives the full app state so section views can read any field they need.
pub fn view_settings(state: &HonkHonk, t: Theme) -> Element<'_, Message> {
    let header = settings_header(t);
    let sidebar = settings_sidebar(&state.settings_section, t);
    let content = settings_content(state, t);

    let body = row![sidebar, content].height(Length::Fill);

    column![header, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn settings_header(t: Theme) -> Element<'static, Message> {
    let back_btn = button(
        row![
            text("←").size(14).color(t.ink()),
            text("Back to sounds").size(13).color(t.ink()),
        ]
        .spacing(theme::space::SM as u16)
        .align_y(Alignment::Center),
    )
    .on_press(Message::ShowMain)
    .padding([8, 14])
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.panel())),
        border: theme::tile_border(t.hairline2(), 1.0),
        ..Default::default()
    });

    let title = row![
        text("Settings")
            .size(22)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .color(t.ink()),
        text(" ruffle feathers").size(12).color(t.ink_dim()),
    ]
    .spacing(theme::space::MD as u16)
    .align_y(Alignment::Center);

    container(
        row![back_btn, title]
            .spacing(theme::space::LG as u16)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([theme::space::MD as u16, theme::space::XL as u16])
    .style(move |_t| container::Style {
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: iced::border::Radius::default(),
        },
        ..Default::default()
    })
    .into()
}

fn settings_sidebar<'a>(active: &'a SettingsSection, t: Theme) -> Element<'a, Message> {
    let items: &[(&str, SettingsSection)] = &[
        ("Audio", SettingsSection::Audio),
        ("Library", SettingsSection::Library),
        ("Hotkeys", SettingsSection::Hotkeys),
        ("Appearance", SettingsSection::Appearance),
        ("About", SettingsSection::About),
    ];

    let nav = items.iter().fold(
        column![].spacing(theme::space::XS as u16),
        |col, (label, section)| {
            let is_active = active == section;
            let item = button(
                text(*label)
                    .size(13)
                    .color(if is_active { t.bg() } else { t.ink() })
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
            )
            .on_press(Message::ShowSettingsSection(section.clone()))
            .width(Length::Fill)
            .padding([theme::space::SM as u16, theme::space::MD as u16])
            .style(move |_t, _s| button::Style {
                background: Some(theme::bg_color(if is_active {
                    t.ink()
                } else {
                    iced::Color::TRANSPARENT
                })),
                border: theme::tile_border(iced::Color::TRANSPARENT, 0.0),
                ..Default::default()
            });
            col.push(item)
        },
    );

    container(column![nav].width(Length::Fixed(220.0)))
        .height(Length::Fill)
        .padding(theme::space::MD as u16)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: iced::Border {
                color: t.hairline(),
                width: 1.0,
                radius: iced::border::Radius::default(),
            },
            ..Default::default()
        })
        .into()
}

fn settings_content<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    let body: Element<'_, Message> = match &state.settings_section {
        SettingsSection::Audio => view_audio_section(state, t),
        SettingsSection::Library => view_library_section(state, t),
        SettingsSection::Hotkeys => view_hotkeys_section(state, t),
        SettingsSection::Appearance => view_appearance_section(t),
        SettingsSection::About => view_about_section(t),
    };

    scrollable(
        container(body)
            .width(Length::Fill)
            .padding([theme::space::XL as u16, theme::space::XXL as u16]),
    )
    .height(Length::Fill)
    .into()
}

/// Generic registry row renderer.
/// Left col: label + hint. Right col: control widget. Separated by hairline bottom border.
fn render_setting_row<'a>(def: &'a SettingDef, state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    let value = get_setting_value(def.id, state);

    let label_col = column![
        text(def.label)
            .size(13)
            .color(t.ink())
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
        text(def.hint).size(11).color(t.ink_dim()),
    ]
    .spacing(theme::space::XS as u16)
    .width(Length::Fixed(260.0));

    let control: Element<'_, Message> = match (&def.control, value) {
        (ControlType::Button, _) => {
            let msg = setting_message(def.id, SettingValue::None);
            button(text(def.label).size(13).color(t.ink()))
                .on_press(msg)
                .padding([8, 18])
                .style(move |_t, _s| button::Style {
                    background: Some(theme::bg_color(t.panel())),
                    border: theme::tile_border(t.hairline2(), 1.0),
                    ..Default::default()
                })
                .into()
        }
        // Toggle, Radio, Slider, Select arms added here as backends land.
        _ => text("—").size(13).color(t.ink_faint()).into(),
    };

    let row_inner = row![label_col, control]
        .spacing(theme::space::XL as u16)
        .align_y(Alignment::Start)
        .width(Length::Fill);

    container(row_inner)
        .width(Length::Fill)
        .padding([18, 0])
        .style(move |_t| container::Style {
            border: iced::Border {
                color: t.hairline(),
                width: 0.0,
                radius: iced::border::Radius::default(),
            },
            ..Default::default()
        })
        .into()
}

/// Read the current value of a setting from app state.
/// Add arms here when backend sub-MVPs land.
pub fn get_setting_value(id: SettingId, _state: &HonkHonk) -> SettingValue {
    match id {
        SettingId::RescanLibrary => SettingValue::None,
        // Future:
        // SettingId::Theme => SettingValue::Index(state.config.theme as usize),
        // SettingId::Density => SettingValue::Index(state.config.density as usize),
        // SettingId::MicPassthrough => SettingValue::Bool(state.config.mic_passthrough),
        _ => SettingValue::None,
    }
}

/// Map a setting id + new value to the specific Message that applies it.
/// Add arms here when backend sub-MVPs land.
pub fn setting_message(id: SettingId, _value: SettingValue) -> Message {
    match id {
        SettingId::RescanLibrary => Message::RescanLibrary,
        // Future:
        // SettingId::Theme => Message::ThemeChanged(Theme::from_index(value.as_index())),
        // SettingId::MicPassthrough => Message::MicPassthroughChanged(value.as_bool()),
        _ => Message::RescanLibrary, // unreachable in Phase 2 — only RescanLibrary in registry
    }
}

/// Shared section layout: bold italic title + subtitle + body content.
fn section_layout<'a>(
    title: &'static str,
    subtitle: &'static str,
    body: Element<'a, Message>,
    t: Theme,
) -> Element<'a, Message> {
    column![
        column![
            text(title)
                .size(26)
                .color(t.ink())
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    style: iced::font::Style::Italic,
                    ..Default::default()
                }),
            text(subtitle).size(13).color(t.ink_dim()),
        ]
        .spacing(theme::space::XS as u16)
        .width(Length::Fill),
        container(Space::new(Length::Fill, 2.0))
            .width(Length::Fill)
            .style(move |_t| container::Style {
                background: Some(theme::bg_color(t.ink())),
                ..Default::default()
            }),
        body,
    ]
    .spacing(theme::space::LG as u16)
    .width(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Section view stubs — fleshed out in Tasks 5 and 6
// ---------------------------------------------------------------------------

fn view_audio_section<'a>(_state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    section_layout("Audio", "Where HonkHonk listens and speaks.", column![].into(), t)
}

fn view_library_section<'a>(_state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    section_layout("Library", "Where HonkHonk looks for your sounds.", column![].into(), t)
}

fn view_hotkeys_section<'a>(_state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    section_layout(
        "Hotkeys",
        "Global shortcuts that work even when HonkHonk isn't focused.",
        column![].into(),
        t,
    )
}

fn view_appearance_section(t: Theme) -> Element<'static, Message> {
    section_layout("Appearance", "How honky should HonkHonk look today?", column![].into(), t)
}

fn view_about_section(t: Theme) -> Element<'static, Message> {
    section_layout("About", "The bird is the word.", column![].into(), t)
}
```

- [ ] **Step 4.3: Wire ViewMode::Settings into app.rs view()**

In `view()` at line 706, change the match to:

```rust
pub fn view(&self) -> Element<'_, Message> {
    match self.view_mode {
        ViewMode::Main => self.view_main(),
        ViewMode::SlotManager => {
            let t = theme::Theme::Dark;
            slot_manager::view_slot_manager(
                &self.slots,
                &self.slot_triggers,
                &self.sounds,
                self.selected_slot,
                t,
            )
        }
        ViewMode::Settings => {
            crate::ui::settings::view_settings(self, theme::Theme::Dark)
        }
    }
}
```

- [ ] **Step 4.4: Add Settings button to view_header()**

In `view_header()` at line 496, after the `slots_btn` definition (after line 506), add:

```rust
let settings_btn = button(text("Settings").size(14).color(t.ink()))
    .on_press(Message::ShowSettings)
    .style(move |_theme, _status| button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink(),
        border: theme::tile_border(t.hairline(), 1.0),
        ..Default::default()
    });
```

Then update the row at line 519 to include it:

```rust
row![title, slots_btn, settings_btn, space::horizontal(), search, stop_btn]
    .spacing(theme::space::LG)
    .align_y(iced::Alignment::Center)
    .into()
```

- [ ] **Step 4.5: Verify build**

```bash
cargo build 2>&1
```

Expected: clean build. Fix any import or type errors.

- [ ] **Step 4.6: Commit**

```bash
git add src/ui/settings.rs src/ui/mod.rs src/app.rs
git commit -m "feat(ui): settings panel skeleton — sidebar, section dispatch, render_setting_row"
```

---

## Task 5: Audio, Hotkeys, Appearance, About section views

**Files:**
- Modify: `src/ui/settings.rs` (replace stubs)

- [ ] **Step 5.1: Replace view_audio_section stub**

```rust
fn view_audio_section<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    // Status badge — engine is always active if user reached settings
    let dot = container(Space::new(8.0, 8.0))
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.good())),
            border: iced::Border {
                radius: iced::border::Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        });

    let status_badge = container(
        column![
            row![dot, text("Audio engine active").size(12).color(t.ink()).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })]
            .spacing(theme::space::SM as u16)
            .align_y(Alignment::Center),
            text("honkhonk-mix · honkhonk-mic")
                .size(11)
                .color(t.ink_dim())
                .font(iced::Font {
                    family: iced::font::Family::Monospace,
                    ..Default::default()
                }),
        ]
        .spacing(theme::space::XS as u16),
    )
    .padding(theme::space::MD as u16)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.panel())),
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: theme::radius::MD,
        },
        ..Default::default()
    });

    // Registry rows for Audio category — empty in Phase 2, populated by issues #71/#72
    let registry_rows = SETTINGS_REGISTRY
        .iter()
        .filter(|d| matches!(d.category, SettingCategory::Audio))
        .fold(column![].spacing(0), |col, def| {
            col.push(render_setting_row(def, state, t))
        });

    section_layout(
        "Audio",
        "Where HonkHonk listens and speaks.",
        column![status_badge, registry_rows]
            .spacing(theme::space::LG as u16)
            .into(),
        t,
    )
}
```

- [ ] **Step 5.2: Replace view_hotkeys_section stub**

```rust
fn view_hotkeys_section<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    use crate::shortcuts::ShortcutsStatus;

    // Portal status badge
    let (dot_color, status_label) = match &state.shortcuts_status {
        ShortcutsStatus::Active => (t.good(), "Connected to KDE portal"),
        ShortcutsStatus::Initializing => (t.ink_dim(), "Connecting to portal…"),
        ShortcutsStatus::Unavailable(_) => (t.accent(), "Portal unavailable"),
    };

    let dot = container(Space::new(8.0, 8.0))
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(dot_color)),
            border: iced::Border {
                radius: iced::border::Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        });

    let portal_badge = container(
        row![dot, text(status_label).size(12).color(t.ink()).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        })]
        .spacing(theme::space::SM as u16)
        .align_y(Alignment::Center),
    )
    .padding(theme::space::MD as u16)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.panel())),
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: theme::radius::MD,
        },
        ..Default::default()
    });

    // Slot bindings table — read-only display of current hotkey assignments
    let bound_slots: Vec<(u8, &str)> = state
        .slot_triggers
        .iter()
        .enumerate()
        .filter_map(|(i, opt)| opt.as_deref().map(|s| (i as u8, s)))
        .collect();

    let binding_rows: Vec<Element<'_, Message>> = if bound_slots.is_empty() {
        vec![text("No hotkeys assigned yet. Use the Slot Manager to bind sounds.")
            .size(12)
            .color(t.ink_dim())
            .into()]
    } else {
        bound_slots
            .into_iter()
            .map(|(slot, trigger)| {
                container(
                    row![
                        text(format!("Slot {}", slot + 1))
                            .size(12)
                            .color(t.ink_dim())
                            .width(Length::Fixed(60.0)),
                        text(trigger)
                            .size(12)
                            .color(t.ink())
                            .font(iced::Font {
                                family: iced::font::Family::Monospace,
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                    ]
                    .spacing(theme::space::MD as u16)
                    .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .style(move |_t| container::Style {
                    background: Some(theme::bg_color(t.panel())),
                    border: iced::Border {
                        color: t.hairline(),
                        width: 1.0,
                        radius: theme::radius::MD,
                    },
                    ..Default::default()
                })
                .into()
            })
            .collect()
    };

    let bindings_list = Column::with_children(binding_rows).spacing(theme::space::XS as u16);

    // No registry rows for Hotkeys in Phase 2
    section_layout(
        "Hotkeys",
        "Global shortcuts that work even when HonkHonk isn't focused.",
        column![portal_badge, bindings_list]
            .spacing(theme::space::LG as u16)
            .into(),
        t,
    )
}
```

Add the `Column` import at the top of the file (alongside existing widget imports):

```rust
use iced::widget::Column;
```

- [ ] **Step 5.3: Replace view_appearance_section stub**

The Appearance section has no registry entries in Phase 2. Show only the section header — no placeholder copy.

```rust
fn view_appearance_section(t: Theme) -> Element<'static, Message> {
    // Registry rows for Appearance — empty until issues #69 (theme) and #70 (density) land
    let registry_rows = SETTINGS_REGISTRY
        .iter()
        .filter(|d| matches!(d.category, SettingCategory::Appearance))
        .fold(column![].spacing(0), |col, _def| col);

    section_layout(
        "Appearance",
        "How honky should HonkHonk look today?",
        registry_rows.into(),
        t,
    )
}
```

- [ ] **Step 5.4: Replace view_about_section stub**

```rust
fn view_about_section(t: Theme) -> Element<'static, Message> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let logo_block = column![
        text("HonkHonk")
            .size(28)
            .color(t.ink())
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                style: iced::font::Style::Italic,
                ..Default::default()
            }),
        text(format!("v{VERSION} · Iced 0.13"))
            .size(13)
            .color(t.ink_dim()),
        text("A soundboard for KDE. Built with Rust, Iced, and PipeWire.")
            .size(12)
            .color(t.ink_faint()),
    ]
    .spacing(theme::space::XS as u16);

    let license_row = row![
        text("License").size(13).color(t.ink()).width(Length::Fixed(260.0)).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        container(
            text("GPL-3.0-or-later")
                .size(12)
                .color(t.ink())
                .font(iced::Font {
                    family: iced::font::Family::Monospace,
                    ..Default::default()
                }),
        )
        .padding([4, 10])
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: iced::Border {
                color: t.hairline(),
                width: 1.0,
                radius: theme::radius::MD,
            },
            ..Default::default()
        }),
    ]
    .spacing(theme::space::XL as u16)
    .align_y(Alignment::Center);

    let credits = column![
        text("Credits").size(13).color(t.ink()).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        text("Iced — iced-rs").size(12).color(t.ink_dim()),
        text("Symphonia — pdeljanov").size(12).color(t.ink_dim()),
        text("ashpd — bilelmoussaoui").size(12).color(t.ink_dim()),
        text("pipewire-rs — PipeWire project").size(12).color(t.ink_dim()),
        text("tray-icon — tauri-apps").size(12).color(t.ink_dim()),
    ]
    .spacing(theme::space::XS as u16);

    section_layout(
        "About",
        "The bird is the word.",
        column![logo_block, license_row, credits]
            .spacing(theme::space::XL as u16)
            .into(),
        t,
    )
}
```

- [ ] **Step 5.5: Verify build**

```bash
cargo build 2>&1
```

Expected: clean build. Fix any missing imports (`ShortcutsStatus`, `Column`, etc.).

- [ ] **Step 5.6: Commit**

```bash
git add src/ui/settings.rs
git commit -m "feat(ui): settings Audio, Hotkeys, Appearance, About section views"
```

---

## Task 6: Library section view

**Files:**
- Modify: `src/ui/settings.rs` (replace view_library_section stub)

The Library section is the most complex — it has a folder list (dynamic, with remove buttons) plus registry-driven rows (RescanLibrary button) plus a static format pill list.

- [ ] **Step 6.1: Replace view_library_section stub**

```rust
fn view_library_section<'a>(state: &'a HonkHonk, t: Theme) -> Element<'a, Message> {
    // Folder list — one row per configured sound directory
    let folder_rows: Vec<Element<'_, Message>> = state
        .config
        .sound_directories
        .iter()
        .map(|path| {
            let path_clone = path.clone();
            let remove_btn = button(text("×").size(14).color(t.ink_faint()))
                .on_press(Message::RemoveSoundDirectory(path_clone))
                .padding([4, 8])
                .style(move |_t, _s| button::Style {
                    background: None,
                    border: iced::Border::default(),
                    ..Default::default()
                });

            container(
                row![
                    text(path.display().to_string())
                        .size(12)
                        .color(t.ink())
                        .font(iced::Font {
                            family: iced::font::Family::Monospace,
                            ..Default::default()
                        })
                        .width(Length::Fill),
                    remove_btn,
                ]
                .spacing(theme::space::SM as u16)
                .align_y(Alignment::Center),
            )
            .padding([10, 12])
            .width(Length::Fill)
            .style(move |_t| container::Style {
                background: Some(theme::bg_color(t.panel())),
                border: iced::Border {
                    color: t.hairline(),
                    width: 1.0,
                    radius: theme::radius::MD,
                },
                ..Default::default()
            })
            .into()
        })
        .collect();

    let add_btn = button(text("+ Add a folder").size(13).color(t.ink_dim()))
        .on_press(Message::AddSoundDirectory)
        .width(Length::Fill)
        .padding([9, 12])
        .style(move |_t, _s| button::Style {
            background: None,
            border: iced::Border {
                color: t.hairline2(),
                width: 1.5,
                radius: theme::radius::MD,
            },
            ..Default::default()
        });

    let folders_widget = column![
        Column::with_children(folder_rows).spacing(theme::space::XS as u16),
        add_btn,
    ]
    .spacing(theme::space::XS as u16)
    .width(Length::Fixed(540.0));

    let folders_row = row![
        column![
            text("Sound folders")
                .size(13)
                .color(t.ink())
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            text("HonkHonk watches these folders. Drop in MP3 / WAV / OGG / FLAC.")
                .size(11)
                .color(t.ink_dim()),
        ]
        .spacing(theme::space::XS as u16)
        .width(Length::Fixed(260.0)),
        folders_widget,
    ]
    .spacing(theme::space::XL as u16)
    .align_y(Alignment::Start)
    .width(Length::Fill);

    // Registry rows for Library category (RescanLibrary button)
    let registry_rows = SETTINGS_REGISTRY
        .iter()
        .filter(|d| matches!(d.category, SettingCategory::Library))
        .fold(column![].spacing(0), |col, def| {
            col.push(render_setting_row(def, state, t))
        });

    // Supported formats — static display
    const FORMATS: &[&str] = &["MP3", "WAV", "OGG Vorbis", "FLAC", "AAC", "Opus"];

    let format_pills: Vec<Element<'_, Message>> = FORMATS
        .iter()
        .map(|fmt| {
            container(
                text(*fmt)
                    .size(11)
                    .color(t.ink_dim())
                    .font(iced::Font {
                        family: iced::font::Family::Monospace,
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
            )
            .padding([5, 11])
            .style(move |_t| container::Style {
                background: Some(theme::bg_color(t.panel())),
                border: iced::Border {
                    color: t.hairline2(),
                    width: 1.0,
                    radius: theme::radius::PILL,
                },
                ..Default::default()
            })
            .into()
        })
        .collect();

    let formats_row = row![
        column![
            text("Supported formats")
                .size(13)
                .color(t.ink())
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            text("Decoded via Symphonia — pure Rust.")
                .size(11)
                .color(t.ink_dim()),
        ]
        .spacing(theme::space::XS as u16)
        .width(Length::Fixed(260.0)),
        Row::with_children(format_pills).spacing(theme::space::XS as u16),
    ]
    .spacing(theme::space::XL as u16)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    section_layout(
        "Library",
        "Where HonkHonk looks for your sounds.",
        column![folders_row, registry_rows, formats_row]
            .spacing(theme::space::LG as u16)
            .into(),
        t,
    )
}
```

Add `Row` to imports at the top of `src/ui/settings.rs`:

```rust
use iced::widget::{button, column, container, row, scrollable, text, Column, Row, Space};
```

- [ ] **Step 6.2: Verify build**

```bash
cargo build 2>&1
```

Expected: clean build.

- [ ] **Step 6.3: Manual smoke test**

```bash
cargo run
```

1. Main window opens — check "Settings" button visible in header
2. Click "Settings" → full-window swap, sidebar with 5 nav items
3. Click each sidebar item → content area switches
4. Audio: status badge shows "Audio engine active"
5. Library: folder list shows configured dirs (or empty), "+ Add a folder" button present, "Scan now" button present, format pills visible
6. Hotkeys: portal status badge + slot bindings table (or empty message)
7. Appearance: section header only, no other content
8. About: version, license, credits
9. "← Back to sounds" button → returns to main grid

- [ ] **Step 6.4: Commit**

```bash
git add src/ui/settings.rs
git commit -m "feat(ui): settings Library section — folder list, add/remove, scan now, format pills"
```

---

## Task 7: Final verification pass

**Files:** read-only check

- [ ] **Step 7.1: Lint**

```bash
cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings. Fix any clippy complaints before continuing.

- [ ] **Step 7.2: Format**

```bash
cargo fmt -- --check 2>&1
```

If violations, run:

```bash
cargo fmt
```

- [ ] **Step 7.3: Full test suite**

```bash
cargo test 2>&1
```

Expected: all tests pass (includes the 12 new tests from Tasks 1–3 and all pre-existing tests).

- [ ] **Step 7.4: Commit any cleanup**

If fmt or clippy required fixes:

```bash
git add -p
git commit -m "chore(settings): clippy + fmt cleanup"
```

- [ ] **Step 7.5: Verify LOC delta is within 500**

```bash
git diff main...HEAD --stat | tail -3
```

Expected: ≤ 500 lines changed (excluding Cargo.lock). If over, review what grew large and split.

---

## Summary

After all tasks complete:

| Component | Status |
|---|---|
| `src/settings/mod.rs` | Created — pure registry, 1 wired entry (RescanLibrary) |
| `src/ui/settings.rs` | Created — all 5 sections, generic row renderer, get_setting_value, setting_message |
| `src/app.rs` | ViewMode::Settings, SettingsSection, 6 new messages + handlers |
| `src/ui/mod.rs` | pub mod settings added |
| `src/lib.rs` | pub mod settings added |
| `Cargo.toml` | ashpd file_chooser feature added |

**Extension contract for future backend sub-MVPs:**
1. Add `SettingDef` to `SETTINGS_REGISTRY` in `src/settings/mod.rs`
2. Add arm to `get_setting_value` in `src/ui/settings.rs`
3. Add arm to `setting_message` in `src/ui/settings.rs`
4. Add `Message` variant + `update()` handler in `src/app.rs`
5. UI auto-renders the new control. No other changes needed.

**Remaining backend issues (tracked separately):**
- #69 Theme persistence + live switching
- #70 Density system
- #71 Mic passthrough toggle
- #72 Monitor output device selection
- #73 Renderer selection
