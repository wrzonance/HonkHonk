# Effects Side Panel + Reusable Panel Framework — Design (#143)

**Status:** Approved 2026-06-23. Supersedes the always-visible effects panel landed in #135.

## Goal

Change the visual language of the voice-effects panel from an always-present
inline block into an **on-demand right-edge drawer** that slides over the
(dimmed) sound grid and hides when dismissed. Abstract the drawer mechanics as a
**reusable side-panel framework** so future settings panels reuse it, and land
the panel-geometry hook that the feather-puff animation (#144) will consume.

## Motivation

User feedback on #135: the effects panel should be hidden by default, not always
on screen with an on/off button. This mirrors a common UI split:

- **Style A** — global, infrequently-changed settings live behind a menu
  (HonkHonk already has this: the `Settings` view).
- **Style B** — frequently-adjusted controls that shouldn't disrupt the main
  view live in panels that open/collapse on demand.

Voice effects are Style B: opened to tweak, then collapsed. A right-edge drawer
over the grid is the natural form.

## Scope

**In scope**
- Effects panel becomes a right-edge drawer (slide in/out, scrim, dismiss).
- A persistent **pull tab** on the right edge toggles it.
- Reusable `side_panel` framework (animation state, geometry, view).
- Panel geometry (edges + center) exposed as the hook for #144.
- Wall-clock slide animation (~150 ms), frame-driven.

**Out of scope**
- Feather-puff burst animation (#144) — only its geometry hook lands here.
- Goose-wing animated chevron (future #144 flavor) — tab ships a static chevron.
- Migrating other panels (Settings, slot manager) to the framework — the
  framework is built ready for them, but no migration this PR.
- Any change to the audio effects DSP, presets, or the effect chain wiring.

## Architecture

### Display model — Stack overlay (existing pattern)

`view_main` already composes a root `iced::widget::Stack`: child 0 is the base
layout (header + chips + grid + now-playing), and overlays (context menu, sound
editor) append as later children. This positional-stability trick preserves the
grid scrollable's offset across diffs (#112). The effects drawer follows the
**same** pattern: it is appended as overlay layer(s) on top of the base, never
inserted into the base column. The base column **loses** its current inline
effects element.

### Animation model — wall-clock (the pattern that beat the #139 jitter)

Predict-and-correct interpolation produced visible jitter on the playhead (#139);
pure wall-clock won. The panel reuses that lesson:

```
PanelAnim state machine:
    Closed
    Opening(start: Instant)
    Open
    Closing(start: Instant)

progress(now) -> f32   // eased 0.0 (closed) .. 1.0 (open)
toggle(now)            // Closed/Closing -> Opening ; Open/Opening -> Closing
close(now)             // -> Closing (no-op if already closed/closing)
is_animating() -> bool // true while Opening/Closing
is_open() -> bool      // anything other than fully Closed (tab still shows)
```

`progress` is `ease(elapsed / DURATION)` clamped to `[0, 1]`; on the frame where
`elapsed >= DURATION`, the state settles to `Open` / `Closed`. Easing is a smooth
ease-in-out (`smoothstep`) so the slide decelerates naturally. No dt accumulation,
no re-anchoring — the same drift-free property the playhead now has.

Toggling mid-animation reverses cleanly: a reversal seeds the new `start` so the
panel continues from its **current** visible progress rather than snapping to an
endpoint (compute the equivalent elapsed for the current progress in the new
direction).

### Slide mechanism — `Pin` (stock Iced 0.14)

Iced 0.14 has no general-element opacity wrapper (only image/svg), but it has
`pin` (absolute x/y positioning) and clips to the window surface. The drawer is a
`row![tab_handle, panel_body]` placed with `pin(drawer).x(drawer_x).y(0)`, where:

```
drawer_x = lerp(closed_x, open_x, progress)
closed_x = window_w - TAB_W              // only the tab pokes out at the edge
open_x   = window_w - TAB_W - PANEL_W    // full body visible, tab at its left
```

The scrim is a full-window `mouse_area` whose background alpha = `progress *
SCRIM_MAX`. The scrim layer is rendered **only while `progress > 0`** so a fully
closed panel never intercepts grid clicks. The pull tab is part of the drawer, so
it travels with the panel and is always reachable on the right edge.

**Key implementation risk (manual-gated):** a full-window pinned layer must not
swallow input to the grid beneath when the panel is closed. Verified by manual
interaction check (the #139 lesson) and by only mounting the scrim when open.

### Module structure — `src/ui/side_panel/` (DRY framework)

Split to honor the 400-line file cap and high cohesion:

| File | Responsibility | Tested |
|---|---|---|
| `side_panel/anim.rs` | `PanelAnim` state machine + eased `progress` | unit (pure) |
| `side_panel/geometry.rs` | `panel_geometry(window, panel_w) -> PanelRect` | unit (pure) |
| `side_panel/view.rs` | `view_side_panel(cfg, progress, window, body)` → scrim + pinned drawer | build-smoke |
| `side_panel/mod.rs` | re-exports; `SidePanelConfig`, `PanelRect` types | — |

`PanelRect { x, y, w, h, center: Point }` describes the panel body at full open.
`geometry` is the **#144 hook**: the feather puff reads `rect.center` and the four
edges to know where to burst feathers from. `geometry` clamps for degenerate
window sizes (never negative width, never NaN center).

`SidePanelConfig` carries the static knobs + messages the framework needs from a
consumer:

```
SidePanelConfig {
    panel_w: f32,
    tab_w: f32,
    title: &'static str,
    tab_glyph: &'static str,   // static chevron for now
    on_toggle: Message,        // tab press
    on_close: Message,         // scrim press / Escape
}
```

### Effects panel as first consumer

`effects_panel_view::view_effects_panel` keeps producing the **body** (master
row, preset chips, parameter sliders) unchanged. `side_panel::view_side_panel`
wraps that body with the drawer chrome (tab, slide position, scrim, title bar with
a ✕). The body's own outer styled container is retained or lightly adjusted so it
reads as a drawer rather than an inline card — a visual nicety, not a behavior
change.

### app.rs impact — net-neutral or smaller

All new *logic* lives in `side_panel/`. `app.rs` (a known 2.7k-line violation that
must not grow) gains only thin glue and **loses** the inline effects element:

- Add fields: `effects_panel: PanelAnim`, `panel_progress: f32`.
- Add messages: `ToggleEffectsPanel`, `CloseEffectsPanel`.
- `update`: small arms for the two messages; `Frame` also refreshes
  `panel_progress = effects_panel.progress(now)` and settles the state; `Escape`
  closes the panel when open (before other Escape behavior).
- `subscription`: frame gate becomes `playing.is_some() ||
  effects_panel.is_animating()`.
- `view_main`: remove `effects` from the base `items` vec and its construction;
  append the side-panel overlay layer(s) when `panel_progress > 0` (scrim+body)
  and always render the tab.

**Constraint:** no new *business logic* in `app.rs` — all of it lives in
`side_panel/`. The glue added is thin and kept minimal; removing the inline
block offsets much of it. A small net increase of logic-free wiring is
acceptable; piling logic into the god file is not.

## Data flow

```
tab press ─► Message::ToggleEffectsPanel ─► effects_panel.toggle(now)
                                            └► is_animating()==true
window::frames() ─► Message::Frame(now) ─► panel_progress = progress(now)
                                          └► settle state when elapsed>=DURATION
view_main(panel_progress) ─► pin(drawer).x(lerp(..., panel_progress))
                              + scrim(alpha = panel_progress*SCRIM_MAX)
scrim press / Escape ─► Message::CloseEffectsPanel ─► effects_panel.close(now)
```

## Error handling & validation

Pure UI state — no IO, no fallible boundaries. Guards: `progress` clamps to
`[0,1]`; `geometry` clamps window/panel dimensions so width is never negative and
center is finite; `DURATION` is a non-zero constant (no div-by-zero).

## Testing

Per CLAUDE.md, Iced view rendering is **not** unit-tested; pure logic is, to the
80% target.

- **`PanelAnim`** (unit): starts closed (progress 0); toggle → progress rises to
  1 across DURATION; toggle again → falls to 0; `is_animating` true only mid-slide;
  progress monotonic per direction and clamped `[0,1]`; mid-animation reversal is
  continuous (no snap); `close` idempotent.
- **`geometry`** (unit): `PanelRect` x/width/center for a normal window; center is
  the right-panel center; a tiny/degenerate window clamps without panic.
- **app.rs** (unit, mirroring existing tests): `ToggleEffectsPanel` flips state;
  `CloseEffectsPanel` closes; `EscapePressed` closes the panel when open; the
  frame-gate predicate includes `is_animating`.
- **Views** (build-smoke only): `view_side_panel` builds at progress 0, 0.5, 1;
  existing `effects_panel_view` smoke test still passes.
- **Manual gate:** slide is smooth at refresh rate; grid stays fully clickable
  when the panel is closed; tab toggles; scrim and Escape dismiss.

## Constraints (carried into the plan as Global Constraints)

- Files ≤ 400 lines; functions ≤ 50 lines; clippy `-D warnings` with
  cognitive-complexity 10 / too-many-arguments 5 / too-many-lines 50.
- No `.unwrap()`/`panic!()` in non-test code; error context chains where relevant.
- No new business logic in `app.rs`; added wiring is thin glue, kept minimal.
- Wall-clock animation only — no predict-and-correct (the #139 regression class).
- Stock Iced 0.14 widgets only; drawer via `pin` + `mouse_area` + `stack`.
- `cargo fmt` clean and `cargo clippy -- -D warnings` clean (CI gates both).
```
