# Slot Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a 4×5 slot manager full-window view — shows all 20 hotkey slots, lets users inspect and unbind slots, navigated via a "Slots" button in the app header.

**Architecture:** Add `ViewMode` enum to `HonkHonk` state. `view()` branches between the renamed `view_main()` and a new `slot_manager::view_slot_manager()`. "Slots" button anchors left in the header next to the title, separated from global actions. Slot manager is view + unbind only — assignment deferred to the right-click context menu PR.

**Tech Stack:** Rust, Iced 0.13, custom `theme::Theme`, `SlotMap`, `SoundEntry`

---

### Task 1: Extend theme — hairline2, good, Tone::highlight

**Files:**
- Modify: `src/ui/theme.rs`

- [ ] **Step 1: Add two methods to the `Hh` trait**

In `src/ui/theme.rs`, update the `pub trait Hh` block:

```rust
pub trait Hh {
    fn bg(self) -> Color;
    fn panel(self) -> Color;
    fn ink(self) -> Color;
    fn ink_dim(self) -> Color;
    fn ink_faint(self) -> Color;
    fn hairline(self) -> Color;
    fn hairline2(self) -> Color;  // stronger divider / dashed border
    fn good(self) -> Color;       // green status indicator
    fn accent(self) -> Color;
}
```

- [ ] **Step 2: Implement on Theme — add after the `hairline` arm**

```rust
fn hairline2(self) -> Color {
    match self {
        Theme::Light => Color::from_rgba(0.0, 0.0, 0.0, 0.12),
        Theme::Dark => Color::from_rgba(1.0, 1.0, 1.0, 0.12),
    }
}
fn good(self) -> Color {
    match self {
        Theme::Light => hex(0x16a34a),
        Theme::Dark => hex(0x4ade80),
    }
}
```

- [ ] **Step 3: Add `Tone::highlight` — saturated tone color for sticker placeholder circles**

In `impl Tone`, add after `tile_tint`:

```rust
pub fn highlight(self, dark: bool) -> Color {
    let (h, s, l) = self.hsl();
    if dark {
        hsl_to_color(h, s / 100.0, (l - 5.0) / 100.0)
    } else {
        hsl_to_color(h, s / 100.0, l / 100.0)
    }
}
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check 2>&1
```

Expected: clean. If trait not fully implemented Rust reports "not all trait items implemented".

- [ ] **Step 5: Commit**

```bash
git add src/ui/theme.rs
git commit -m "feat(theme): add hairline2, good colors and Tone::highlight"
```

---

### Task 2: ViewMode + state + messages + update arms + tests

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing tests first**

In `src/app.rs`, inside `#[cfg(test)] mod tests { ... }`, add after the existing `close_context_menu_clears_it` test:

```rust
#[test]
fn show_slots_sets_view_mode() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShowSlots);
    assert_eq!(app.view_mode(), ViewMode::SlotManager);
    assert!(app.selected_slot().is_none());
}

#[test]
fn show_main_resets_view_mode() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShowSlots);
    let _ = app.update(Message::ShowMain);
    assert_eq!(app.view_mode(), ViewMode::Main);
}

#[test]
fn select_slot_sets_selected() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SelectSlot(3));
    assert_eq!(app.selected_slot(), Some(3));
}

#[test]
fn clear_slot_keeps_selection_showing_empty_panel() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/tmp/test.mp3");
    let _ = app.update(Message::AssignSlot(3, path.clone()));
    let _ = app.update(Message::SelectSlot(3));
    let _ = app.update(Message::ClearSlot(3));
    assert_eq!(app.selected_slot(), Some(3));
    assert!(app.slots().get(3).is_none());
}
```

- [ ] **Step 2: Run — confirm tests FAIL to compile**

```bash
cargo test 2>&1 | head -20
```

Expected: compile errors for `ViewMode`, `ShowSlots`, `ShowMain`, `SelectSlot`, `view_mode()`, `selected_slot()`.

- [ ] **Step 3: Add ViewMode enum**

Add before the `pub enum Message` block in `src/app.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ViewMode {
    #[default]
    Main,
    SlotManager,
}
```

- [ ] **Step 4: Add new Message variants**

In `pub enum Message`, add after `CloseContextMenu`:

```rust
// Navigation
ShowSlots,
ShowMain,
SelectSlot(u8),
```

- [ ] **Step 5: Add fields to HonkHonk struct**

In `pub struct HonkHonk`, add after the `context_menu` field:

```rust
view_mode: ViewMode,
selected_slot: Option<u8>,
```

- [ ] **Step 6: Initialize in new() and new_for_test()**

In both `HonkHonk::new(...)` and `HonkHonk::new_for_test()`, add to the struct literal:

```rust
view_mode: ViewMode::default(),
selected_slot: None,
```

- [ ] **Step 7: Add update arms**

In the `update` match, add after the `CloseContextMenu` arm:

```rust
Message::ShowSlots => {
    self.view_mode = ViewMode::SlotManager;
    self.selected_slot = None;
    Task::none()
}
Message::ShowMain => {
    self.view_mode = ViewMode::Main;
    Task::none()
}
Message::SelectSlot(idx) => {
    self.selected_slot = Some(idx);
    Task::none()
}
```

- [ ] **Step 8: Add accessor methods**

In `impl HonkHonk`, add after `context_menu()`:

```rust
pub fn view_mode(&self) -> ViewMode {
    self.view_mode
}
pub fn selected_slot(&self) -> Option<u8> {
    self.selected_slot
}
```

- [ ] **Step 9: Run tests — confirm they PASS**

```bash
cargo test 2>&1 | tail -15
```

Expected: all tests pass including the four new ones.

- [ ] **Step 10: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): ViewMode + slot manager navigation state and messages"
```

---

### Task 3: Extract view_main(), branch view(), add Slots header button

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Rename view() to view_main()**

Change `pub fn view(&self)` to `fn view_main(&self)`. Body unchanged.

- [ ] **Step 2: Add new view() that branches on ViewMode**

Add immediately after `view_main`:

```rust
pub fn view(&self) -> Element<'_, Message> {
    let t = theme::Theme::Dark;
    match self.view_mode {
        ViewMode::Main => self.view_main(),
        ViewMode::SlotManager => {
            slot_manager::view_slot_manager(
                &self.slots,
                &self.sounds,
                self.selected_slot,
                t,
            )
        }
    }
}
```

`slot_manager` is not imported yet — add a temporary stub so it compiles. Add this `use` at the top of the `impl HonkHonk` block:

> **Note:** The `slot_manager` module will be created in Task 4. Until then, add this temporary arm to avoid compile errors:

```rust
ViewMode::SlotManager => {
    // temporary — replaced when slot_manager module is added in Task 4
    self.view_main()
}
```

- [ ] **Step 3: Update view_header to add "Slots" button**

Replace the entire `view_header` body with:

```rust
fn view_header(&self, t: theme::Theme) -> Element<'_, Message> {
    let title = text("HonkHonk").size(24).color(t.ink());

    let slots_btn = button(text("Slots").size(14).color(t.ink()))
        .on_press(Message::ShowSlots)
        .style(move |_theme, _status| button::Style {
            background: Some(theme::bg_color(t.panel())),
            text_color: t.ink(),
            border: theme::tile_border(t.hairline(), 1.0),
            ..Default::default()
        });

    let search = search_bar::view_search_bar(&self.search_query);

    let stop_btn = button(text("Stop All").size(14).color(t.ink()))
        .on_press(Message::StopAll)
        .style(move |_theme, _status| button::Style {
            background: Some(theme::bg_color(t.panel())),
            text_color: t.ink(),
            border: theme::tile_border(t.hairline(), 1.0),
            ..Default::default()
        });

    row![title, slots_btn, space::horizontal(), search, stop_btn]
        .spacing(theme::space::LG)
        .align_y(iced::Alignment::Center)
        .into()
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass — view_main() extraction is mechanical, no logic change.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): branch view() on ViewMode, Slots header button left of search"
```

---

### Task 4: Create src/ui/slot_manager.rs — header, grid, tiles

**Files:**
- Create: `src/ui/slot_manager.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Register the module**

In `src/ui/mod.rs`, add:

```rust
pub mod slot_manager;
```

- [ ] **Step 2: Create the file with imports and utility helpers**

Create `src/ui/slot_manager.rs`:

```rust
use iced::widget::{button, column, container, row, scrollable, text, Column, Row, Space};
use iced::{Element, Length};

use crate::app::Message;
use crate::state::{SlotMap, SoundEntry};
use crate::ui::theme::{self, Hh, Theme, Tone};

fn tone_for(sound: &SoundEntry) -> Tone {
    let idx = sound
        .id
        .get(..8)
        .and_then(|s| u64::from_str_radix(s, 16).ok())
        .unwrap_or(0) as usize;
    Tone::from_index(idx)
}

fn fmt_duration(ms: Option<u64>) -> String {
    ms.map(|ms| format!("{}:{:02}", ms / 60000, (ms % 60000) / 1000))
        .unwrap_or_else(|| "—".into())
}
```

- [ ] **Step 3: Add the public entry point**

Append to `src/ui/slot_manager.rs`:

```rust
pub fn view_slot_manager<'a>(
    slots: &'a SlotMap,
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message> {
    let bound_count = (0u8..20).filter(|&i| slots.get(i).is_some()).count();
    let header = slot_header(bound_count, t);
    let divider = container(Space::new(1, Length::Fill)).style(move |_t| container::Style {
        background: Some(theme::bg_color(t.hairline())),
        ..Default::default()
    });
    let grid = slot_grid(slots, sounds, selected_slot, t);
    let side = sidebar(slots, sounds, selected_slot, t);
    let body = row![grid, divider, side].height(Length::Fill);
    container(column![header, body].height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.bg())),
            ..Default::default()
        })
        .into()
}
```

- [ ] **Step 4: Add slot_header**

Append to `src/ui/slot_manager.rs`:

```rust
fn slot_header<'a>(bound_count: usize, t: Theme) -> Element<'a, Message> {
    let back_btn = button(
        row![
            text("←").size(14).color(t.ink()),
            text("Back to sounds").size(13).color(t.ink()),
        ]
        .spacing(theme::space::XS)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::ShowMain)
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink(),
        border: theme::tile_border(t.hairline(), 1.0),
        ..Default::default()
    });

    let title = text("Slots").size(22).color(t.ink());
    let sep = text("·").size(14).color(t.ink_dim());
    let stats = text(format!("{bound_count} bound")).size(12).color(t.ink_dim());

    container(
        row![back_btn, title, sep, stats]
            .spacing(theme::space::MD)
            .align_y(iced::Alignment::Center),
    )
    .padding([theme::space::MD, theme::space::LG])
    .style(move |_t| container::Style {
        border: iced::Border {
            color: t.hairline(),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}
```

- [ ] **Step 5: Add slot_grid, slot_tile, bound_tile, empty_tile**

Append to `src/ui/slot_manager.rs`:

```rust
fn slot_grid<'a>(
    slots: &'a SlotMap,
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message> {
    let rows: Vec<Element<'_, Message>> = (0u8..4)
        .map(|row_idx| {
            let tiles: Vec<Element<'_, Message>> = (0u8..5)
                .map(|col_idx| {
                    let idx = row_idx * 5 + col_idx;
                    let sound =
                        slots.get(idx).and_then(|p| sounds.iter().find(|s| &s.path == p));
                    slot_tile(idx, sound, selected_slot == Some(idx), t)
                })
                .collect();
            Row::with_children(tiles).spacing(theme::space::MD).into()
        })
        .collect();

    scrollable(
        container(Column::with_children(rows).spacing(theme::space::MD))
            .padding(theme::space::LG)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn slot_tile<'a>(
    idx: u8,
    sound: Option<&'a SoundEntry>,
    selected: bool,
    t: Theme,
) -> Element<'a, Message> {
    match sound {
        Some(s) => bound_tile(idx, s, selected, t),
        None => empty_tile(idx, selected, t),
    }
}

fn bound_tile<'a>(idx: u8, sound: &'a SoundEntry, selected: bool, t: Theme) -> Element<'a, Message> {
    let tone = tone_for(sound);
    let bg = tone.tile_tint(t.is_dark());
    let border = if selected {
        iced::Border { color: t.ink(), width: 2.5, radius: 18.0.into() }
    } else {
        iced::Border { color: t.hairline(), width: 1.0, radius: 18.0.into() }
    };
    let circle = container(Space::new(40, 40)).style(move |_t| container::Style {
        background: Some(theme::bg_color(tone.highlight(t.is_dark()))),
        border: iced::Border { radius: 20.0.into(), ..Default::default() },
        ..Default::default()
    });
    button(
        column![
            text(format!("#{:02}", idx + 1)).size(10).color(t.ink_faint()),
            circle,
            text(sound.name.clone()).size(11).color(t.ink()),
            text("no hotkey").size(10).color(t.ink_faint()),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center)
        .padding(theme::space::SM),
    )
    .on_press(Message::SelectSlot(idx))
    .width(Length::Fill)
    .height(138)
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(bg)),
        text_color: t.ink(),
        border,
        ..Default::default()
    })
    .into()
}

fn empty_tile<'a>(idx: u8, selected: bool, t: Theme) -> Element<'a, Message> {
    let border = if selected {
        iced::Border { color: t.ink(), width: 2.5, radius: 18.0.into() }
    } else {
        iced::Border { color: t.hairline2(), width: 2.0, radius: 18.0.into() }
    };
    button(
        column![
            text(format!("#{:02}", idx + 1)).size(10).color(t.ink_faint()),
            text("+").size(22).color(t.ink_faint()),
            text("EMPTY").size(10).color(t.ink_faint()),
        ]
        .spacing(6)
        .align_x(iced::Alignment::Center)
        .padding(theme::space::SM),
    )
    .on_press(Message::SelectSlot(idx))
    .width(Length::Fill)
    .height(138)
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink_faint(),
        border,
        ..Default::default()
    })
    .into()
}
```

- [ ] **Step 6: Add sidebar stub so the module compiles**

Append to `src/ui/slot_manager.rs`:

```rust
fn sidebar<'a>(
    _slots: &'a SlotMap,
    _sounds: &'a [SoundEntry],
    _selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message> {
    container(
        text("Select a slot to inspect it")
            .size(13)
            .color(t.ink_faint()),
    )
    .width(320)
    .height(Length::Fill)
    .padding(theme::space::LG)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.panel())),
        ..Default::default()
    })
    .into()
}
```

- [ ] **Step 7: Wire import into app.rs and replace the temporary stub**

In `src/app.rs`, add the import at the top of the file with the other `use crate::ui` imports:

```rust
use crate::ui::slot_manager;
```

Replace the temporary `ViewMode::SlotManager` arm with the real call:

```rust
ViewMode::SlotManager => {
    slot_manager::view_slot_manager(
        &self.slots,
        &self.sounds,
        self.selected_slot,
        t,
    )
}
```

- [ ] **Step 8: Verify it compiles and tests pass**

```bash
cargo check 2>&1
cargo test 2>&1 | tail -10
```

Expected: clean compile, all tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/ui/slot_manager.rs src/ui/mod.rs src/app.rs
git commit -m "feat(ui): slot manager header + 4x5 grid with bound/empty tiles"
```

---

### Task 5: Add full sidebar and ship

**Files:**
- Modify: `src/ui/slot_manager.rs`

- [ ] **Step 1: Add sound_header helper**

In `src/ui/slot_manager.rs`, append:

```rust
fn sound_header<'a>(sound: &'a SoundEntry, t: Theme) -> Element<'a, Message> {
    let tone = tone_for(sound);
    let circle = container(Space::new(56, 56)).style(move |_t| container::Style {
        background: Some(theme::bg_color(tone.highlight(t.is_dark()))),
        border: iced::Border { radius: 28.0.into(), ..Default::default() },
        ..Default::default()
    });
    let info = column![
        text(sound.name.clone()).size(17).color(t.ink()),
        text(format!("{} · {}", sound.category, fmt_duration(sound.duration_ms)))
            .size(11)
            .color(t.ink_dim()),
    ]
    .spacing(2);
    row![circle, info]
        .spacing(theme::space::MD)
        .align_y(iced::Alignment::Center)
        .into()
}
```

- [ ] **Step 2: Add sidebar_bound helper**

Append to `src/ui/slot_manager.rs`:

```rust
fn sidebar_bound<'a>(idx: u8, sound: &'a SoundEntry, t: Theme) -> Element<'a, Message> {
    let slot_label = text(format!("SLOT #{:02}", idx + 1)).size(10).color(t.ink_dim());

    let hk_display = container(text("—").size(13).color(t.ink()))
        .padding([theme::space::SM, theme::space::MD])
        .width(Length::Fill)
        .style(move |_t| container::Style {
            border: iced::Border { color: t.accent(), width: 1.5, radius: 10.0.into() },
            ..Default::default()
        });

    let dot = container(Space::new(8, 8)).style(move |_t| container::Style {
        background: Some(theme::bg_color(t.good())),
        border: iced::Border { radius: 4.0.into(), ..Default::default() },
        ..Default::default()
    });

    let portal = container(
        row![dot, text("Registered via xdg-desktop-portal").size(11).color(t.ink_dim())]
            .spacing(theme::space::SM)
            .align_y(iced::Alignment::Center),
    )
    .padding([theme::space::SM, theme::space::MD])
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.bg())),
        border: theme::tile_border(t.hairline(), 1.0),
        ..Default::default()
    });

    let unbind = button(text("Unbind").size(12).color(iced::Color::from_rgb(0.86, 0.15, 0.15)))
        .on_press(Message::ClearSlot(idx))
        .width(Length::Fill)
        .style(move |_t, _s| button::Style {
            background: None,
            text_color: iced::Color::from_rgb(0.86, 0.15, 0.15),
            border: iced::Border {
                color: iced::Color::from_rgba(0.86, 0.15, 0.15, 0.4),
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        });

    column![
        slot_label,
        sound_header(sound, t),
        text("GLOBAL HOTKEY").size(11).color(t.ink_dim()),
        hk_display,
        text("PORTAL STATUS").size(11).color(t.ink_dim()),
        portal,
        unbind,
    ]
    .spacing(theme::space::MD)
    .into()
}
```

- [ ] **Step 3: Add sidebar_empty helper**

Append to `src/ui/slot_manager.rs`:

```rust
fn sidebar_empty<'a>(idx: u8, t: Theme) -> Element<'a, Message> {
    let slot_label = text(format!("SLOT #{:02}", idx + 1)).size(10).color(t.ink_dim());

    let placeholder = container(
        column![
            text("🪿").size(32),
            text("Slot is empty").size(13).color(t.ink()),
            text("Assign via right-click on any sound tile")
                .size(11)
                .color(t.ink_dim()),
        ]
        .spacing(theme::space::SM)
        .align_x(iced::Alignment::Center)
        .padding(theme::space::LG),
    )
    .width(Length::Fill)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.bg())),
        border: iced::Border {
            color: t.hairline2(),
            width: 2.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    });

    column![slot_label, placeholder].spacing(theme::space::MD).into()
}
```

- [ ] **Step 4: Replace the sidebar stub with the real implementation**

Find the stub `sidebar` function (the one with `_slots`, `_sounds`, `_selected_slot` params) and replace it entirely with:

```rust
fn sidebar<'a>(
    slots: &'a SlotMap,
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message> {
    let inner: Element<'_, Message> = match selected_slot {
        None => text("Select a slot to inspect it")
            .size(13)
            .color(t.ink_faint())
            .into(),
        Some(idx) => {
            let sound = slots.get(idx).and_then(|p| sounds.iter().find(|s| &s.path == p));
            match sound {
                Some(s) => sidebar_bound(idx, s, t),
                None => sidebar_empty(idx, t),
            }
        }
    };
    container(inner)
        .width(320)
        .height(Length::Fill)
        .padding(theme::space::LG)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(t.panel())),
            ..Default::default()
        })
        .into()
}
```

- [ ] **Step 5: Build, test, lint, format**

```bash
cargo build --release 2>&1 | tail -5
cargo test 2>&1 | tail -10
cargo clippy -- -D warnings 2>&1
cargo fmt -- --check 2>&1
```

Expected:
- Build: `Finished release [optimized]`
- Tests: all pass
- Clippy: zero warnings. If `too_many_lines` fires on any function, split it by extracting a named sub-helper.
- Fmt: clean (run `cargo fmt` if it reports diffs, then re-check)

- [ ] **Step 6: Commit**

```bash
git add src/ui/slot_manager.rs
git commit -m "feat(ui): slot manager sidebar — bound/empty detail panels"
```

- [ ] **Step 7: Create PR**

```bash
gh pr create \
  --title "feat(ui): slot manager full-window view" \
  --body "$(cat <<'EOF'
## Summary
- Add `ViewMode` enum — `Main | SlotManager` — to `HonkHonk` state
- "Slots" nav button in header anchors left next to title, separated from Stop All global action
- Slot manager: 4×5 grid of slot tiles + right sidebar with slot detail
- Bound slots: tone-tinted tile, sound name, sticker placeholder circle, hotkey badge
- Empty slots: dashed border, EMPTY label
- Sidebar: sound info, read-only hotkey display, portal status, Unbind button
- Extend theme with `hairline2`, `good` colors and `Tone::highlight`

## Out of scope
- Sound assignment from within slot manager (deferred to right-click context menu PR)
- Hotkey rebinding UI
- Conflict detection

## Test plan
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` clean
- [ ] Launch app — "Slots" button visible in header, left of search bar
- [ ] Click "Slots" — full-window slot manager appears with 4×5 grid
- [ ] Bound slots show sound name + tinted background
- [ ] Empty slots show dashed border + EMPTY label
- [ ] Click any slot — right sidebar shows slot detail
- [ ] Bound slot sidebar shows Unbind button; click clears the slot, sidebar shows empty state
- [ ] "Back to sounds" returns to main grid
- [ ] Header "Slots" button reappears after returning to main view
EOF
)"
```
