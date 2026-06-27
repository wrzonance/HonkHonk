# Panel Feather Motion Design

## Context

Issue #183 follows the panel feather edge emitter work in PR #182. The current
flourish emits particles evenly along the panel edge, but its motion is still a
simple velocity plus gravity update. Particle `size` only affects drawing scale;
it does not change how different feather materials move.

The next iteration should make the flourish feel more like real feathers moving
through air while keeping HonkHonk's frame pipeline cheap and predictable. This
is still a soundboard app, not a flight simulation.

## Goal

Replace the generic gravity-only feel with deterministic feather motion:

- A visible initial puff away from the panel edge.
- Gravity affects particles from the first frame.
- Air resistance and lateral wobble make larger feathers descend slowly and
  swoop left/right.
- Smaller dust/chunks fall faster and move less dramatically.
- The shared fade window remains the hard cleanup boundary.

## Non-Goals

- Do not change the edge distribution from PR #182.
- Do not emit feathers on close; open-only emission remains current behavior.
- Do not add bitmap or pixel-art feather assets.
- Do not make particle class control lifetime.
- Do not introduce a general physics engine or expensive per-frame simulation.
- Do not change settings, panel geometry, or unrelated animation behavior.

## Particle Classes

Introduce explicit particle classes instead of deriving semantics from a raw
numeric size:

```rust
pub enum FeatherClass {
    Dust,
    Chunk,
    Feather,
}
```

Each `FeatherParticle` keeps its class plus pre-seeded motion parameters. The
class gives later rendering and asset work a stable hook:

- `Dust`: tiny flecks or short dashes, fastest descent, minimal swoop.
- `Chunk`: small white/gray blobs or barb fragments, medium descent and drift.
- `Feather`: recognizable feather shape, slowest descent, strongest lateral
  swoop and rotation.

The seeded class mix should be deterministic and include all three classes in
each burst. A simple fixed pattern is enough, for example more dust/chunks than
large feathers so the flourish reads as a puff with a few hero feathers.

## Motion Model

Keep the model force-inspired but computationally cheap. On each tick:

1. Apply gravity immediately to vertical velocity.
2. Apply class-specific drag/damping with multiplication factors.
3. Add deterministic lateral air wobble from a per-particle phase.
4. Advance position from velocity.
5. Advance rotation and fade using the existing shared duration.

The initial puff remains an initial velocity impulse, not a period where gravity
is suspended. If the puff needs to read more strongly, increase the initial
outward velocity rather than delaying gravity.

Suggested class behavior:

```text
Dust:    lower drag, higher downward speed, low wobble amplitude
Chunk:   medium drag, medium downward speed, medium wobble
Feather: higher drag/lift, slow downward speed, stronger low-frequency wobble
```

The path should read as roughly parabolic early, then air resistance bends it
into a slower floating descent. Larger feathers may still be mid-air when the
flourish clears; that is acceptable because the fade window is the lifecycle
authority.

## Performance Guardrails

Keep tick work bounded and allocation-free:

- Keep particle count near the current 18.
- No allocation during `PanelFlourish::tick`.
- No per-frame normalization for the base motion model.
- Avoid per-frame division in the hot path; precompute reciprocal-like factors
  or use constants at spawn time where useful.
- Avoid square roots except the existing cursor bump path, which already needs
  distance and only runs when cursor interaction is active.
- One `sin`-like wobble per particle per frame is acceptable at this particle
  count, but the phase and frequency should be seeded once per particle.

If later profiling shows the sine call matters, swap the wobble for a small
deterministic lookup table without changing the public model.

## Rendering

Stay with renderer-neutral canvas drawing for this iteration:

- `Dust`: a tiny pale dot or short dash, only a few pixels.
- `Chunk`: a small white/gray blob or short barb fragment.
- `Feather`: the current spine-and-barb shape, potentially with slightly
  stronger silhouette.

Do not add image assets yet. Future sprite or pixel-art work can replace the
class-specific draw functions while keeping the same `FeatherClass` and motion
model.

## Lifecycle And Frame Subscription

`BURST_DURATION` remains the hard cleanup boundary for every class. Particle
class changes motion only, not lifetime.

The app should continue subscribing to frames only while one of the existing
animation sources needs frames:

```text
playing sound || panel slide animating || panel flourish animating
```

Closing the effects panel must continue to clear any in-flight flourish and must
not emit a close burst. Hidden/tray state and disabled panel animations still
clear or skip the flourish as they do today.

## Integration Scope

Keep this contained to the existing panel flourish surface:

- `src/ui/side_panel/flourish.rs`: particle class, spawn parameters, and cheap
  force-style tick update.
- `src/ui/side_panel/flourish_view.rs`: class-specific drawing.
- `tests/side_panel_flourish.rs`: pure behavior coverage.
- `src/app/panels.rs`: only if lifecycle/frame-subscription tests expose a gap.

No changes to `src/app.rs`, settings, panel geometry, edge distribution, or new
assets are part of this iteration.

The eventual PR should be stacked on `feat/panel-feather-edge-emitter`, not
`main`.

## Testing

Use pure model tests rather than screenshot tests:

- Burst seeding includes `Dust`, `Chunk`, and `Feather`.
- Over the same time window, dust descends farther than feather.
- Feather has stronger lateral displacement than dust.
- All classes fade together and clear at `BURST_DURATION`.
- Close transitions still do not emit feathers.
- Frame subscription stops after the slide/flourish lifecycle settles.

Run at minimum:

```bash
cargo test --test side_panel_flourish
cargo test app::panels::tests
```

Before opening the stacked PR, also run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

`cargo build --release` is part of this verification because the sluggishness
report came from a release binary.

## Spec Review Notes

- No unresolved design markers remain.
- Scope is intentionally limited to motion and drawing primitives.
- The hard fade window resolves the potential ambiguity between slow-floating
  large feathers and frame-subscription cleanup.
- The performance constraints explicitly avoid simulation work that would be
  disproportionate for a decorative soundboard flourish.
