# Panel Feather Motion Implementation Plan

> Status: implemented on `feat/issue-183-smooth-feather-pipeline`.

## Goal

Make the panel feather flourish feel like dust, small feather chunks, and full
feathers moving through air resistance while preserving the existing open-only
panel lifecycle.

The work stacks on `feat/panel-feather-edge-emitter` and keeps Issue #183
limited to the side-panel flourish model, renderer, tests, and design docs.

## Requirements

- Add explicit `FeatherClass` values: `Dust`, `Chunk`, and `Feather`.
- Seed every burst deterministically with all three classes.
- Keep the initial puff as an initial velocity impulse.
- Apply gravity immediately from the first tick.
- Make dust descend fastest with minimal swoop.
- Make chunks descend and drift at a medium rate.
- Make full feathers descend slowest with the strongest left/right swoop.
- Keep `BURST_DURATION` as the hard cleanup boundary for every class.
- Do not let particle class control lifetime.
- Preserve app-level open-only emission: closing a panel clears the flourish and
  does not emit a close burst.
- Keep frame subscriptions active only while existing animation sources need
  frames.
- Avoid a general physics engine or expensive per-frame work.
- Do not change `src/app.rs`, panel geometry, settings, or unrelated UI flows.

## Files

- `src/ui/side_panel/flourish.rs`
  - Owns `FeatherClass`, seeded class parameters, cheap motion updates, fade, and
    cleanup.
- `src/ui/side_panel/flourish_view.rs`
  - Draws class-specific particles using renderer-neutral Iced canvas
    primitives.
- `src/ui/side_panel/mod.rs`
  - Re-exports `FeatherClass` with the existing side-panel flourish API.
- `tests/side_panel_flourish.rs`
  - Covers class mix, class motion differences, seeded wobble, visual size
    differences, and shared cleanup.
- `src/app/panels.rs`
  - Used only for lifecycle verification. No implementation change was required.
- `docs/superpowers/specs/2026-06-27-panel-feather-motion-design.md`
  - Captures the design rationale and performance guardrails.

## Implementation Tasks

### 1. Introduce Feather Classes

Add `FeatherClass` and attach it to each `FeatherParticle`. Seed a deterministic
mix that includes dust, chunks, and full feathers in every burst.

Validation:

```bash
cargo test --test side_panel_flourish burst_seeds_all_feather_classes
```

Commit:

```text
dd2eef0 feat(ui): classify panel feather particles
```

### 2. Add Cheap Air-Resistance Motion

Pre-seed class-specific size, impulse, drag, wobble, and rotation parameters.
Update `PanelFlourish::tick` to apply gravity, damping, lateral wobble, position,
rotation, and fade without allocations in the hot path.

Validation:

```bash
cargo test --test side_panel_flourish dust_descends_farther_than_full_feather
cargo test --test side_panel_flourish full_feathers_swoop_more_than_dust
cargo test --test side_panel_flourish same_class_feathers_diverge_from_seeded_wobble_phase
cargo test --test side_panel_flourish
```

Commits:

```text
e02d920 feat(ui): add feather air-resistance motion
53d46cb test(ui): pin panel feather wobble
```

### 3. Split Rendering By Particle Class

Render dust as tiny pale dots, chunks as small fragments, and full feathers as
the recognizable spine-and-barb shape. Keep the renderer path in Iced canvas
primitives so future sprite work can replace only the drawing functions.

Validation:

```bash
cargo test --test side_panel_flourish feather_classes_have_distinct_visual_sizes
cargo test --test side_panel_flourish
cargo test ui::side_panel::view::tests::builds_across_progress
```

Commit:

```text
0ec78cf feat(ui): render panel feather particle classes
```

### 4. Preserve Cleanup And Frame Lifecycle

Pin the shared cleanup contract across all classes and verify the app-level close
behavior remains open-only.

Validation:

```bash
cargo test --test side_panel_flourish all_classes_clear_at_shared_burst_duration
cargo test app::panels::tests
cargo test app::panels::tests::closing_open_panel_does_not_emit_flourish
cargo test app::panels::tests::close_flourish_does_not_extend_frame_subscription_after_slide
```

Commit:

```text
a1f9fcc test(ui): pin panel feather cleanup lifecycle
```

## Final Verification

Run before opening the stacked draft PR:

```bash
cargo fmt --check
cargo test --test side_panel_flourish
cargo test app::panels::tests
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
wc -l src/ui/side_panel/flourish.rs src/ui/side_panel/flourish_view.rs tests/side_panel_flourish.rs
git diff --check
git status --short --branch
```

Recorded results:

- `cargo fmt --check`: passed.
- `cargo test --test side_panel_flourish`: passed, 13 tests.
- `cargo test app::panels::tests`: passed, 5 tests.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test`: passed.
- `cargo build --release`: passed.
- File sizes: `flourish.rs` 368 lines, `flourish_view.rs` 168 lines,
  `side_panel_flourish.rs` 346 lines.
- `git diff --check`: passed.
- Worktree was clean after verification.

## PR Prep

Open the branch as a draft stacked PR:

```bash
git push -u origin feat/issue-183-smooth-feather-pipeline
gh pr create --draft \
  --repo wrzonance/HonkHonk \
  --base feat/panel-feather-edge-emitter \
  --head feat/issue-183-smooth-feather-pipeline \
  --title "[codex] smooth panel feather motion" \
  --body "..."
```

The PR should close Issue #183.
