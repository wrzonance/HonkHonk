# Panel Feather Edge Emitter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make docked panel feather flourishes emit from the full panel edge with deterministic pseudo-random spread, while keeping the current feather rendering unchanged.

**Architecture:** Replace the single-point `BurstOrigin` model with `BurstEmitter`, which can be either a full `BurstLine` edge or a center point. `PanelFlourish::emit` keeps its current app-facing signature, computes the emitter internally, and seeds edge particles from stratified points along the line. Rendering in `flourish_view.rs` remains untouched.

**Tech Stack:** Rust 2024, Iced geometry types (`Point`, `Vector`), existing `ui::side_panel` module, cargo test/clippy.

---

## Companion Snippets

Exact code snippets are split into
`docs/superpowers/plans/2026-06-26-panel-feather-edge-emitter-snippets.md`
so each plan artifact stays under HonkHonk's 400-line file limit.

Use the snippet headings named in each task. Do not edit
`src/ui/side_panel/flourish_view.rs`; feather drawing is out of scope.

---

## File Structure

- Modify `tests/side_panel_flourish.rs`: replace point-origin tests with emitter geometry and edge-distribution tests.
- Modify `src/ui/side_panel/flourish.rs`: define `BurstLine` and `BurstEmitter`, replace `panel_burst_origin` with `panel_burst_emitter`, and seed edge particles along full line emitters.
- Modify `src/ui/side_panel/mod.rs`: re-export `BurstEmitter`, `BurstLine`, and `panel_burst_emitter`.
- Leave `src/app/panels.rs` unchanged unless compilation reveals only a rename import issue; app glue should still call `PanelFlourish::emit(panel, window, transition, now)`.

---

### Task 1: Specify The Line Emitter Behavior In Tests

**Files:**
- Modify: `tests/side_panel_flourish.rs`
- Read: `docs/superpowers/plans/2026-06-26-panel-feather-edge-emitter-snippets.md`

- [ ] **Step 1: Replace the flourish integration tests**

Replace `tests/side_panel_flourish.rs` with the complete snippet named
`Replacement tests/side_panel_flourish.rs`.

- [ ] **Step 2: Run the tests and verify the missing API failure**

Run:

```bash
cargo test --test side_panel_flourish
```

Expected: FAIL with unresolved imports for `BurstEmitter`, `BurstLine`, and
`panel_burst_emitter`.

- [ ] **Step 3: Keep the failing tests uncommitted**

Run:

```bash
git status --short
```

Expected: `tests/side_panel_flourish.rs` is modified and uncommitted. Do not
commit this failing state; Task 2 commits the tests with the first passing
implementation.

---

### Task 2: Add The Explicit Emitter Geometry API

**Files:**
- Modify: `src/ui/side_panel/flourish.rs`
- Modify: `src/ui/side_panel/mod.rs`
- Modify: `tests/side_panel_flourish.rs`
- Read: `docs/superpowers/plans/2026-06-26-panel-feather-edge-emitter-snippets.md`

- [ ] **Step 1: Replace the burst geometry types**

In `src/ui/side_panel/flourish.rs`, replace the current `BurstSource` and
`BurstOrigin` definitions with the snippet named `Emitter type definitions`.

- [ ] **Step 2: Replace the panel geometry helper**

In `src/ui/side_panel/flourish.rs`, replace the full `panel_burst_origin`
function with the snippet named `panel_burst_emitter`.

- [ ] **Step 3: Replace the edge helper**

In `src/ui/side_panel/flourish.rs`, replace the old `edge` helper with the
snippet named `edge_line helper`.

- [ ] **Step 4: Update `PanelFlourish::emit`**

Replace:

```rust
let origin = panel_burst_origin(panel, window);
self.particles = seed_particles(origin, transition);
```

with:

```rust
let emitter = panel_burst_emitter(panel, window);
self.particles = seed_particles(emitter, transition);
```

- [ ] **Step 5: Update seed signatures and midpoint edge behavior**

In `src/ui/side_panel/flourish.rs`, replace the existing `seed_particles`,
`seed_particle`, and `particle_direction` blocks using snippets named:

- `seed_particles and seed_particle signatures`
- `particle_direction and midpoint emitter_point`

Also replace:

```rust
position: translate(origin.point, offset),
```

with:

```rust
position: translate(emitter_point(emitter), offset),
```

- [ ] **Step 6: Update public re-exports**

In `src/ui/side_panel/mod.rs`, replace the current `pub use flourish::{...};`
block with the snippet named `mod.rs re-export`.

- [ ] **Step 7: Run the geometry-focused tests**

Run:

```bash
cargo test --test side_panel_flourish right_docked_panel_emits_from_full_inner_edge_away_from_panel
cargo test --test side_panel_flourish all_docked_panel_edges_emit_from_full_inner_edge
cargo test --test side_panel_flourish floating_panel_emits_from_center
```

Expected: each command passes. The distribution test may still fail until Task 3.

- [ ] **Step 8: Commit the emitter API**

```bash
git add tests/side_panel_flourish.rs src/ui/side_panel/flourish.rs src/ui/side_panel/mod.rs
git commit -m "feat(ui): model feather bursts as edge emitters"
```

---

### Task 3: Seed Edge Particles Along The Full Emitter Line

**Files:**
- Modify: `src/ui/side_panel/flourish.rs`
- Test: `tests/side_panel_flourish.rs`
- Read: `docs/superpowers/plans/2026-06-26-panel-feather-edge-emitter-snippets.md`

- [ ] **Step 1: Confirm the distribution test fails before implementation**

Run:

```bash
cargo test --test side_panel_flourish edge_particles_span_most_of_the_panel_edge
```

Expected: FAIL because Task 2 still places edge particles at the line midpoint.

- [ ] **Step 2: Replace midpoint sampling with stratified line sampling**

In `src/ui/side_panel/flourish.rs`, replace `emitter_point` with the snippet named
`stratified emitter_point`.

Then replace:

```rust
position: translate(emitter_point(emitter), offset),
```

with:

```rust
position: translate(emitter_point(emitter, i), offset),
```

- [ ] **Step 3: Run the full flourish test suite**

Run:

```bash
cargo test --test side_panel_flourish
```

Expected: PASS, including `edge_particles_span_most_of_the_panel_edge`.

- [ ] **Step 4: Run the app panel tests**

Run:

```bash
cargo test app::panels::tests
```

Expected: PASS for the two app panel tests.

- [ ] **Step 5: Commit the distribution change**

```bash
git add src/ui/side_panel/flourish.rs
git commit -m "feat(ui): spread feather bursts along panel edge"
```

---

### Task 4: Final Verification

**Files:**
- Verify: `src/ui/side_panel/flourish.rs`
- Verify: `src/ui/side_panel/mod.rs`
- Verify: `tests/side_panel_flourish.rs`
- Verify: `src/ui/side_panel/flourish_view.rs`

- [ ] **Step 1: Format the Rust code**

Run:

```bash
cargo fmt
```

Expected: command exits successfully.

- [ ] **Step 2: Check line counts stay under the repo cap**

Run:

```bash
wc -l src/ui/side_panel/flourish.rs src/ui/side_panel/mod.rs tests/side_panel_flourish.rs
```

Expected: each listed file is under 400 lines.

- [ ] **Step 3: Run targeted tests**

Run:

```bash
cargo test --test side_panel_flourish
cargo test app::panels::tests
```

Expected: all selected tests pass.

- [ ] **Step 4: Run clippy for all targets**

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: command exits successfully with no warnings.

- [ ] **Step 5: Confirm rendering code was not changed**

Run:

```bash
git diff origin/main -- src/ui/side_panel/flourish_view.rs
```

Expected: no output. If there is output, remove only the unintended
`flourish_view.rs` change before committing because feather drawing is out of
scope for this iteration.

- [ ] **Step 6: Commit final formatting if needed**

Run:

```bash
git status --short
```

If formatting changed tracked files, run:

```bash
git add src/ui/side_panel/flourish.rs src/ui/side_panel/mod.rs tests/side_panel_flourish.rs
git commit -m "style(ui): format panel feather emitter"
```

Expected: commit is created only when `git status --short` shows formatting changes.

---

## Completion Checklist

- [ ] `BurstEmitter` and `BurstLine` are exported from `honkhonk::ui::side_panel`.
- [ ] `panel_burst_emitter` reports full inner-edge lines for docked panels.
- [ ] Floating panels still emit from center.
- [ ] Edge particle positions span most of the panel edge.
- [ ] Feather rendering remains unchanged.
- [ ] Targeted tests pass.
- [ ] `cargo clippy --all-targets -- -D warnings` passes.
