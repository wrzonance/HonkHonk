# Sound Grid + Click to Play — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the sound library as a clickable grid of Confetti-themed tiles. Click tile → decode → play to virtual sink + headset. Stop-all button in header. Category chip bar for filtering.

**Architecture:** New `src/ui/` module provides Confetti theme colors and grid layout. `app.rs` gains full state (sounds, playing ID, active category, config) and wires click events to the existing audio engine via `AudioCommand::Play`. Library scan + config load happen in `main.rs` before Iced launches. Decoding is synchronous in the update handler (acceptable for short clips).

**Tech Stack:** Iced 0.14 (container, button, row, column, scrollable, text), existing audio engine + decoder, Confetti theme palette from design reference.

**Issue:** #7

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `src/ui/mod.rs` | Re-exports theme + grid |
| Create | `src/ui/theme.rs` | Confetti color palette, Tone enum, Hh trait, space/radius constants (~150 lines) |
| Create | `src/ui/sound_grid.rs` | Grid layout function, tile styling, category chips (~180 lines) |
| Modify | `src/state/library.rs` | Add `category: String` field to `SoundEntry`, derive from parent dir |
| Modify | `src/lib.rs` | Add `pub mod ui;` |
| Modify | `src/app.rs` | Full state (sounds, playing, config, category), new Messages, grid view |
| Modify | `src/main.rs` | Load config, scan library, pass to app |
| Modify | `tests/app_test.rs` | Tests for PlaySound, StopAll, CategorySelected messages |

---

## Task 1: Create `src/ui/theme.rs` — Confetti Theme Palette

**Files:**
- Create: `src/ui/theme.rs`

- [ ] **Step 1: Create `src/ui/theme.rs`**

Adapted from `docs/design-reference/src-rust/ui/theme.rs`. Trimmed to what Phase 1 uses (no canvas imports):

```rust
use iced::{Background, Border, Color, border::Radius};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn is_dark(self) -> bool {
        matches!(self, Theme::Dark)
    }
}

pub mod space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

pub mod radius {
    use iced::border::Radius;
    pub const SM: Radius = Radius {
        top_left: 8.0,
        top_right: 8.0,
        bottom_left: 8.0,
        bottom_right: 8.0,
    };
    pub const MD: Radius = Radius {
        top_left: 12.0,
        top_right: 12.0,
        bottom_left: 12.0,
        bottom_right: 12.0,
    };
    pub const TILE: Radius = Radius {
        top_left: 20.0,
        top_right: 20.0,
        bottom_left: 20.0,
        bottom_right: 20.0,
    };
    pub const PILL: Radius = Radius {
        top_left: 999.0,
        top_right: 999.0,
        bottom_left: 999.0,
        bottom_right: 999.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Amber,
    Orange,
    Yellow,
    Lime,
    Cyan,
    Blue,
    Pink,
    Red,
    Purple,
    Gray,
}

const TONES: [Tone; 10] = [
    Tone::Amber,
    Tone::Orange,
    Tone::Yellow,
    Tone::Lime,
    Tone::Cyan,
    Tone::Blue,
    Tone::Pink,
    Tone::Red,
    Tone::Purple,
    Tone::Gray,
];

impl Tone {
    pub fn from_index(idx: usize) -> Self {
        TONES[idx % TONES.len()]
    }

    fn hsl(self) -> (f32, f32, f32) {
        match self {
            Tone::Amber => (38.0, 95.0, 55.0),
            Tone::Orange => (22.0, 90.0, 56.0),
            Tone::Yellow => (50.0, 95.0, 55.0),
            Tone::Lime => (95.0, 65.0, 50.0),
            Tone::Cyan => (190.0, 75.0, 50.0),
            Tone::Blue => (220.0, 70.0, 56.0),
            Tone::Pink => (340.0, 80.0, 60.0),
            Tone::Red => (0.0, 75.0, 55.0),
            Tone::Purple => (270.0, 60.0, 60.0),
            Tone::Gray => (220.0, 8.0, 55.0),
        }
    }

    pub fn tile_tint(self, dark: bool) -> Color {
        let (h, s, _) = self.hsl();
        if dark {
            hsl_to_color(h, s.min(40.0) / 100.0, 0.13)
        } else {
            hsl_to_color(h, s.min(60.0) / 100.0, 0.93)
        }
    }
}

pub trait Hh {
    fn bg(self) -> Color;
    fn panel(self) -> Color;
    fn ink(self) -> Color;
    fn ink_dim(self) -> Color;
    fn ink_faint(self) -> Color;
    fn hairline(self) -> Color;
    fn accent(self) -> Color;
}

impl Hh for Theme {
    fn bg(self) -> Color {
        match self {
            Theme::Light => hex(0xf4efe4),
            Theme::Dark => hex(0x171410),
        }
    }
    fn panel(self) -> Color {
        match self {
            Theme::Light => hex(0xfffaf0),
            Theme::Dark => hex(0x1f1c16),
        }
    }
    fn ink(self) -> Color {
        match self {
            Theme::Light => hex(0x1a1208),
            Theme::Dark => hex(0xfbf3df),
        }
    }
    fn ink_dim(self) -> Color {
        match self {
            Theme::Light => hex(0x6a553a),
            Theme::Dark => hex(0xa39377),
        }
    }
    fn ink_faint(self) -> Color {
        match self {
            Theme::Light => hex(0xa8957a),
            Theme::Dark => hex(0x6a5b46),
        }
    }
    fn hairline(self) -> Color {
        match self {
            Theme::Light => Color::from_rgba(0.0, 0.0, 0.0, 0.06),
            Theme::Dark => Color::from_rgba(1.0, 1.0, 1.0, 0.06),
        }
    }
    fn accent(self) -> Color {
        hex(0xfbbf24)
    }
}

pub fn bg_color(c: Color) -> Background {
    Background::Color(c)
}

pub fn tile_border(color: Color, width: f32) -> Border {
    Border {
        color,
        width,
        radius: radius::TILE,
    }
}

fn hex(rgb: u32) -> Color {
    let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
    let b = (rgb & 0xff) as f32 / 255.0;
    Color::from_rgb(r, g, b)
}

fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h6 = (h / 60.0).rem_euclid(6.0);
    let x = c * (1.0 - (h6.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    Color::from_rgb(r1 + m, g1 + m, b1 + m)
}
```

- [ ] **Step 2: Verify it compiles in isolation**

Run: `cargo check`
Expected: Compiles (will fail until mod.rs and lib.rs updated — that's Task 2)

---

## Task 2: Create `src/ui/mod.rs` and Register Module

**Files:**
- Create: `src/ui/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/ui/mod.rs`**

```rust
pub mod sound_grid;
pub mod theme;
```

- [ ] **Step 2: Add `pub mod ui;` to `src/lib.rs`**

File becomes:

```rust
pub mod app;
pub mod audio;
pub mod state;
pub mod tray;
pub mod ui;
```

- [ ] **Step 3: Create stub `src/ui/sound_grid.rs`**

```rust
use iced::Element;

use crate::app::Message;

pub fn view_grid(_sounds: &[crate::state::SoundEntry], _playing: Option<&str>, _category: Option<&str>) -> Element<'static, Message> {
    iced::widget::text("grid placeholder").into()
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src/ui/mod.rs src/ui/theme.rs src/ui/sound_grid.rs src/lib.rs
git commit -m "feat(ui): add theme module with Confetti palette and grid stub"
```

---

## Task 3: Add `category` Field to `SoundEntry`

**Files:**
- Modify: `src/state/library.rs`
- Modify: existing tests in `src/state/library.rs`

- [ ] **Step 1: Add `category` field to `SoundEntry`**

In `src/state/library.rs`, modify the struct:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub format: AudioFormat,
    pub duration_ms: Option<u64>,
    pub category: String,
}
```

- [ ] **Step 2: Update `entry_from_path` to derive category**

Replace the `entry_from_path` function:

```rust
fn entry_from_path(path: &Path) -> Option<SoundEntry> {
    let ext = path.extension()?.to_str()?;
    if !is_audio_extension(ext) {
        return None;
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let category = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "General".into());

    Some(SoundEntry {
        id: path_to_id(path),
        name,
        path: path.to_path_buf(),
        format: AudioFormat::from_extension(ext),
        duration_ms: None,
        category,
    })
}
```

- [ ] **Step 3: Fix existing tests — add `category` assertions**

Update `entry_has_correct_fields` test:

```rust
#[test]
fn entry_has_correct_fields() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("my_sound.wav");
    fs::write(&file_path, b"wav data").unwrap();

    let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
    assert_eq!(entries.len(), 1);

    let entry = &entries[0];
    assert_eq!(entry.name, "my_sound");
    assert_eq!(entry.path, file_path);
    assert_eq!(entry.format, AudioFormat::Wav);
    assert!(entry.duration_ms.is_none());
    assert!(!entry.id.is_empty());
    assert!(!entry.category.is_empty());
}
```

- [ ] **Step 4: Add test for category derivation from subdirectory**

```rust
#[test]
fn scan_derives_category_from_parent_dir() {
    let dir = tempfile::tempdir().unwrap();
    let memes = dir.path().join("Memes");
    fs::create_dir(&memes).unwrap();
    fs::write(memes.join("honk.mp3"), b"data").unwrap();

    let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].category, "Memes");
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib state`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/state/library.rs
git commit -m "feat(state): add category field to SoundEntry, derived from parent dir"
```

---

## Task 4: Rewrite `src/app.rs` — Full State + New Messages

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Replace app.rs with full Phase 1 state and messages**

```rust
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length, Subscription, Task, Theme};

use crate::audio::{AudioCommand, AudioEvent, AudioHandle};
use crate::state::{AppConfig, SoundEntry};
use crate::tray::{TrayEvent, TrayHandle};
use crate::ui::sound_grid;

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    ToggleVisibility,
    Quit,
    TrayEvent(TrayEvent),
    TrayPoll,
    AudioEvent(AudioEvent),
    PlaySound(String),
    StopAll,
    SelectCategory(Option<String>),
}

impl Message {
    pub fn from_tray_event(event: TrayEvent) -> Self {
        match event {
            TrayEvent::ToggleVisibility => Message::ToggleVisibility,
            TrayEvent::Quit => Message::Quit,
        }
    }
}

pub struct HonkHonk {
    visible: bool,
    exit: bool,
    tray_rx: Arc<Mutex<Receiver<TrayEvent>>>,
    _tray: Option<TrayHandle>,
    audio: Option<AudioHandle>,
    sounds: Vec<SoundEntry>,
    playing: Option<String>,
    active_category: Option<String>,
    config: AppConfig,
}

impl HonkHonk {
    pub fn new(
        mut tray: TrayHandle,
        audio: AudioHandle,
        sounds: Vec<SoundEntry>,
        config: AppConfig,
    ) -> Self {
        let rx = tray.take_rx();
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
            _tray: Some(tray),
            audio: Some(audio),
            sounds,
            playing: None,
            active_category: None,
            config,
        }
    }

    pub fn new_for_test() -> Self {
        let (_tx, rx) = std::sync::mpsc::channel();
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
            _tray: None,
            audio: None,
            sounds: Vec::new(),
            playing: None,
            active_category: None,
            config: AppConfig::default(),
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn playing(&self) -> Option<&str> {
        self.playing.as_deref()
    }

    pub fn active_category(&self) -> Option<&str> {
        self.active_category.as_deref()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleVisibility => {
                self.visible = !self.visible;
                Task::none()
            }
            Message::Quit => {
                if let Some(ref audio) = self.audio {
                    audio.shutdown();
                }
                self.exit = true;
                iced::exit()
            }
            Message::TrayEvent(event) => {
                let msg = Message::from_tray_event(event);
                self.update(msg)
            }
            Message::TrayPoll => {
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }

                let event = self.tray_rx.lock().ok().and_then(|rx| rx.try_recv().ok());
                if let Some(e) = event {
                    let msg = Message::from_tray_event(e);
                    return self.update(msg);
                }

                if let Some(ref audio) = self.audio {
                    if let Some(event) = audio.try_recv() {
                        return self.update(Message::AudioEvent(event));
                    }
                }

                Task::none()
            }
            Message::AudioEvent(event) => {
                match event {
                    AudioEvent::Ready => {
                        eprintln!("honkhonk: audio engine ready");
                    }
                    AudioEvent::PlaybackStarted { sound_id } => {
                        self.playing = Some(sound_id);
                    }
                    AudioEvent::PlaybackFinished { .. } => {
                        self.playing = None;
                    }
                    AudioEvent::Error(e) => {
                        eprintln!("honkhonk: audio error: {e}");
                    }
                }
                Task::none()
            }
            Message::PlaySound(sound_id) => {
                let sound = self.sounds.iter().find(|s| s.id == sound_id);
                let sound = match sound {
                    Some(s) => s,
                    None => return Task::none(),
                };

                let decoded = match crate::audio::decode(&sound.path) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("honkhonk: decode error: {e}");
                        return Task::none();
                    }
                };

                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::Play {
                        sound_id,
                        samples: Arc::new(decoded.samples),
                        sample_rate: decoded.sample_rate,
                        channels: decoded.channels,
                    });
                }

                Task::none()
            }
            Message::StopAll => {
                if let Some(ref audio) = self.audio {
                    audio.send(AudioCommand::Stop);
                }
                self.playing = None;
                Task::none()
            }
            Message::SelectCategory(cat) => {
                self.active_category = cat;
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let header = self.view_header();
        let chips = self.view_category_chips();
        let grid = sound_grid::view_grid(
            &self.sounds,
            self.playing.as_deref(),
            self.active_category.as_deref(),
        );

        let content = column![header, chips, scrollable(grid).height(Length::Fill)]
            .spacing(crate::ui::theme::space::MD);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(crate::ui::theme::space::XL)
            .style(|_theme| container::Style {
                background: Some(crate::ui::theme::bg_color(
                    crate::ui::theme::Hh::bg(crate::ui::theme::Theme::Dark),
                )),
                ..Default::default()
            })
            .into()
    }

    fn view_header(&self) -> Element<'_, Message> {
        let title = text("HonkHonk")
            .size(24)
            .color(crate::ui::theme::Hh::ink(crate::ui::theme::Theme::Dark));

        let stop_btn = button(
            text("Stop All")
                .size(14)
                .color(crate::ui::theme::Hh::ink(crate::ui::theme::Theme::Dark)),
        )
        .on_press(Message::StopAll)
        .style(|_theme, _status| button::Style {
            background: Some(crate::ui::theme::bg_color(
                crate::ui::theme::Hh::panel(crate::ui::theme::Theme::Dark),
            )),
            text_color: crate::ui::theme::Hh::ink(crate::ui::theme::Theme::Dark),
            border: crate::ui::theme::tile_border(
                crate::ui::theme::Hh::hairline(crate::ui::theme::Theme::Dark),
                1.0,
            ),
            ..Default::default()
        });

        row![title, iced::widget::horizontal_space(), stop_btn]
            .align_y(iced::Alignment::Center)
            .into()
    }

    fn view_category_chips(&self) -> Element<'_, Message> {
        use std::collections::BTreeSet;
        let theme = crate::ui::theme::Theme::Dark;

        let categories: BTreeSet<&str> = self.sounds.iter().map(|s| s.category.as_str()).collect();

        let all_chip = self.category_chip("All", self.active_category.is_none(), None);

        let chips: Vec<Element<'_, Message>> = std::iter::once(all_chip)
            .chain(categories.into_iter().map(|cat| {
                let is_active = self.active_category.as_deref() == Some(cat);
                self.category_chip(cat, is_active, Some(cat.to_owned()))
            }))
            .collect();

        let chip_row = chips
            .into_iter()
            .fold(row![].spacing(crate::ui::theme::space::SM), |r, chip| {
                r.push(chip)
            });

        scrollable(chip_row)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new(),
            ))
            .into()
    }

    fn category_chip(
        &self,
        label: &str,
        active: bool,
        value: Option<String>,
    ) -> Element<'_, Message> {
        let theme = crate::ui::theme::Theme::Dark;
        let bg = if active {
            crate::ui::theme::Hh::accent(theme)
        } else {
            crate::ui::theme::Hh::panel(theme)
        };
        let text_color = if active {
            iced::Color::from_rgb(0.1, 0.07, 0.03)
        } else {
            crate::ui::theme::Hh::ink(theme)
        };

        button(text(label).size(13).color(text_color))
            .on_press(Message::SelectCategory(value))
            .padding([crate::ui::theme::space::XS, crate::ui::theme::space::MD])
            .style(move |_theme, _status| button::Style {
                background: Some(crate::ui::theme::bg_color(bg)),
                text_color,
                border: iced::Border {
                    color: iced::Color::TRANSPARENT,
                    width: 0.0,
                    radius: crate::ui::theme::radius::PILL,
                },
                ..Default::default()
            })
            .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll)
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: May fail until main.rs updated (new `HonkHonk::new` signature). Proceed to Task 5.

---

## Task 5: Implement `src/ui/sound_grid.rs` — Grid Layout + Tile Styling

**Files:**
- Modify: `src/ui/sound_grid.rs`

- [ ] **Step 1: Replace stub with full grid implementation**

```rust
use iced::widget::{button, column, container, row, text};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::state::SoundEntry;
use crate::ui::theme::{self, Hh, Theme, Tone};

const COLUMNS: usize = 5;
const TILE_HEIGHT: f32 = 140.0;

pub fn view_grid<'a>(
    sounds: &'a [SoundEntry],
    playing: Option<&str>,
    category: Option<&str>,
) -> Element<'a, Message> {
    let theme = Theme::Dark;

    let filtered: Vec<&SoundEntry> = sounds
        .iter()
        .filter(|s| match category {
            Some(cat) => s.category == cat,
            None => true,
        })
        .collect();

    if filtered.is_empty() {
        return container(
            text("No sounds found. Add audio files to your sound directory.")
                .size(16)
                .color(theme.ink_dim()),
        )
        .width(Length::Fill)
        .padding(theme::space::XXL)
        .into();
    }

    let rows: Vec<Element<'a, Message>> = filtered
        .chunks(COLUMNS)
        .map(|chunk| {
            let tiles: Vec<Element<'a, Message>> = chunk
                .iter()
                .enumerate()
                .map(|(i, sound)| {
                    let is_playing = playing == Some(sound.id.as_str());
                    let tone_idx = u64::from_str_radix(&sound.id[..8], 16)
                        .unwrap_or(0) as usize;
                    tile_view(sound, is_playing, Tone::from_index(tone_idx), theme)
                })
                .collect();

            let r = tiles
                .into_iter()
                .fold(row![].spacing(theme::space::LG), |r, t| r.push(t));

            r.into()
        })
        .collect();

    let grid = rows
        .into_iter()
        .fold(column![].spacing(theme::space::LG), |c, r| c.push(r));

    grid.width(Length::Fill).into()
}

fn tile_view<'a>(
    sound: &'a SoundEntry,
    is_playing: bool,
    tone: Tone,
    theme: Theme,
) -> Element<'a, Message> {
    let duration_str = match sound.duration_ms {
        Some(ms) => {
            let secs = ms / 1000;
            format!("{}:{:02}", secs / 60, secs % 60)
        }
        None => "—".into(),
    };

    let category_text = text(&sound.category)
        .size(11)
        .color(theme.ink_dim());

    let name_text = text(&sound.name)
        .size(15)
        .color(theme.ink());

    let duration_text = text(duration_str)
        .size(11)
        .color(theme.ink_faint());

    let content = column![category_text, name_text, duration_text]
        .spacing(theme::space::SM)
        .padding(theme::space::LG);

    let bg = tone.tile_tint(theme.is_dark());
    let border_color = if is_playing {
        theme.accent()
    } else {
        theme.hairline()
    };
    let border_width = if is_playing { 2.5 } else { 1.0 };

    button(content)
        .on_press(Message::PlaySound(sound.id.clone()))
        .width(Length::Fill)
        .height(TILE_HEIGHT)
        .style(move |_theme, status| {
            let bg_final = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    lighten(bg, 0.03)
                }
                _ => bg,
            };
            button::Style {
                background: Some(theme::bg_color(bg_final)),
                text_color: theme.ink(),
                border: Border {
                    color: border_color,
                    width: border_width,
                    radius: theme::radius::TILE,
                },
                ..Default::default()
            }
        })
        .into()
}

fn lighten(c: iced::Color, amount: f32) -> iced::Color {
    iced::Color {
        r: (c.r + amount).min(1.0),
        g: (c.g + amount).min(1.0),
        b: (c.b + amount).min(1.0),
        a: c.a,
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: Compiles (after Task 4 app.rs is in place)

---

## Task 6: Update `src/main.rs` — Load Config + Scan Library

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace main.rs with config/library loading**

```rust
fn main() -> iced::Result {
    pipewire::init();

    if let Err(e) = gtk::init() {
        eprintln!("fatal: failed to initialize GTK (required for system tray): {e}");
        std::process::exit(1);
    }

    let config = match honkhonk::state::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: failed to load config, using defaults: {e}");
            honkhonk::state::AppConfig::default()
        }
    };

    let sounds = match honkhonk::state::Library::scan(&config.sound_directories) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: failed to scan sound library: {e}");
            Vec::new()
        }
    };

    let tray_handle = match honkhonk::tray::build_tray() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("fatal: failed to initialize system tray: {e}");
            std::process::exit(1);
        }
    };

    let audio_handle = match honkhonk::audio::spawn() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("fatal: failed to start audio engine: {e}");
            std::process::exit(1);
        }
    };

    let tray_handle = std::sync::Mutex::new(Some(tray_handle));
    let audio_handle = std::sync::Mutex::new(Some(audio_handle));

    iced::application(
        move || {
            let tray = tray_handle
                .lock()
                .expect("tray mutex poisoned")
                .take()
                .expect("boot called more than once");
            let audio = audio_handle
                .lock()
                .expect("audio mutex poisoned")
                .take()
                .expect("boot called more than once");
            honkhonk::app::HonkHonk::new(tray, audio, sounds, config)
        },
        honkhonk::app::HonkHonk::update,
        honkhonk::app::HonkHonk::view,
    )
    .title("HonkHonk")
    .subscription(honkhonk::app::HonkHonk::subscription)
    .theme(honkhonk::app::HonkHonk::theme)
    .run()
}
```

- [ ] **Step 2: Verify full compilation**

Run: `cargo build`
Expected: Compiles and links successfully

- [ ] **Step 3: Commit**

```bash
git add src/main.rs src/app.rs src/ui/sound_grid.rs
git commit -m "feat(ui): sound grid with click-to-play and category chips"
```

---

## Task 7: Update Tests

**Files:**
- Modify: `tests/app_test.rs`

- [ ] **Step 1: Update existing tests for new `new_for_test` signature**

The existing tests use `HonkHonk::new_for_test()` which still works (no signature change). Add new tests:

```rust
use honkhonk::app::{HonkHonk, Message};

#[test]
fn quit_message_sets_should_exit() {
    let mut app = HonkHonk::new_for_test();
    let _task = app.update(Message::Quit);
    assert!(app.should_exit());
}

#[test]
fn toggle_visibility_flips_visible_state() {
    let mut app = HonkHonk::new_for_test();
    assert!(app.is_visible());
    let _task = app.update(Message::ToggleVisibility);
    assert!(!app.is_visible());
    let _task = app.update(Message::ToggleVisibility);
    assert!(app.is_visible());
}

#[test]
fn tray_event_quit_maps_to_quit_message() {
    let msg = Message::from_tray_event(honkhonk::tray::TrayEvent::Quit);
    assert_eq!(msg, Message::Quit);
}

#[test]
fn tray_event_toggle_maps_to_toggle_message() {
    let msg = Message::from_tray_event(honkhonk::tray::TrayEvent::ToggleVisibility);
    assert_eq!(msg, Message::ToggleVisibility);
}

#[test]
fn toggle_visibility_does_not_exit() {
    let mut app = HonkHonk::new_for_test();
    let _task = app.update(Message::ToggleVisibility);
    assert!(!app.should_exit());
    assert!(!app.is_visible());
}

#[test]
fn audio_playback_started_event_sets_playing() {
    let mut app = HonkHonk::new_for_test();
    let event = honkhonk::audio::AudioEvent::PlaybackStarted {
        sound_id: "test-id".into(),
    };
    let _task = app.update(Message::AudioEvent(event));
    assert_eq!(app.playing(), Some("test-id"));
}

#[test]
fn audio_playback_finished_event_clears_playing() {
    let mut app = HonkHonk::new_for_test();
    let started = honkhonk::audio::AudioEvent::PlaybackStarted {
        sound_id: "test-id".into(),
    };
    let _task = app.update(Message::AudioEvent(started));
    assert_eq!(app.playing(), Some("test-id"));

    let finished = honkhonk::audio::AudioEvent::PlaybackFinished {
        sound_id: "test-id".into(),
    };
    let _task = app.update(Message::AudioEvent(finished));
    assert_eq!(app.playing(), None);
}

#[test]
fn stop_all_clears_playing() {
    let mut app = HonkHonk::new_for_test();
    let started = honkhonk::audio::AudioEvent::PlaybackStarted {
        sound_id: "x".into(),
    };
    let _task = app.update(Message::AudioEvent(started));
    assert!(app.playing().is_some());

    let _task = app.update(Message::StopAll);
    assert_eq!(app.playing(), None);
}

#[test]
fn select_category_updates_state() {
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.active_category(), None);

    let _task = app.update(Message::SelectCategory(Some("Memes".into())));
    assert_eq!(app.active_category(), Some("Memes"));

    let _task = app.update(Message::SelectCategory(None));
    assert_eq!(app.active_category(), None);
}

#[test]
fn play_sound_with_no_matching_id_does_not_crash() {
    let mut app = HonkHonk::new_for_test();
    let _task = app.update(Message::PlaySound("nonexistent".into()));
    assert_eq!(app.playing(), None);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add tests/app_test.rs
git commit -m "test(app): add tests for PlaySound, StopAll, SelectCategory messages"
```

---

## Task 8: Final Verification

- [ ] **Step 1: Run full lint + test suite**

Run: `cargo clippy -- -D warnings && cargo test`
Expected: Zero warnings, all tests pass

- [ ] **Step 2: Run the application**

Run: `cargo run`
Expected: Window shows with dark Confetti background. If `~/Music/HonkHonk/` has audio files, grid renders tiles. Clicking a tile plays audio (audible in headset, visible in `wpctl status` stream). "Stop All" halts playback. Category chips filter the grid.

- [ ] **Step 3: Verify with `wpctl status`**

Run: `wpctl status` (while a sound is playing)
Expected: See "honkhonk-to-sink" and "honkhonk-monitor" streams under Streams section

- [ ] **Step 4: Final commit if any fixups needed**

```bash
git add -A
git commit -m "fix(ui): address lint/compilation issues from grid integration"
```

(Skip this commit if no fixups needed.)

---

## Out of Scope

- Search bar (Issue #8)
- Volume slider (Issue #8)
- Now-playing progress bar (Issue #8)
- Canvas sticker tiles (Phase 3)
- Tile rotation (Phase 3)
- Responsive column count by window width (can hardcode 5 for now)
- Dark/Light theme toggle (Phase 3)
- List view mode (Phase 3)
