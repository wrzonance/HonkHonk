# Effects Side Panel + Reusable Panel Framework — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the always-visible effects panel into an on-demand right-edge drawer (pull tab + slide), abstracted as a reusable `side_panel` framework, landing the panel-geometry hook for #144.

**Architecture:** A new `src/ui/side_panel/` module holds all logic: a wall-clock `PanelAnim` state machine (`anim.rs`), pure `panel_geometry` (`geometry.rs`, the #144 hook), and a `view_side_panel` renderer (`view.rs`) that slides a tab+body drawer via the stock Iced 0.14 `Float` widget over a fading scrim. `app.rs` gains only thin glue: two messages, two fields, small update arms, a subscription-gate tweak, and a few `view_main` lines — while losing the inline effects element.

**Tech Stack:** Rust, Iced 0.14 (MVU; `float`, `mouse_area`, `stack`, `container.align_right`), `std::time::{Instant, Duration}`.

## Global Constraints

- Files ≤ 400 lines; functions ≤ 50 lines.
- clippy `-D warnings` clean: cognitive-complexity ≤ 10, too-many-arguments ≤ 5, too-many-lines ≤ 50, type-complexity ≤ 200.
- No `.unwrap()` / `panic!()` in non-test code.
- `cargo fmt` clean (CI gates fmt AND clippy).
- **No business logic added to `app.rs`** (a known 2.7k-line violation). app.rs changes are thin glue only, kept minimal; all logic lives in `src/ui/side_panel/`.
- **Wall-clock animation only** — no predict-and-correct (the #139 jitter regression class).
- **Stock Iced 0.14 widgets only.** Drawer slide via `float` + horizontal translate; scrim via `mouse_area`; layering via `stack!`.
- Iced view rendering is NOT unit-tested (CLAUDE.md) — views get a build-smoke test only; pure logic is unit-tested to the 80% target.
- Branch: `feat/effects-side-panel` (already created from `origin/main`). Never commit to `main`. Commit format: Conventional Commits, ending `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `src/ui/side_panel/anim.rs` | Create | `PanelAnim` wall-clock state machine + easing |
| `src/ui/side_panel/geometry.rs` | Create | `panel_geometry` + `PanelRect` (#144 hook) |
| `src/ui/side_panel/view.rs` | Create | `view_side_panel` + `SidePanelConfig` |
| `src/ui/side_panel/mod.rs` | Create | module wiring + re-exports |
| `src/ui/mod.rs` | Modify | register `pub mod side_panel;` |
| `src/ui/effects_panel_view.rs` | Modify | preset chips → vertical column (narrow drawer) |
| `src/app.rs` | Modify | messages, fields, update arms, subscription, `view_main` |

---

## Task 1: `PanelAnim` wall-clock animation state machine

**Files:**
- Create: `src/ui/side_panel/anim.rs`

**Interfaces:**
- Consumes: nothing (pure; `std::time` only).
- Produces:
  - `pub const SLIDE_DURATION: Duration` (150 ms)
  - `pub enum PanelAnim` with `Default` = closed
  - `pub fn progress(&self, now: Instant) -> f32` (eased 0..=1, non-mutating)
  - `pub fn tick(&mut self, now: Instant) -> f32` (settles then returns progress)
  - `pub fn toggle(&mut self, now: Instant)`
  - `pub fn close(&mut self, now: Instant)`
  - `pub fn is_animating(&self) -> bool`
  - `pub fn is_open(&self) -> bool`

- [ ] **Step 1: Write the failing tests**

Create `src/ui/side_panel/anim.rs` with ONLY the test module first (it will not compile — that is the RED state):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn default_is_closed() {
        let a = PanelAnim::default();
        let now = Instant::now();
        assert_eq!(a.progress(now), 0.0);
        assert!(!a.is_open());
        assert!(!a.is_animating());
    }

    #[test]
    fn toggle_opens_then_settles_to_one() {
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        a.toggle(t0);
        assert!(a.is_open());
        assert!(a.is_animating());
        assert!(a.progress(t0) < 0.01);
        let settled = a.tick(t0 + SLIDE_DURATION);
        assert_eq!(settled, 1.0);
        assert!(!a.is_animating());
        assert!(a.is_open());
    }

    #[test]
    fn toggle_again_closes_to_zero() {
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        a.toggle(t0);
        a.tick(t0 + SLIDE_DURATION); // now Open
        let t1 = t0 + SLIDE_DURATION;
        a.toggle(t1);
        assert!(!a.is_open());
        assert!(a.is_animating());
        let settled = a.tick(t1 + SLIDE_DURATION);
        assert_eq!(settled, 0.0);
        assert!(!a.is_animating());
    }

    #[test]
    fn progress_monotonic_and_clamped_while_opening() {
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        a.toggle(t0);
        let mut prev = 0.0;
        for ms in 0..=200 {
            let v = a.progress(t0 + Duration::from_millis(ms));
            assert!((0.0..=1.0).contains(&v), "out of range at {ms}ms: {v}");
            assert!(v >= prev - 1e-6, "went backward at {ms}ms: {v} < {prev}");
            prev = v;
        }
        assert_eq!(a.progress(t0 + Duration::from_millis(200)), 1.0);
    }

    #[test]
    fn mid_slide_reversal_is_continuous() {
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        a.toggle(t0); // opening
        let mid = t0 + Duration::from_millis(75);
        let before = a.progress(mid);
        a.toggle(mid); // reverse to closing
        let after = a.progress(mid);
        assert!((after - before).abs() < 1e-3, "snapped: {before} -> {after}");
        assert!(!a.is_open());
    }

    #[test]
    fn close_is_idempotent_when_closed() {
        let mut a = PanelAnim::default();
        let now = Instant::now();
        a.close(now);
        assert!(!a.is_open());
        assert!(!a.is_animating());
    }

    #[test]
    fn ease_hits_endpoints_and_midpoint() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        assert!((ease(0.5) - 0.5).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib side_panel::anim 2>&1 | tail -20`
Expected: compile error — `PanelAnim`, `ease`, `SLIDE_DURATION` not found.

- [ ] **Step 3: Write the implementation**

Prepend above the test module in `src/ui/side_panel/anim.rs`:

```rust
//! Wall-clock open/close animation for side panels. Mirrors the playhead's
//! drift-free wall-clock approach (#139): progress is a pure function of elapsed
//! time since the current leg began, eased with smoothstep — no predict-and-correct.

use std::time::{Duration, Instant};

/// Duration of a full open or close slide.
pub const SLIDE_DURATION: Duration = Duration::from_millis(150);

/// Open/close animation state. `0.0` = fully closed, `1.0` = fully open.
#[derive(Debug, Clone, Copy)]
pub enum PanelAnim {
    /// Steady at `progress` (`0.0` closed or `1.0` open).
    Settled(f32),
    /// Mid-slide: `from`→`to` over `SLIDE_DURATION` starting at `start`.
    Animating { from: f32, to: f32, start: Instant },
}

impl Default for PanelAnim {
    fn default() -> Self {
        PanelAnim::Settled(0.0)
    }
}

impl PanelAnim {
    /// Eased progress `0.0..=1.0` at `now`, without mutating state.
    pub fn progress(&self, now: Instant) -> f32 {
        match *self {
            PanelAnim::Settled(v) => v,
            PanelAnim::Animating { from, to, start } => {
                let frac = ease(fraction(now.saturating_duration_since(start), SLIDE_DURATION));
                from + (to - from) * frac
            }
        }
    }

    /// Settles the leg once it has fully elapsed, then returns progress at `now`.
    /// Call once per frame.
    pub fn tick(&mut self, now: Instant) -> f32 {
        if let PanelAnim::Animating { to, start, .. } = *self {
            if now.saturating_duration_since(start) >= SLIDE_DURATION {
                *self = PanelAnim::Settled(to);
            }
        }
        self.progress(now)
    }

    /// True while a slide is in progress (drives the frame subscription).
    pub fn is_animating(&self) -> bool {
        matches!(self, PanelAnim::Animating { .. })
    }

    /// True when open or opening (target progress > 0).
    pub fn is_open(&self) -> bool {
        self.target() > 0.0
    }

    /// Reverses or begins a slide toward the opposite end, continuous from the
    /// current visible progress (no snap on mid-slide reversal).
    pub fn toggle(&mut self, now: Instant) {
        let to = if self.is_open() { 0.0 } else { 1.0 };
        self.retarget(to, now);
    }

    /// Slides toward closed, continuous from current progress. No-op if already
    /// fully closed or closing.
    pub fn close(&mut self, now: Instant) {
        if self.is_open() {
            self.retarget(0.0, now);
        }
    }

    fn target(&self) -> f32 {
        match *self {
            PanelAnim::Settled(v) => v,
            PanelAnim::Animating { to, .. } => to,
        }
    }

    fn retarget(&mut self, to: f32, now: Instant) {
        let from = self.progress(now);
        *self = if (from - to).abs() < f32::EPSILON {
            PanelAnim::Settled(to)
        } else {
            PanelAnim::Animating { from, to, start: now }
        };
    }
}

/// `elapsed / duration` clamped to `0.0..=1.0`; zero duration yields `1.0`
/// (instantly complete — never divides by zero).
fn fraction(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

/// Smoothstep ease-in-out on `0.0..=1.0`: zero velocity at both ends.
fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib side_panel::anim 2>&1 | tail -20`
Expected: all 7 tests pass. (Module must be registered — if `cargo test` cannot find `side_panel`, temporarily ensure `src/ui/side_panel/mod.rs` declares `mod anim;` and `src/ui/mod.rs` declares `pub mod side_panel;`. Task 3 finalizes `mod.rs`; for this task a minimal `mod.rs` with `mod anim;` is acceptable.)

- [ ] **Step 5: Commit**

```bash
git add src/ui/side_panel/anim.rs src/ui/side_panel/mod.rs src/ui/mod.rs
git commit -m "feat(ui): wall-clock PanelAnim state machine for side panels (#143)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `panel_geometry` + `PanelRect` (the #144 hook)

**Files:**
- Create: `src/ui/side_panel/geometry.rs`

**Interfaces:**
- Consumes: `iced::Point`.
- Produces:
  - `pub struct PanelRect { pub x: f32, pub y: f32, pub w: f32, pub h: f32, pub center: Point }`
  - `pub fn panel_geometry(window: (f32, f32), panel_w: f32) -> PanelRect`

- [ ] **Step 1: Write the failing tests**

Create `src/ui/side_panel/geometry.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_window_right_anchors_body() {
        let r = panel_geometry((1280.0, 800.0), 400.0);
        assert_eq!(r.x, 880.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.w, 400.0);
        assert_eq!(r.h, 800.0);
        assert_eq!(r.center, Point::new(1080.0, 400.0));
    }

    #[test]
    fn panel_wider_than_window_clamps() {
        let r = panel_geometry((300.0, 600.0), 400.0);
        assert_eq!(r.w, 300.0);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.center, Point::new(150.0, 300.0));
    }

    #[test]
    fn degenerate_window_is_finite() {
        let r = panel_geometry((0.0, 0.0), 400.0);
        assert_eq!(r.w, 0.0);
        assert_eq!(r.x, 0.0);
        assert!(r.center.x.is_finite() && r.center.y.is_finite());
    }

    #[test]
    fn negative_window_is_guarded() {
        let r = panel_geometry((-50.0, -50.0), 400.0);
        assert_eq!(r.w, 0.0);
        assert_eq!(r.x, 0.0);
        assert!(r.center.x.is_finite() && r.center.y.is_finite());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib side_panel::geometry 2>&1 | tail -20`
Expected: compile error — `panel_geometry`, `PanelRect` not found.

- [ ] **Step 3: Write the implementation**

Prepend above the tests:

```rust
//! Panel geometry for the side-panel framework. `panel_geometry` returns the
//! BODY rectangle at full open — right-anchored against the window's right edge.
//! Its edges and `center` are the hook the #144 feather-puff animation consumes
//! to know where to burst feathers from.

use iced::Point;

/// The panel body's rectangle at full open, plus its center point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub center: Point,
}

/// Geometry of a right-anchored body `panel_w` wide in a `(win_w, win_h)` window.
/// Clamps degenerate inputs: width is never negative and never exceeds the
/// window; the center is always finite.
pub fn panel_geometry(window: (f32, f32), panel_w: f32) -> PanelRect {
    let win_w = window.0.max(0.0);
    let win_h = window.1.max(0.0);
    let w = panel_w.clamp(0.0, win_w);
    let x = win_w - w;
    PanelRect {
        x,
        y: 0.0,
        w,
        h: win_h,
        center: Point::new(x + w / 2.0, win_h / 2.0),
    }
}
```

Add `mod geometry;` to `src/ui/side_panel/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib side_panel::geometry 2>&1 | tail -20`
Expected: all 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/ui/side_panel/geometry.rs src/ui/side_panel/mod.rs
git commit -m "feat(ui): panel_geometry + PanelRect (the #144 feather-puff hook) (#143)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `view_side_panel` + `SidePanelConfig` + module wiring

**Files:**
- Create: `src/ui/side_panel/view.rs`
- Modify: `src/ui/side_panel/mod.rs` (finalize re-exports)
- Modify: `src/ui/mod.rs` (register `pub mod side_panel;`)

**Interfaces:**
- Consumes: `crate::app::Message` (for the carried toggle/close messages); `crate::ui::theme::{self, Hh, Theme}`.
- Produces:
  - `pub struct SidePanelConfig { pub panel_w: f32, pub tab_w: f32, pub title: &'static str, pub on_toggle: Message, pub on_close: Message }`
  - `pub fn view_side_panel<'a>(cfg: SidePanelConfig, progress: f32, window: (f32, f32), body: Element<'a, Message>, t: Theme) -> Element<'a, Message>`

**Notes for the implementer:**
- The drawer slides via `iced::widget::float(...)` whose `.translate(|content, viewport| Vector)` closure offsets it horizontally by `(1.0 - progress) * panel_w`. `Float` lays the drawer out at natural size and, while translated, re-hosts it as an overlay — so the body keeps full width while sliding off-screen and the tab/sliders stay clickable at their drawn position. Verified against iced_widget 0.14.2 source.
- Right-anchor with `container(floated).align_right(Length::Fill).height(Length::Fill)`.
- The scrim is mounted ONLY when `progress > 0.0`, so a closed panel never intercepts grid input. (`container`/`float` forward events to children only; empty regions fall through the `stack!` to the grid below — confirmed from Pin/Float/container source.)
- `Message` is `Clone`; clone `cfg.on_toggle` / `cfg.on_close` into the tab/scrim/close-button.
- Use `Space::new().width(Length::Fill)` (the codebase idiom) for the header spacer.

- [ ] **Step 1: Write the implementation + build-smoke test**

Create `src/ui/side_panel/view.rs`:

```rust
//! Renders the side-panel drawer: a fading scrim over the main view plus a
//! pull-tab + body that slides in from the right edge. The slide uses `Float`
//! (stock Iced 0.14): the drawer is laid out at natural size, then translated by
//! the hidden fraction; while translated `Float` re-hosts it as an overlay, so
//! the body keeps full width off-screen and input lands at the drawn position.

use iced::widget::{button, column, container, float, mouse_area, row, scrollable, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Vector};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

/// Static configuration + messages a consumer hands the framework.
pub struct SidePanelConfig {
    /// Panel body width in logical px.
    pub panel_w: f32,
    /// Pull-tab width in logical px.
    pub tab_w: f32,
    /// Title shown in the drawer header.
    pub title: &'static str,
    /// Emitted when the pull tab is pressed.
    pub on_toggle: Message,
    /// Emitted when the scrim or the header ✕ is pressed.
    pub on_close: Message,
}

/// Max scrim opacity at full open.
const SCRIM_MAX: f32 = 0.5;
/// Header / ✕ font size.
const TITLE_SIZE: f32 = 14.0;
/// Chevron glyph size.
const GLYPH_SIZE: f32 = 18.0;

/// Builds the side-panel overlay layer for animation `progress` (0=closed..1=open)
/// in a `window`-sized area, wrapping `body` as the drawer content.
pub fn view_side_panel<'a>(
    cfg: SidePanelConfig,
    progress: f32,
    _window: (f32, f32),
    body: Element<'a, Message>,
    t: Theme,
) -> Element<'a, Message> {
    let progress = progress.clamp(0.0, 1.0);
    let panel_w = cfg.panel_w;

    let drawer = row![tab(&cfg, progress, t), panel_body(&cfg, body, t)].height(Length::Fill);
    let floated =
        float(drawer).translate(move |_content, _viewport| Vector::new((1.0 - progress) * panel_w, 0.0));
    let anchored = container(floated).align_right(Length::Fill).height(Length::Fill);

    if progress <= 0.0 {
        return anchored.into();
    }

    let alpha = SCRIM_MAX * progress;
    let scrim = mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_| {
            container::Style {
                background: Some(Background::Color(Color { a: alpha, ..Color::BLACK })),
                ..Default::default()
            }
        }),
    )
    .on_press(cfg.on_close.clone());

    iced::widget::stack![scrim, anchored]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Vertical pull tab. Chevron hints the action: "‹" to pull open, "›" to push closed.
fn tab<'a>(cfg: &SidePanelConfig, progress: f32, t: Theme) -> Element<'a, Message> {
    let glyph = if progress > 0.5 { "\u{203A}" } else { "\u{2039}" };
    button(
        container(text(glyph).size(GLYPH_SIZE).color(t.ink()))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(cfg.tab_w))
    .height(Length::Fill)
    .on_press(cfg.on_toggle.clone())
    .style(move |_th, _s| tab_style(t))
    .into()
}

/// Drawer body: title bar (+ ✕) over the scrollable content.
fn panel_body<'a>(cfg: &SidePanelConfig, body: Element<'a, Message>, t: Theme) -> Element<'a, Message> {
    let header = row![
        text(cfg.title).size(TITLE_SIZE).color(t.ink()),
        Space::new().width(Length::Fill),
        close_button(cfg, t),
    ]
    .align_y(Alignment::Center);

    let content = column![header, scrollable(body).height(Length::Fill)]
        .spacing(theme::space::MD)
        .padding(theme::space::LG);

    container(content)
        .width(Length::Fixed(cfg.panel_w))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: Border { color: t.hairline(), width: 1.0, radius: theme::radius::MD },
            ..Default::default()
        })
        .into()
}

fn close_button<'a>(cfg: &SidePanelConfig, t: Theme) -> Element<'a, Message> {
    button(text("\u{2715}").size(TITLE_SIZE).color(t.ink()))
        .on_press(cfg.on_close.clone())
        .style(move |_th, _s| tab_style(t))
        .into()
}

fn tab_style(t: Theme) -> button::Style {
    button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink(),
        border: Border { color: t.hairline(), width: 1.0, radius: theme::radius::MD },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> SidePanelConfig {
        // Uses pre-existing Message variants so this smoke test does not depend on
        // the new effects-panel messages (added in the app-wiring task).
        SidePanelConfig {
            panel_w: 400.0,
            tab_w: 28.0,
            title: "Test",
            on_toggle: Message::StopAll,
            on_close: Message::StopAll,
        }
    }

    #[test]
    fn builds_across_progress() {
        for p in [0.0_f32, 0.5, 1.0] {
            let _el = view_side_panel(cfg(), p, (1280.0, 800.0), text("body").into(), Theme::Dark);
        }
    }
}
```

Finalize `src/ui/side_panel/mod.rs`:

```rust
//! Reusable side-panel framework: a right-edge drawer with a pull tab that slides
//! over the main view. The effects panel (#143) is the first consumer; future
//! settings panels reuse it. Animation lives in [`anim`], geometry (the #144 hook)
//! in [`geometry`], rendering in [`view`].

mod anim;
mod geometry;
mod view;

pub use anim::{PanelAnim, SLIDE_DURATION};
pub use geometry::{panel_geometry, PanelRect};
pub use view::{view_side_panel, SidePanelConfig};
```

Register the module in `src/ui/mod.rs` — add alphabetically before `pub mod slot_manager;`:

```rust
pub mod side_panel;
```

- [ ] **Step 2: Build + run the smoke test**

Run: `cargo build 2>&1 | tail -20 && cargo test --lib side_panel::view 2>&1 | tail -10`
Expected: build succeeds; `builds_across_progress` passes. If `align_right`, `center_x`, or `float` differ in arity, adjust to the iced 0.14.2 signatures (`container.align_right(Length)`, `container.center_x(Length)`, `iced::widget::float(content)`), then rebuild.

- [ ] **Step 3: Verify clippy + fmt clean**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1 | tail -20`
Expected: no warnings. Note any unused `panel_geometry`/`PanelRect`/`SLIDE_DURATION`/`panel_w param` re-exports: they are consumed in Task 5 (and #144). If `dead_code` fires on a re-export before Task 5, it is expected at this checkpoint — confirm it disappears after Task 5 rather than adding `allow`. (`view_side_panel`'s `_window` param is intentionally unused now; keep the leading underscore.)

- [ ] **Step 4: Commit**

```bash
git add src/ui/side_panel/view.rs src/ui/side_panel/mod.rs src/ui/mod.rs
git commit -m "feat(ui): reusable side_panel drawer view (Float slide + scrim) (#143)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Effects body — preset selector as a vertical column

The drawer body is ~400 px wide; the existing preset chips are a horizontal `Row` of 5 that would overflow. Render them as a full-width vertical column (a natural "selector" in a narrow drawer). This is the only body change — sliders and the master row already fit ~368 px of content.

**Files:**
- Modify: `src/ui/effects_panel_view.rs` (`view_preset_chips`, ~lines 87-108)

**Interfaces:**
- Consumes: nothing new.
- Produces: unchanged public surface (`view_effects_panel` signature is identical).

- [ ] **Step 1: Change the preset chips Row → full-width Column**

In `src/ui/effects_panel_view.rs`, replace the body of `view_preset_chips` so the chips stack vertically and each fills the panel width. Replace:

```rust
    let mut chips = Row::new().spacing(theme::space::SM);
    for p in PresetId::ALL {
```

…and the closing `chips.into()` block, with a `Column`:

```rust
    let mut chips = Column::new().spacing(theme::space::SM).width(Length::Fill);
    for p in PresetId::ALL {
        let selected = p == active;
        let chip = button(
            column![
                text(format!("{} {}", p.glyph(), p.label()))
                    .size(theme::font::LABEL)
                    .color(t.ink()),
                text(p.description())
                    .size(theme::font::LABEL)
                    .color(t.ink_dim()),
            ]
            .spacing(theme::space::XS),
        )
        .width(Length::Fill)
        .on_press(Message::SelectEffectPreset(p))
        .padding([theme::space::XS, theme::space::MD])
        .style(move |_th, _s| chip_style(t, selected));
        chips = chips.push(chip);
    }
    chips.into()
```

Ensure the `use` line imports `Column` (it already imports `Column, Row`; the now-unused `Row` import must be removed to satisfy `-D warnings`). Update:

```rust
use iced::widget::{button, column, container, row, slider, text, Column, Row, Space};
```
→
```rust
use iced::widget::{button, column, container, row, slider, text, Column, Space};
```

- [ ] **Step 2: Build + smoke test**

Run: `cargo build 2>&1 | tail -10 && cargo test --lib effects_panel_view 2>&1 | tail -10`
Expected: builds; `effects_panel_view_builds_for_each_preset` still passes.

- [ ] **Step 3: clippy + fmt**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1 | tail -20`
Expected: no warnings (no unused `Row` import).

- [ ] **Step 4: Commit**

```bash
git add src/ui/effects_panel_view.rs
git commit -m "feat(ui): effects preset selector as vertical column for the drawer (#143)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Wire the drawer into `app.rs` (messages, state, update, view)

All app.rs changes land together so there is no half-wired `dead_code` window. Thin glue only — no business logic.

**Files:**
- Modify: `src/app.rs` (imports; `Message`; struct fields; both initializers; `update` arms for `ToggleEffectsPanel`/`CloseEffectsPanel`, `Frame`, `EscapePressed`; `subscription`; `view_main`)

**Interfaces:**
- Consumes (from Tasks 1 & 3): `crate::ui::side_panel::{PanelAnim, SidePanelConfig, view_side_panel, SLIDE_DURATION}`.
- Produces: `Message::ToggleEffectsPanel`, `Message::CloseEffectsPanel`; `effects_panel: PanelAnim`, `panel_progress: f32` fields.

- [ ] **Step 1: Write the failing app-state tests**

In `src/app.rs`, inside the existing `#[cfg(test)] mod tests` block (which has `use super::*;`), add:

```rust
    #[test]
    fn toggle_effects_panel_opens_then_closes() {
        let mut app = HonkHonk::new_for_test();
        assert!(!app.effects_panel.is_open());
        let _ = app.update(Message::ToggleEffectsPanel);
        assert!(app.effects_panel.is_open());
        assert!(app.effects_panel.is_animating());
        let _ = app.update(Message::ToggleEffectsPanel);
        assert!(!app.effects_panel.is_open());
    }

    #[test]
    fn close_effects_panel_closes_open_panel() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ToggleEffectsPanel);
        assert!(app.effects_panel.is_open());
        let _ = app.update(Message::CloseEffectsPanel);
        assert!(!app.effects_panel.is_open());
    }

    #[test]
    fn escape_closes_open_effects_panel() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ToggleEffectsPanel);
        assert!(app.effects_panel.is_open());
        let _ = app.update(Message::EscapePressed);
        assert!(!app.effects_panel.is_open());
    }

    #[test]
    fn frame_settles_panel_progress_after_slide() {
        let mut app = HonkHonk::new_for_test();
        let _ = app.update(Message::ToggleEffectsPanel); // opening
        let later = Instant::now() + crate::ui::side_panel::SLIDE_DURATION;
        let _ = app.update(Message::Frame(later));
        assert_eq!(app.panel_progress, 1.0);
        assert!(!app.effects_panel.is_animating());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::tests::toggle_effects_panel 2>&1 | tail -20`
Expected: compile error — `effects_panel` field and `ToggleEffectsPanel`/`CloseEffectsPanel` variants do not exist.

- [ ] **Step 3: Add the import**

In `src/app.rs`, after `use crate::ui::effects_panel_view;` (line ~15) add:

```rust
use crate::ui::side_panel::{self, PanelAnim, SidePanelConfig};
```

- [ ] **Step 4: Add the two Message variants**

In the `pub enum Message`, under the `// Voice effects` group (after `SetEffectParamUi { ... }`), add:

```rust
    /// Toggle the effects side panel open/closed (pull tab).
    ToggleEffectsPanel,
    /// Close the effects side panel (scrim / ✕ / Escape).
    CloseEffectsPanel,
```

- [ ] **Step 5: Add the two struct fields**

In `struct HonkHonk`, after the `waveform_cache` field (~line 188), add:

```rust
    /// Open/close animation state for the effects side panel (#143). Logic lives
    /// in `ui::side_panel`.
    effects_panel: PanelAnim,
    /// Eased panel progress (0=closed..1=open) fed to the view; refreshed each
    /// frame by `effects_panel.tick`.
    panel_progress: f32,
```

In BOTH initializers (`new` ~line 352 and `new_for_test` ~line 398), after `waveform_cache: std::collections::HashMap::new(),` add:

```rust
            effects_panel: PanelAnim::default(),
            panel_progress: 0.0,
```

- [ ] **Step 6: Add the update arms**

In `update`, alongside the other effects arms (near the `SelectEffectPreset` group, ~line 949), add:

```rust
            Message::ToggleEffectsPanel => {
                let now = Instant::now();
                self.effects_panel.toggle(now);
                self.panel_progress = self.effects_panel.progress(now);
                Task::none()
            }
            Message::CloseEffectsPanel => {
                let now = Instant::now();
                self.effects_panel.close(now);
                self.panel_progress = self.effects_panel.progress(now);
                Task::none()
            }
```

- [ ] **Step 7: Tick the panel in the `Frame` handler**

Replace the existing `Frame` arm (~line 768):

```rust
            Message::Frame(now) => {
                if let Some(ref clock) = self.playhead {
                    self.display_progress = clock.display(now);
                }
                Task::none()
            }
```

with:

```rust
            Message::Frame(now) => {
                if let Some(ref clock) = self.playhead {
                    self.display_progress = clock.display(now);
                }
                self.panel_progress = self.effects_panel.tick(now);
                Task::none()
            }
```

- [ ] **Step 8: Close the panel on Escape (priority after modal overlays)**

In the `EscapePressed` arm (~line 654), insert a branch after the editor branch and before the `search_had_focus` branch:

```rust
                } else if self.effects_panel.is_open() {
                    // Effects drawer closes before search-state handling.
                    let now = Instant::now();
                    self.effects_panel.close(now);
                    self.panel_progress = self.effects_panel.progress(now);
                } else if self.search_had_focus {
```

- [ ] **Step 9: Extend the frame subscription gate**

In `subscription` (~line 1291), replace:

```rust
        if self.playing.is_some() {
            subs.push(iced::window::frames().map(Message::Frame));
        }
```

with:

```rust
        if self.playing.is_some() || self.effects_panel.is_animating() {
            subs.push(iced::window::frames().map(Message::Frame));
        }
```

- [ ] **Step 10: Move the effects panel out of the base column and into an overlay layer**

In `view_main`, DELETE the inline construction (~line 1367):

```rust
        let effects = effects_panel_view::view_effects_panel(&self.effects_ui, t);
```

and REMOVE `effects,` from the `items` vec (~line 1383) so it reads:

```rust
        let items: Vec<Element<'_, Message>> = vec![
            top.into(),
            chips,
            scrollable(grid).height(Length::Fill).into(),
            now_playing,
        ];
```

Then, immediately after `let mut layers: Vec<Element<'_, Message>> = vec![base.into()];` (~line 1405) and BEFORE the context-menu `if`, push the drawer layer (always present — the tab shows even when closed; the scrim mounts only when open):

```rust
        // Effects side panel (#143): pull tab always visible; scrim + body slide
        // in when open. Pushed below the context-menu/editor modals so those stack
        // on top. All drawer logic lives in `ui::side_panel`.
        let effects_body = effects_panel_view::view_effects_panel(&self.effects_ui, t);
        let effects_cfg = SidePanelConfig {
            panel_w: 400.0,
            tab_w: 28.0,
            title: "Voice Effects",
            on_toggle: Message::ToggleEffectsPanel,
            on_close: Message::CloseEffectsPanel,
        };
        layers.push(side_panel::view_side_panel(
            effects_cfg,
            self.panel_progress,
            self.window_size,
            effects_body,
            t,
        ));
```

- [ ] **Step 11: Run the app tests**

Run: `cargo test --lib app:: 2>&1 | tail -25`
Expected: the four new tests pass; all pre-existing app tests still pass (no regression from removing the inline effects element).

- [ ] **Step 12: Full build, fmt, clippy, whole suite**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings 2>&1 | tail -25 && cargo test 2>&1 | tail -25`
Expected: no clippy warnings; entire test suite green. Confirm the Task-3 `dead_code` re-exports are now consumed (no warning).

- [ ] **Step 13: Commit**

```bash
git add src/app.rs
git commit -m "feat(ui): on-demand effects side panel — pull tab, slide, scrim (#143)

Replaces the always-visible inline effects panel with a right-edge drawer.
Wall-clock PanelAnim (the pattern that beat the #139 playhead jitter) drives
the slide; the frame subscription ticks only while animating. All logic lives
in ui::side_panel; app.rs carries thin glue only.

Closes #143

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Manual verification (the #139 lesson — human-gated)

Pure-logic tests cannot prove the Iced layout behaves. After Task 5, build and run, then verify by hand before opening the PR.

- [ ] **Step 1: Build and run**

Run: `cargo run 2>&1 | tail -5`

- [ ] **Step 2: Verify the interaction checklist**

- [ ] Closed state: the pull tab is visible on the right edge; the **grid is fully clickable** (sounds play, right-click edit works) — the closed drawer layer must NOT swallow input.
- [ ] Click the tab → the panel slides in smoothly over a dimmed grid; the chevron flips.
- [ ] Effect controls work inside the drawer (preset select, Mix slider, sliders, ON/OFF).
- [ ] Click the scrim, the ✕, or press Escape → the panel slides out; the tab returns.
- [ ] Toggle rapidly mid-slide → reverses smoothly, no snap/jump.
- [ ] On a software-renderer run (`HONKHONK_RENDERER=software cargo run`) the slide still works.

If the closed drawer swallows grid input, the fix is in `view_side_panel`: ensure the scrim is omitted at `progress <= 0.0` and that the closed `anchored` container forwards events (it does by construction) — do not add a backdrop when closed.

- [ ] **Step 3: Record the result** in the PR description's Testing checklist.

---

## Self-Review (completed during planning)

**1. Spec coverage:**
- On-demand drawer / hidden-by-default → Tasks 3, 5 (overlay layer, tab toggle). ✓
- Pull-tab trigger → Task 3 `tab()`, Task 5 `on_toggle`. ✓
- Slide over dimmed grid → Task 3 `float` + scrim. ✓
- Reusable framework (DRY) → `side_panel/` module, generic `SidePanelConfig`. ✓
- #144 geometry hook → Task 2 `panel_geometry`/`PanelRect`. ✓
- Wall-clock animation (#139 class avoided) → Task 1 `PanelAnim`. ✓
- app.rs net-minimal, no business logic → Task 5 thin glue; logic in module. ✓
- Effects controls unchanged, relocated → Task 4 (presets only) + Task 5 (body reused). ✓
- Escape / scrim / ✕ dismiss → Task 5 Step 8, Task 3 scrim/close. ✓
- Out of scope (feather puff, goose-wing, other panels) → not in any task. ✓

**2. Placeholder scan:** none — every code step carries complete code.

**3. Type consistency:** `PanelAnim`, `progress`/`tick`/`toggle`/`close`/`is_open`/`is_animating`, `SLIDE_DURATION`, `panel_geometry`/`PanelRect`, `view_side_panel`/`SidePanelConfig` (`panel_w`,`tab_w`,`title`,`on_toggle`,`on_close`), `Message::{ToggleEffectsPanel, CloseEffectsPanel}`, fields `effects_panel`/`panel_progress` — names match across Tasks 1, 3, 5. ✓
```
