# Panel Feather Edge Emitter Design

## Context

PR #176 added a decorative feather flourish when the effects side panel opens or
closes. The current model computes one `BurstOrigin` per panel and seeds all
particles near that point. For the right-docked effects panel, that point is the
middle of the panel's inner edge.

Current feather rendering has no semantic size variants. `FeatherParticle` stores
a numeric `size`, currently cycling through four values, and
`flourish_view.rs` draws every particle with the same stroked spine-and-barb
routine. This iteration does not change feather drawing, particle count, colors,
or size semantics.

## Goal

Make docked panels emit feathers along the full length of the panel edge instead
of from one center point, while preserving the current look and animation feel.
The distribution should read as pseudo-random but remain evenly spread and
deterministic for tests.

## Non-Goals

- Do not introduce small/medium/large feather variants.
- Do not add pixel-art feather sprites.
- Do not change the settings toggle, duration, fade, gravity, cursor bump, or
  panel open/close lifecycle.
- Do not add unrelated side-panel abstractions beyond what the emitter model
  needs.

## API Shape

Replace the point-only burst source with an explicit emitter shape:

```rust
pub enum BurstEmitter {
    Edge(BurstLine),
    Center(Point),
}

pub struct BurstLine {
    pub start: Point,
    pub end: Point,
    pub direction: Vector,
}
```

`BurstLine::direction` is the normalized outward direction particles should move
away from the panel. For the existing right effects panel:

```text
start = (panel.x, panel.y)
end = (panel.x, panel.y + panel.h)
direction = (-1, 0)
```

Floating panels continue to emit from `Center(panel.center)`.

## Geometry

`panel_burst_origin` will become a line-aware helper, likely named
`panel_burst_emitter`, that maps a `PanelRect` plus window size to a
`BurstEmitter`.

Docked panels map to their full inner edge:

- Right-docked panel: left edge, direction `(-1, 0)`.
- Left-docked panel: right edge, direction `(1, 0)`.
- Bottom-docked panel: top edge, direction `(0, -1)`.
- Top-docked panel: bottom edge, direction `(0, 1)`.

If a panel is floating or fills the window in a way that makes a docked inner edge
ambiguous, keep the existing center-emission behavior.

## Particle Seeding

`PanelFlourish::emit` will ask for a `BurstEmitter` and seed particles from it.
For `Center`, behavior remains equivalent to today.

For `Edge`, each particle receives a stratified position along the line:

```text
slot_t = (i + 0.5) / PARTICLES
jitter_t = deterministic small offset inside the slot
t = clamp(slot_t + jitter_t, slot_min, slot_max)
position = lerp(line.start, line.end, t) + outward_offset + side_jitter
```

The stratification gives even coverage across the whole edge. The deterministic
jitter prevents the particles from looking mechanically spaced. Velocity keeps
the current outward direction, scatter, speed variation, close-transition
rotation bias, gravity, and cursor interaction.

## Data Flow

The existing app flow remains unchanged:

```text
ToggleEffectsPanel / CloseEffectsPanel
  -> app/panels.rs computes panel_geometry(...)
  -> PanelFlourish::emit(panel, window, transition, now)
  -> side_panel/flourish.rs computes BurstEmitter
  -> side_panel/flourish_view.rs renders current particles
```

Only `ui::side_panel` tests and pure animation code should need meaningful
changes. App glue should remain thin.

## Error Handling

The code path remains pure and non-fallible. Degenerate panel/window geometry
must stay guarded: no NaN positions, no division by zero, and empty or zero-length
edge lines fall back to the center point or a safe normalized direction.

## Testing

Extend `tests/side_panel_flourish.rs` to validate the public behavior:

- Right-docked panels report a full inner-edge `BurstLine`.
- Left/top/bottom docked panels report their matching full inner edges and
  outward directions.
- Floating panels still report center emission.
- Edge particles still move away from the panel.
- Edge particles span most of the panel edge length instead of clustering near
  the edge center.
- Existing fade, gravity, cursor bump, and repeated-close behavior keep passing.

Run at minimum:

```bash
cargo test --test side_panel_flourish
cargo test app::panels::tests
```

Before a PR, also run the usual Rust verification for the touched surface:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Implementation Notes

Keep `src/ui/side_panel/flourish.rs` under the repo's 400-line file limit. If
the emitter model makes that file crowded, split geometry-specific emitter logic
into a small sibling module rather than adding to `app.rs`.
