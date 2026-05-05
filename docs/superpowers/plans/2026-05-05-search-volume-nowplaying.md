# Search Bar + Volume Controls + Now-Playing Bar — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add search filtering, volume slider, and now-playing bar with progress feedback to the HonkHonk soundboard UI (Issue #8).

**Architecture:** Three new UI component files (`search_bar.rs`, `volume.rs`, `now_playing.rs`) following the existing pattern in `sound_grid.rs` — each exports a public `view_*` function returning `Element<Message>`. Audio progress is tracked via `PlaybackState::progress()` in `playback.rs` and emitted throttled from the PipeWire completion timer in `engine.rs`. App state in `app.rs` gains `search_query` and `progress` fields.

**Tech Stack:** Rust, Iced 0.13 (`text_input`, `slider`, `container`, `row`), PipeWire (existing engine)

---

### Task 1: Add `progress()` to PlaybackState

**Files:**
- Modify: `src/audio/playback.rs:201-303`

- [ ] **Step 1: Write failing test for `progress()`**

Add to the existing `#[cfg(test)]` block — there isn't one in `playback.rs` yet, so create it at the bottom of the file (after line 303):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_state_at(cursor: usize, total: usize) -> PlaybackState {
        let samples = Arc::new(vec![0.0_f32; total]);
        let mut state = PlaybackState::new();
        state.start("test".into(), samples, 48000, 2);
        state.cursor = cursor;
        state
    }

    #[test]
    fn progress_at_start_is_zero() {
        let state = make_state_at(0, 9600);
        assert!((state.progress() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_at_midpoint() {
        let state = make_state_at(4800, 9600);
        assert!((state.progress() - 0.5).abs() < 0.001);
    }

    #[test]
    fn progress_at_end_is_one() {
        let state = make_state_at(9600, 9600);
        assert!((state.progress() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_with_no_samples_is_zero() {
        let state = PlaybackState::new();
        assert!((state.progress() - 0.0).abs() < f32::EPSILON);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib audio::playback::tests -- --nocapture`
Expected: compile error — `progress()` method does not exist.

- [ ] **Step 3: Implement `progress()` method**

Add this method to `impl PlaybackState` in `src/audio/playback.rs`, after the `set_volume` method (after line 268):

```rust
pub fn progress(&self) -> f32 {
    match &self.samples {
        Some(s) if !s.is_empty() => self.cursor as f32 / s.len() as f32,
        _ => 0.0,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib audio::playback::tests -- --nocapture`
Expected: all 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/audio/playback.rs
git commit -m "feat(audio): add progress() method to PlaybackState"
```

---

### Task 2: Emit `AudioEvent::Progress` from engine

**Files:**
- Modify: `src/audio/engine.rs:29-34` (AudioEvent enum)
- Modify: `src/audio/engine.rs:118-157` (setup_completion_timer)

- [ ] **Step 1: Add `Progress(f32)` variant to `AudioEvent`**

In `src/audio/engine.rs`, change the `AudioEvent` enum (line 29) to:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Ready,
    PlaybackStarted { sound_id: String },
    PlaybackFinished { sound_id: String },
    Progress(f32),
    Error(String),
}
```

- [ ] **Step 2: Emit progress from completion timer callback**

The existing `setup_completion_timer` fires every 100ms and already borrows `active_timer` to check playback state. Add progress emission right before the `done` check. Replace the `setup_completion_timer` function (lines 118-157) with:

```rust
fn setup_completion_timer(
    pw_loop: &pipewire::loop_::Loop,
    active_timer: Rc<RefCell<Option<ActivePlayback>>>,
    evt_tx_timer: mpsc::Sender<AudioEvent>,
) -> Result<pipewire::loop_::TimerSource<'_>, AudioError> {
    let timer = pw_loop.add_timer(move |_expirations| {
        let (done, progress) = {
            let borrow = active_timer.borrow();
            if let Some(ref ap) = *borrow {
                let sink_done = !ap.sink_state.borrow().is_active();
                let mon_done = !ap.monitor_state.borrow().is_active();
                let p = ap.sink_state.borrow().progress();
                (sink_done && mon_done, Some(p))
            } else {
                (false, None)
            }
        };

        if let Some(p) = progress {
            let _ = evt_tx_timer.send(AudioEvent::Progress(p));
        }

        if done {
            if let Some(ap) = active_timer.borrow_mut().take() {
                let _ = evt_tx_timer.send(AudioEvent::PlaybackFinished {
                    sound_id: ap.sound_id,
                });
            }
        }
    });

    if let Err(e) = timer
        .update_timer(
            Some(std::time::Duration::from_millis(100)),
            Some(std::time::Duration::from_millis(100)),
        )
        .into_result()
    {
        return Err(AudioError::PipeWireInit(format!(
            "arm completion timer: {e}"
        )));
    }

    Ok(timer)
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: compiles. There will be a warning about the new `Progress` variant not being matched in `app.rs` — that's expected and fixed in Task 5.

- [ ] **Step 4: Commit**

```bash
git add src/audio/engine.rs
git commit -m "feat(audio): emit AudioEvent::Progress from completion timer"
```

---

### Task 3: Create search bar component

**Files:**
- Create: `src/ui/search_bar.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Create `src/ui/search_bar.rs`**

```rust
use iced::widget::{container, text_input};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

pub fn view_search_bar(query: &str) -> Element<'_, Message> {
    let t = Theme::Dark;

    let input = text_input("Find a sound\u{2026}", query)
        .on_input(Message::SearchChanged)
        .size(13.5)
        .width(Length::Fixed(300.0))
        .style(move |_theme, _status| text_input::Style {
            background: theme::bg_color(t.panel()).into(),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: theme::radius::PILL,
            },
            icon: t.ink_dim(),
            placeholder: t.ink_faint(),
            value: t.ink(),
            selection: t.accent(),
        });

    container(input).into()
}
```

- [ ] **Step 2: Register module in `src/ui/mod.rs`**

Replace contents of `src/ui/mod.rs` with:

```rust
pub mod search_bar;
pub mod sound_grid;
pub mod theme;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: compiles (may warn about `SearchChanged` not existing yet — fixed in Task 5).

- [ ] **Step 4: Commit**

```bash
git add src/ui/search_bar.rs src/ui/mod.rs
git commit -m "feat(ui): add pill-shaped search bar component"
```

---

### Task 4: Create volume slider and now-playing bar components

**Files:**
- Create: `src/ui/volume.rs`
- Create: `src/ui/now_playing.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Create `src/ui/volume.rs`**

```rust
use iced::widget::{container, row, slider, text};
use iced::Element;

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

pub fn view_volume(volume: f32) -> Element<'static, Message> {
    let t = Theme::Dark;
    let pct = format!("{}%", (volume * 100.0).round() as u32);

    let vol_slider = slider(0.0..=1.0, volume, Message::VolumeChanged)
        .step(0.01)
        .width(140.0);

    let label = text(pct)
        .size(12)
        .color(t.ink_dim());

    row![vol_slider, label]
        .spacing(theme::space::SM)
        .align_y(iced::Alignment::Center)
        .into()
}
```

- [ ] **Step 2: Create `src/ui/now_playing.rs`**

```rust
use iced::widget::{container, row, space, text, Column};
use iced::{Border, Element, Length};

use crate::app::Message;
use crate::state::SoundEntry;
use crate::ui::theme::{self, Hh, Theme};
use crate::ui::volume;

pub fn view_now_playing<'a>(
    playing: Option<&'a str>,
    sounds: &'a [SoundEntry],
    progress: f32,
    vol: f32,
) -> Element<'a, Message> {
    let t = Theme::Dark;

    let sound = match playing {
        Some(id) => sounds.iter().find(|s| s.id == id),
        None => None,
    };

    let sound = match sound {
        Some(s) => s,
        None => return container(space::vertical(0.0)).into(),
    };

    let placeholder = container(space::horizontal(0.0))
        .width(44.0)
        .height(44.0)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.bg())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: theme::radius::MD,
            },
            ..Default::default()
        });

    let name = text(sound.name.clone()).size(14).color(t.ink());
    let subtitle = text(format!("HONKING NOW \u{00b7} {}", sound.category))
        .size(10.5)
        .color(t.ink_dim());
    let info = Column::new()
        .push(name)
        .push(subtitle)
        .spacing(theme::space::XS);

    let progress_bar = view_progress_bar(progress, t);
    let vol_widget = volume::view_volume(vol);

    let content = row![
        placeholder,
        info,
        progress_bar,
        space::horizontal(Length::Fill),
        vol_widget,
    ]
    .spacing(theme::space::LG)
    .align_y(iced::Alignment::Center);

    container(content)
        .width(Length::Fill)
        .padding([theme::space::MD, theme::space::XL])
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: iced::border::Radius::default(),
            },
            ..Default::default()
        })
        .into()
}

fn view_progress_bar(progress: f32, t: Theme) -> Element<'static, Message> {
    let filled_width = (progress.clamp(0.0, 1.0) * 320.0).round();

    let filled = container(space::horizontal(0.0))
        .width(filled_width)
        .height(6.0)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.accent())),
            border: Border {
                radius: theme::radius::SM,
                ..Default::default()
            },
            ..Default::default()
        });

    let track = container(filled)
        .width(320.0)
        .height(6.0)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.bg())),
            border: Border {
                radius: theme::radius::SM,
                ..Default::default()
            },
            ..Default::default()
        });

    track.into()
}
```

- [ ] **Step 3: Register both modules in `src/ui/mod.rs`**

Replace contents of `src/ui/mod.rs` with:

```rust
pub mod now_playing;
pub mod search_bar;
pub mod sound_grid;
pub mod theme;
pub mod volume;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: compiles (may warn about `VolumeChanged`/`SearchChanged` not matched — fixed next task).

- [ ] **Step 5: Commit**

```bash
git add src/ui/volume.rs src/ui/now_playing.rs src/ui/mod.rs
git commit -m "feat(ui): add volume slider and now-playing bar components"
```

---

### Task 5: Wire everything into app.rs

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing tests for new message handlers**

Add these tests to the existing `mod tests` block in `src/app.rs` (after line 373):

```rust
#[test]
fn search_changed_updates_query() {
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.search_query(), "");
    let _ = app.update(Message::SearchChanged("honk".into()));
    assert_eq!(app.search_query(), "honk");
}

#[test]
fn volume_changed_updates_config() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::VolumeChanged(0.42));
    assert!((app.config.volume - 0.42).abs() < f32::EPSILON);
}

#[test]
fn progress_event_updates_progress() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.65)));
    assert!((app.progress() - 0.65).abs() < f32::EPSILON);
}

#[test]
fn playback_finished_resets_progress() {
    let mut app = HonkHonk::new_for_test();
    app.progress = 0.8;
    app.playing = Some("test".into());
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        sound_id: "test".into(),
    }));
    assert!((app.progress() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn search_filters_sounds_in_view_data() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        SoundEntry {
            id: "aaa".into(),
            name: "Goose Honk".into(),
            path: "/a.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            category: "Honk".into(),
        },
        SoundEntry {
            id: "bbb".into(),
            name: "Vine Boom".into(),
            path: "/b.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            category: "Memes".into(),
        },
    ];
    let _ = app.update(Message::SearchChanged("goose".into()));
    assert_eq!(app.filtered_sounds().len(), 1);
    assert_eq!(app.filtered_sounds()[0].id, "aaa");
}

#[test]
fn search_is_case_insensitive() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![SoundEntry {
        id: "aaa".into(),
        name: "Goose Honk".into(),
        path: "/a.mp3".into(),
        format: crate::state::AudioFormat::Mp3,
        duration_ms: Some(1000),
        category: "Honk".into(),
    }];
    let _ = app.update(Message::SearchChanged("GOOSE".into()));
    assert_eq!(app.filtered_sounds().len(), 1);
}

#[test]
fn search_and_category_filter_stack() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        SoundEntry {
            id: "aaa".into(),
            name: "Goose Honk".into(),
            path: "/a.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            category: "Honk".into(),
        },
        SoundEntry {
            id: "bbb".into(),
            name: "Goose Boom".into(),
            path: "/b.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            category: "Memes".into(),
        },
    ];
    let _ = app.update(Message::SelectCategory(Some("Honk".into())));
    let _ = app.update(Message::SearchChanged("goose".into()));
    assert_eq!(app.filtered_sounds().len(), 1);
    assert_eq!(app.filtered_sounds()[0].id, "aaa");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::tests -- --nocapture`
Expected: compile errors — `SearchChanged`, `VolumeChanged`, `search_query()`, `progress()`, `filtered_sounds()` don't exist yet.

- [ ] **Step 3: Add new Message variants**

In `src/app.rs`, add to the `Message` enum (after line 22, before the closing `}`):

```rust
SearchChanged(String),
VolumeChanged(f32),
```

- [ ] **Step 4: Add new state fields and accessors**

Add `search_query` and `progress` fields to the `HonkHonk` struct (after `config` on line 43):

```rust
search_query: String,
progress: f32,
```

Initialize them in `new()` (after `config,` on line 63):

```rust
search_query: String::new(),
progress: 0.0,
```

Initialize them in `new_for_test()` (after `config: AppConfig::default(),` on line 78):

```rust
search_query: String::new(),
progress: 0.0,
```

Add accessor methods after `active_category()` (after line 96):

```rust
pub fn search_query(&self) -> &str {
    &self.search_query
}

pub fn progress(&self) -> f32 {
    self.progress
}

pub fn filtered_sounds(&self) -> Vec<&SoundEntry> {
    let query = self.search_query.to_lowercase();
    self.sounds
        .iter()
        .filter(|s| match &self.active_category {
            Some(cat) => s.category == *cat,
            None => true,
        })
        .filter(|s| {
            query.is_empty() || s.name.to_lowercase().contains(&query)
        })
        .collect()
}
```

- [ ] **Step 5: Add update handlers for new messages**

In the `update` method's `match` block, add before the closing `}` (after `SelectCategory` arm, line 190):

```rust
Message::SearchChanged(query) => {
    self.search_query = query;
    Task::none()
}
Message::VolumeChanged(v) => {
    self.config.volume = v.clamp(0.0, 1.0);
    if let Some(ref audio) = self.audio {
        audio.send(AudioCommand::SetVolume(self.config.volume));
    }
    if let Err(e) = self.config.save() {
        eprintln!("honkhonk: config save error: {e}");
    }
    Task::none()
}
```

Add `Progress` handling in the `AudioEvent` match arm. After the `PlaybackStarted` arm (line 143):

```rust
AudioEvent::Progress(p) => {
    self.progress = p;
}
```

In the `PlaybackFinished` arm (line 145), also reset progress:

```rust
AudioEvent::PlaybackFinished { .. } => {
    self.playing = None;
    self.progress = 0.0;
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib app::tests -- --nocapture`
Expected: all tests PASS (both old and new).

- [ ] **Step 7: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): wire search, volume, and progress message handlers"
```

---

### Task 6: Update view to compose all new components

**Files:**
- Modify: `src/app.rs` (view methods)
- Modify: `src/ui/sound_grid.rs:11-15` (add search_query param)

- [ ] **Step 1: Update `sound_grid::view_grid` to accept search query**

In `src/ui/sound_grid.rs`, change the `view_grid` function signature (line 11) to:

```rust
pub fn view_grid<'a>(
    sounds: &'a [SoundEntry],
    playing: Option<&str>,
    category: Option<&str>,
    search_query: &str,
) -> Element<'a, Message> {
```

Add search filtering after the category filter (after line 23, replacing the closing of the `filtered` let):

```rust
let filtered: Vec<&SoundEntry> = sounds
    .iter()
    .filter(|s| match category {
        Some(cat) => s.category == cat,
        None => true,
    })
    .filter(|s| {
        search_query.is_empty()
            || s.name.to_lowercase().contains(&search_query.to_lowercase())
    })
    .collect();
```

- [ ] **Step 2: Update `app.rs` imports**

Replace the import block at the top of `src/app.rs` (lines 1-11) with:

```rust
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Element, Length, Subscription, Task, Theme};

use crate::audio::{AudioCommand, AudioEvent, AudioHandle};
use crate::state::{AppConfig, SoundEntry};
use crate::tray::{TrayEvent, TrayHandle};
use crate::ui::sound_grid;
use crate::ui::theme::{self, Hh};
use crate::ui::{now_playing, search_bar};
```

- [ ] **Step 3: Update `view()` to add search bar to header and now-playing at bottom**

Replace the `view` method in `src/app.rs` with:

```rust
pub fn view(&self) -> Element<'_, Message> {
    let t = theme::Theme::Dark;
    let header = self.view_header(t);
    let chips = self.view_category_chips(t);
    let grid = sound_grid::view_grid(
        &self.sounds,
        self.playing.as_deref(),
        self.active_category.as_deref(),
        &self.search_query,
    );

    let now_playing = now_playing::view_now_playing(
        self.playing.as_deref(),
        &self.sounds,
        self.progress,
        self.config.volume,
    );

    let content = column![
        header,
        chips,
        scrollable(grid).height(Length::Fill),
        now_playing,
    ]
    .spacing(theme::space::MD);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::space::XL)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.bg())),
            ..Default::default()
        })
        .into()
}
```

- [ ] **Step 4: Add search bar to header**

Replace the `view_header` method in `src/app.rs` with:

```rust
fn view_header(&self, t: theme::Theme) -> Element<'_, Message> {
    let title = text("HonkHonk").size(24).color(t.ink());

    let search = search_bar::view_search_bar(&self.search_query);

    let stop_btn = button(text("Stop All").size(14).color(t.ink()))
        .on_press(Message::StopAll)
        .style(move |_theme, _status| button::Style {
            background: Some(theme::bg_color(t.panel())),
            text_color: t.ink(),
            border: theme::tile_border(t.hairline(), 1.0),
            ..Default::default()
        });

    row![title, space::horizontal(Length::Fill), search, stop_btn]
        .spacing(theme::space::LG)
        .align_y(iced::Alignment::Center)
        .into()
}
```

- [ ] **Step 5: Verify compilation and tests**

Run: `cargo build && cargo test`
Expected: compiles and all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/ui/sound_grid.rs
git commit -m "feat(ui): compose search bar, volume, and now-playing into main view"
```

---

### Task 7: Manual verification

- [ ] **Step 1: Run the app**

Run: `cargo run`

Verify:
1. Search bar appears in header between "HonkHonk" title and "Stop All" button
2. Typing in search bar instantly filters the sound grid (case-insensitive)
3. Category chips + search filter stack (selecting a category AND typing narrows results)
4. Clearing search shows all sounds in active category again
5. Now-playing bar appears at bottom when a sound plays
6. Now-playing bar shows sound name and "HONKING NOW · {category}" subtitle
7. Progress bar fills left-to-right during playback
8. Volume slider adjusts playback volume
9. Volume percentage label updates as slider moves
10. Now-playing bar disappears when playback finishes
11. Pressing "Stop All" clears the now-playing bar

- [ ] **Step 2: Run full test suite and clippy**

Run: `cargo clippy -- -D warnings && cargo test`
Expected: zero warnings, all tests pass.

- [ ] **Step 3: Final commit if any clippy fixes needed**

```bash
git add -u
git commit -m "fix: address clippy warnings"
```
