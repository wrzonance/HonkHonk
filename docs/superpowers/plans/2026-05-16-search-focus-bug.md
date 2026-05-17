# Search Bar Focus Bug Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix search box losing focus after the first keystroke by making the widget tree shape stable across all query states.

**Architecture:** `view_search_bar()` currently returns a `container(input)` when query is empty and a `stack![input, overlay]` when non-empty. This structural change causes Iced's reconciler to treat the `text_input` as a new widget instance on first keystroke, resetting focus. The fix: always return `stack![input, overlay]`, where `overlay` is an empty `row![]` when query is empty and the ✕ clear button container when non-empty.

**Tech Stack:** Rust, Iced 0.14

---

### Task 1: Fix `view_search_bar` to always emit `stack!`

**Files:**
- Modify: `src/ui/search_bar.rs:47-75`

Per project testing philosophy (`CLAUDE.md`), Iced view rendering is explicitly excluded from unit tests ("framework responsibility"). The existing state tests in `src/app.rs` (`search_changed_updates_query`, `search_filters_sounds`) cover the behavior that matters. Verification here is build + lint + manual smoke test.

- [ ] **Step 1: Replace `view_search_bar` body**

Open `src/ui/search_bar.rs` and replace the entire file with:

```rust
use iced::widget::{button, container, row, text, text_input};
use iced::{Alignment, Border, Element, Length, Padding};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

pub fn view_search_bar(query: &str) -> Element<'_, Message> {
    let t = Theme::Dark;

    // Reserve right space for the clear button so typed text doesn't run under it.
    let padding = if query.is_empty() {
        Padding::from(5.0)
    } else {
        Padding {
            top: 5.0,
            right: 30.0,
            bottom: 5.0,
            left: 10.0,
        }
    };

    let input: Element<'_, Message> = text_input("Find a sound to honk\u{2026}", query)
        .on_input(Message::SearchChanged)
        .size(theme::font::BODY)
        .width(Length::Fixed(300.0))
        .padding(padding)
        .style(move |_theme, status| {
            let border_color = match status {
                text_input::Status::Focused { .. } => t.accent(),
                _ => t.hairline(),
            };
            text_input::Style {
                background: theme::bg_color(t.panel()),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: theme::radius::PILL,
                },
                icon: t.ink_dim(),
                placeholder: t.ink_faint(),
                value: t.ink(),
                selection: t.accent(),
            }
        })
        .into();

    // Always use stack so the widget tree shape is stable across all query states.
    // Changing from container → stack on first keystroke caused Iced to reset
    // text_input focus. An empty row as the second layer has no hit area or cost.
    let overlay: Element<'_, Message> = if query.is_empty() {
        row![].into()
    } else {
        // Clear button — floats over the right edge of the input via stack.
        let clear_btn = button(text("\u{2715}").size(theme::font::BODY).color(t.ink_dim()))
            .on_press(Message::SearchChanged(String::new()))
            .padding(Padding {
                top: 4.0,
                right: 10.0,
                bottom: 4.0,
                left: 4.0,
            })
            .style(move |_t, status| button::Style {
                text_color: match status {
                    button::Status::Hovered | button::Status::Pressed => t.ink(),
                    _ => t.ink_dim(),
                },
                background: None,
                ..Default::default()
            });

        container(clear_btn)
            .width(Length::Fixed(300.0))
            .align_x(Alignment::End)
            .align_y(Alignment::Center)
            .into()
    };

    iced::widget::stack![input, overlay].into()
}
```

Key changes:
- Added `row` to the `use iced::widget::` import
- Removed the `if query.is_empty() { return container(input).into(); }` early return (lines 47–49)
- Built `overlay` as `Element<'_, Message>` — `row![].into()` when empty, clear button container when non-empty
- Single `stack![input, overlay]` exit point

- [ ] **Step 2: Verify build passes**

```bash
cargo build 2>&1
```

Expected: compiles with zero errors.

- [ ] **Step 3: Run lint**

```bash
cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 4: Run formatter check**

```bash
cargo fmt -- --check 2>&1
```

Expected: no diff output (exit 0). If it reports diffs, run `cargo fmt` and re-check.

- [ ] **Step 5: Run existing tests**

```bash
cargo test 2>&1
```

Expected: all tests pass, including `search_changed_updates_query` and `search_filters_sounds`.

- [ ] **Step 6: Manual smoke test**

Run the app:

```bash
cargo run 2>&1
```

1. Click the search box once.
2. Type several characters without clicking again (e.g., "honk").
3. All characters should appear in the box — focus must not drop after the first character.
4. Click outside the search box — focus should leave.
5. Click the search box again and verify it re-focuses.
6. Type a character, then click the ✕ clear button — box should clear.

- [ ] **Step 7: Commit**

```bash
git add src/ui/search_bar.rs
git commit -m "fix(ui): stable stack layout prevents search focus reset on first keystroke"
```
