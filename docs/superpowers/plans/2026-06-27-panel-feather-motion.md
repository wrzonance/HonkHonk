# Panel Feather Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make panel feather particles move like dust, feather chunks, and full feathers falling through air resistance while preserving the existing open-only flourish lifecycle.

**Architecture:** Keep the current `PanelFlourish` ownership model and edge emitter from PR #182. Add a public `FeatherClass` plus pre-seeded, per-particle motion parameters in `flourish.rs`; use cheap per-frame damping and deterministic lateral wobble in `tick`; draw each class differently in `flourish_view.rs`.

**Tech Stack:** Rust 2024, Iced 0.14 canvas, existing `PanelFlourish` model tests in `tests/side_panel_flourish.rs`, existing app lifecycle tests in `src/app/panels.rs`.

---

## File Structure

- Modify `src/ui/side_panel/flourish.rs`
  - Owns `FeatherClass`, class parameters, deterministic seeding, and the allocation-free tick update.
  - Keeps `BURST_DURATION`, `PanelFlourish`, `BurstEmitter`, and edge distribution behavior.
- Modify `src/ui/side_panel/flourish_view.rs`
  - Draws `Dust`, `Chunk`, and `Feather` with renderer-neutral canvas primitives.
  - Keeps future sprite replacement isolated to drawing functions.
- Modify `src/ui/side_panel/mod.rs`
  - Re-export `FeatherClass` with the existing flourish types so tests and future callers can assert class behavior.
- Modify `tests/side_panel_flourish.rs`
  - Adds pure model tests for class mix, class motion differences, and shared fade cleanup.
  - Keeps model tests on open transitions because app glue no longer emits close bursts, while `PanelFlourish::emit` remains a pure model primitive.
- Modify `src/app/panels.rs` only if a lifecycle test fails after model changes.
  - Existing open-only and frame-subscription tests should remain valid.

---

### Task 1: Introduce Feather Classes And Deterministic Seeding

**Files:**
- Modify: `src/ui/side_panel/flourish.rs`
- Modify: `src/ui/side_panel/mod.rs`
- Test: `tests/side_panel_flourish.rs`

- [ ] **Step 1: Write the failing class-mix test**

Add imports at the top of `tests/side_panel_flourish.rs`:

```rust
use std::collections::BTreeSet;
```

Update the `honkhonk::ui::side_panel` import to include `FeatherClass`:

```rust
use honkhonk::ui::side_panel::{
    BurstEmitter, BurstLine, FeatherClass, PanelFlourish, PanelRect, PanelTransition,
    panel_burst_emitter,
};
```

Add this test after `edge_particles_span_most_of_the_panel_edge`:

```rust
#[test]
fn burst_seeds_all_feather_classes() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    let classes = flourish
        .particles()
        .iter()
        .map(|p| p.class)
        .collect::<BTreeSet<_>>();

    assert!(classes.contains(&FeatherClass::Dust));
    assert!(classes.contains(&FeatherClass::Chunk));
    assert!(classes.contains(&FeatherClass::Feather));
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test --test side_panel_flourish burst_seeds_all_feather_classes
```

Expected: FAIL at compile time because `FeatherClass` and `FeatherParticle::class` do not exist yet.

- [ ] **Step 3: Add `FeatherClass` and particle fields**

In `src/ui/side_panel/flourish.rs`, add the enum near `FeatherParticle`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeatherClass {
    Dust,
    Chunk,
    Feather,
}
```

Extend `FeatherParticle`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatherParticle {
    pub class: FeatherClass,
    pub position: Point,
    pub velocity: Vector,
    pub alpha: f32,
    pub size: f32,
    pub rotation: f32,
    wobble_phase: f32,
    wobble_frequency: f32,
    wobble_strength: f32,
    horizontal_drag: f32,
    vertical_drag: f32,
    rotation_velocity: f32,
}
```

Add class parameters below `deterministic_jitter`:

```rust
#[derive(Debug, Clone, Copy)]
struct FeatherClassParams {
    size: f32,
    outward_speed: f32,
    vertical_bias: f32,
    wobble_frequency: f32,
    wobble_strength: f32,
    horizontal_drag: f32,
    vertical_drag: f32,
    rotation_velocity: f32,
}

fn feather_class(i: usize) -> FeatherClass {
    match i % 6 {
        0 | 3 => FeatherClass::Dust,
        1 | 4 => FeatherClass::Chunk,
        _ => FeatherClass::Feather,
    }
}

fn class_params(class: FeatherClass, i: usize) -> FeatherClassParams {
    let variant = (i % 3) as f32;
    match class {
        FeatherClass::Dust => FeatherClassParams {
            size: 3.0 + variant,
            outward_speed: 82.0 + variant * 5.0,
            vertical_bias: 34.0 + variant * 8.0,
            wobble_frequency: 8.0 + variant,
            wobble_strength: 6.0 + variant,
            horizontal_drag: 2.2,
            vertical_drag: 0.25,
            rotation_velocity: 1.4 + variant * 0.2,
        },
        FeatherClass::Chunk => FeatherClassParams {
            size: 8.0 + variant * 1.5,
            outward_speed: 74.0 + variant * 6.0,
            vertical_bias: 10.0 + variant * 5.0,
            wobble_frequency: 5.6 + variant * 0.5,
            wobble_strength: 15.0 + variant * 2.0,
            horizontal_drag: 1.4,
            vertical_drag: 0.75,
            rotation_velocity: 0.95 + variant * 0.12,
        },
        FeatherClass::Feather => FeatherClassParams {
            size: 16.0 + variant * 2.5,
            outward_speed: 68.0 + variant * 4.0,
            vertical_bias: -18.0 + variant * 4.0,
            wobble_frequency: 3.2 + variant * 0.35,
            wobble_strength: 26.0 + variant * 3.0,
            horizontal_drag: 0.9,
            vertical_drag: 1.65,
            rotation_velocity: 0.55 + variant * 0.08,
        },
    }
}
```

Update `seed_particle` to use the class parameters:

```rust
fn seed_particle(emitter: BurstEmitter, transition: PanelTransition, i: usize) -> FeatherParticle {
    let class = feather_class(i);
    let params = class_params(class, i);
    let dir = particle_direction(emitter, i);
    let perp = Vector::new(-dir.y, dir.x);
    let scatter = ((i % 7) as f32 - 3.0) / 3.0;
    let drift = params.vertical_bias + (-4.0 + (i % 3) as f32 * 4.0);
    let velocity = add(
        scale(dir, params.outward_speed),
        add(scale(perp, scatter * 28.0), Vector::new(0.0, drift)),
    );
    let offset = add(
        scale(dir, 5.0 + (i % 3) as f32 * 2.0),
        scale(perp, scatter * 6.0),
    );
    let rotation_bias = match transition {
        PanelTransition::Open => 0.0,
        PanelTransition::Close => 0.35,
    };

    FeatherParticle {
        class,
        position: translate(emitter_point(emitter, i), offset),
        velocity,
        alpha: 1.0,
        size: params.size,
        rotation: rotation_bias + scatter * 0.45,
        wobble_phase: i as f32 * 1.618_034,
        wobble_frequency: params.wobble_frequency,
        wobble_strength: params.wobble_strength,
        horizontal_drag: params.horizontal_drag,
        vertical_drag: params.vertical_drag,
        rotation_velocity: params.rotation_velocity,
    }
}
```

In `src/ui/side_panel/mod.rs`, update the re-export:

```rust
pub use flourish::{
    BURST_DURATION, BurstEmitter, BurstLine, FeatherClass, FeatherParticle, PanelFlourish,
    PanelTransition, panel_burst_emitter,
};
```

- [ ] **Step 4: Run the focused test to verify it passes**

Run:

```bash
cargo test --test side_panel_flourish burst_seeds_all_feather_classes
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ui/side_panel/flourish.rs src/ui/side_panel/mod.rs tests/side_panel_flourish.rs
git commit -m "feat(ui): classify panel feather particles"
```

---

### Task 2: Implement Cheap Feather Motion Physics

**Files:**
- Modify: `src/ui/side_panel/flourish.rs`
- Test: `tests/side_panel_flourish.rs`

- [ ] **Step 1: Write failing class-motion tests**

Add these helpers near the panel helper functions in `tests/side_panel_flourish.rs`:

```rust
fn first_particle_of(
    flourish: &PanelFlourish,
    class: FeatherClass,
) -> honkhonk::ui::side_panel::FeatherParticle {
    *flourish
        .particles()
        .iter()
        .find(|p| p.class == class)
        .expect("burst should include requested feather class")
}

fn tick_for(flourish: &mut PanelFlourish, start: Instant, total: Duration) {
    let total_ms = total.as_millis() as u64;
    let mut elapsed_ms = 16;
    while elapsed_ms < total_ms {
        assert!(flourish.tick(start + Duration::from_millis(elapsed_ms), None));
        elapsed_ms += 16;
    }
    assert!(flourish.tick(start + total, None));
}
```

Add these tests after `burst_seeds_all_feather_classes`:

```rust
#[test]
fn dust_descends_farther_than_full_feather() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    let dust_start = first_particle_of(&flourish, FeatherClass::Dust).position.y;
    let feather_start = first_particle_of(&flourish, FeatherClass::Feather).position.y;

    tick_for(&mut flourish, now, Duration::from_millis(900));

    let dust_drop = first_particle_of(&flourish, FeatherClass::Dust).position.y - dust_start;
    let feather_drop =
        first_particle_of(&flourish, FeatherClass::Feather).position.y - feather_start;

    assert!(
        dust_drop > feather_drop + 18.0,
        "dust should fall faster than a full feather: dust={dust_drop}, feather={feather_drop}"
    );
}

#[test]
fn full_feathers_swoop_more_than_dust() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    let dust_start = first_particle_of(&flourish, FeatherClass::Dust).position.x;
    let feather_start = first_particle_of(&flourish, FeatherClass::Feather).position.x;

    tick_for(&mut flourish, now, Duration::from_millis(1200));

    let dust_dx = (first_particle_of(&flourish, FeatherClass::Dust).position.x - dust_start).abs();
    let feather_dx =
        (first_particle_of(&flourish, FeatherClass::Feather).position.x - feather_start).abs();

    assert!(
        feather_dx > dust_dx + 8.0,
        "full feathers should have more lateral swoop: dust={dust_dx}, feather={feather_dx}"
    );
}
```

- [ ] **Step 2: Run tests to verify the motion test fails**

Run:

```bash
cargo test --test side_panel_flourish dust_descends_farther_than_full_feather
cargo test --test side_panel_flourish full_feathers_swoop_more_than_dust
```

Expected: FAIL on `full_feathers_swoop_more_than_dust` because `tick_particle` does not use lateral wobble yet. `dust_descends_farther_than_full_feather` may already pass from the class-specific initial vertical bias; keep it because it pins the final behavior.

- [ ] **Step 3: Implement the cheap force-style update**

In `src/ui/side_panel/flourish.rs`, keep `GRAVITY`. Add a cheap drag helper below `tick_particle`:

```rust
fn drag_factor(drag_per_second: f32, dt: f32) -> f32 {
    (1.0 - drag_per_second * dt).clamp(0.0, 1.0)
}
```

Replace `tick_particle` with:

```rust
fn tick_particle(particle: &mut FeatherParticle, dt: f32, cursor: Option<Point>) {
    if let Some(cursor) = cursor {
        particle.velocity = add(particle.velocity, cursor_bump(*particle, cursor, dt));
    }

    let wobble = (particle.wobble_phase + particle.wobble_frequency * dt).sin();
    particle.wobble_phase += particle.wobble_frequency * dt;
    particle.velocity.x += wobble * particle.wobble_strength * dt;
    particle.velocity.y += GRAVITY * dt;

    particle.velocity.x *= drag_factor(particle.horizontal_drag, dt);
    particle.velocity.y *= drag_factor(particle.vertical_drag, dt);

    particle.position = translate(particle.position, scale(particle.velocity, dt));
    particle.rotation += particle.rotation_velocity * dt * (0.6 + wobble * 0.4);
}
```

- [ ] **Step 4: Run focused motion tests**

Run:

```bash
cargo test --test side_panel_flourish dust_descends_farther_than_full_feather
cargo test --test side_panel_flourish full_feathers_swoop_more_than_dust
```

Expected: PASS. If a threshold is too tight after implementation, adjust the assertion margins, not the test intent:

- dust must descend measurably farther than feather over the same time window.
- full feather must show measurably more lateral motion than dust.

- [ ] **Step 5: Run all flourish model tests**

Run:

```bash
cargo test --test side_panel_flourish
```

Expected: PASS. If `emitted_edge_particles_start_away_from_panel` fails because wobble altered the x velocity before the assertion, check that the assertion reads the initial seeded velocity before any tick.

- [ ] **Step 6: Commit**

```bash
git add src/ui/side_panel/flourish.rs tests/side_panel_flourish.rs
git commit -m "feat(ui): add feather air-resistance motion"
```

---

### Task 3: Draw Dust, Chunks, And Full Feathers Differently

**Files:**
- Modify: `src/ui/side_panel/flourish_view.rs`
- Test: `tests/side_panel_flourish.rs`

- [ ] **Step 1: Write a lightweight rendering-data test**

Add this test to `tests/side_panel_flourish.rs` after the class mix test:

```rust
#[test]
fn feather_classes_have_distinct_visual_sizes() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    let dust_size = first_particle_of(&flourish, FeatherClass::Dust).size;
    let chunk_size = first_particle_of(&flourish, FeatherClass::Chunk).size;
    let feather_size = first_particle_of(&flourish, FeatherClass::Feather).size;

    assert!(dust_size < chunk_size);
    assert!(chunk_size < feather_size);
}
```

This test validates the public data that rendering consumes without brittle screenshot assertions.

- [ ] **Step 2: Run the visual-size test**

Run:

```bash
cargo test --test side_panel_flourish feather_classes_have_distinct_visual_sizes
```

Expected: PASS if Task 1 class sizes are already in place. If it fails, fix `class_params` so `Dust < Chunk < Feather`.

- [ ] **Step 3: Split drawing by class**

In `src/ui/side_panel/flourish_view.rs`, replace `draw_feather` with a dispatcher:

```rust
fn draw_feather(frame: &mut canvas::Frame, particle: FeatherParticle, colors: FeatherColors) {
    if particle.alpha <= 0.0 {
        return;
    }

    match particle.class {
        super::FeatherClass::Dust => draw_dust(frame, particle, colors),
        super::FeatherClass::Chunk => draw_chunk(frame, particle, colors),
        super::FeatherClass::Feather => draw_full_feather(frame, particle, colors),
    }
}
```

Add these functions below it:

```rust
fn draw_dust(frame: &mut canvas::Frame, particle: FeatherParticle, colors: FeatherColors) {
    use iced::widget::canvas::Path;

    let color = Color {
        a: particle.alpha * 0.55,
        ..colors.shadow
    };
    let radius = (particle.size * 0.45).max(1.0);
    let dot = Path::circle(particle.position, radius);
    frame.fill(&dot, color);
}

fn draw_chunk(frame: &mut canvas::Frame, particle: FeatherParticle, colors: FeatherColors) {
    use iced::widget::canvas::{Path, Stroke};

    let dir = unit_from_angle(particle.rotation);
    let normal = Vector::new(-dir.y, dir.x);
    let start = translate(particle.position, scale(dir, -particle.size * 0.35));
    let end = translate(particle.position, scale(dir, particle.size * 0.35));
    let tip = translate(particle.position, scale(normal, particle.size * 0.25));
    let color = Color {
        a: particle.alpha * 0.72,
        ..colors.shadow
    };

    frame.stroke(
        &Path::line(start, end),
        Stroke::default().with_color(color).with_width(1.2),
    );
    frame.stroke(
        &Path::line(particle.position, tip),
        Stroke::default().with_color(color).with_width(1.0),
    );
}

fn draw_full_feather(frame: &mut canvas::Frame, particle: FeatherParticle, colors: FeatherColors) {
    use iced::widget::canvas::{Path, Stroke};

    let dir = unit_from_angle(particle.rotation);
    let spine = scale(dir, particle.size);
    let start = translate(particle.position, scale(spine, -0.45));
    let end = translate(particle.position, scale(spine, 0.55));
    let ink = Color {
        a: particle.alpha * 0.95,
        ..colors.ink
    };
    let shadow = Color {
        a: particle.alpha * 0.55,
        ..colors.shadow
    };
    frame.stroke(
        &Path::line(start, end),
        Stroke::default().with_color(ink).with_width(1.5),
    );
    draw_barbs(frame, particle, dir, shadow);
}
```

Do not change `draw_barbs`, `unit_from_angle`, `translate`, `add`, or `scale` unless the compiler points out unused helper changes.

- [ ] **Step 4: Run rendering compile tests**

Run:

```bash
cargo test --test side_panel_flourish
cargo test ui::side_panel::view::tests::builds_across_progress
```

Expected: PASS. The second command verifies the side-panel UI path still builds.

- [ ] **Step 5: Commit**

```bash
git add src/ui/side_panel/flourish_view.rs tests/side_panel_flourish.rs
git commit -m "feat(ui): render panel feather particle classes"
```

---

### Task 4: Preserve Lifecycle And Frame Subscription Behavior

**Files:**
- Modify: `src/app/panels.rs` only if tests fail.
- Test: `src/app/panels.rs`
- Test: `tests/side_panel_flourish.rs`

- [ ] **Step 1: Add model test for shared cleanup**

In `tests/side_panel_flourish.rs`, add this test near `feathers_float_down_and_fade_out`:

```rust
#[test]
fn all_classes_clear_at_shared_burst_duration() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    assert!(flourish
        .particles()
        .iter()
        .any(|p| p.class == FeatherClass::Dust));
    assert!(flourish
        .particles()
        .iter()
        .any(|p| p.class == FeatherClass::Chunk));
    assert!(flourish
        .particles()
        .iter()
        .any(|p| p.class == FeatherClass::Feather));

    assert!(!flourish.tick(now + honkhonk::ui::side_panel::BURST_DURATION, None));
    assert!(!flourish.is_animating());
    assert!(flourish.particles().is_empty());
}
```

- [ ] **Step 2: Run lifecycle tests**

Run:

```bash
cargo test --test side_panel_flourish all_classes_clear_at_shared_burst_duration
cargo test app::panels::tests
```

Expected: PASS. If `app::panels::tests` fails, inspect whether class motion left `panel_flourish.is_animating()` true past `BURST_DURATION`; fix `PanelFlourish::tick` to clear at `>= BURST_DURATION`, preserving the current behavior.

- [ ] **Step 3: Check close-transition behavior remains open-only at app level**

Run:

```bash
cargo test app::panels::tests::closing_open_panel_does_not_emit_flourish
cargo test app::panels::tests::close_flourish_does_not_extend_frame_subscription_after_slide
```

Expected: PASS. Do not change app behavior to emit on close.

- [ ] **Step 4: Commit**

If this task only added tests:

```bash
git add tests/side_panel_flourish.rs
git commit -m "test(ui): pin panel feather cleanup lifecycle"
```

If a lifecycle fix was required:

```bash
git add src/app/panels.rs tests/side_panel_flourish.rs
git commit -m "fix(ui): preserve panel feather cleanup lifecycle"
```

---

### Task 5: Full Verification And Stacked PR Prep

**Files:**
- Verify all touched files.
- No implementation edits unless verification exposes a defect.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS. If it fails, run `cargo fmt`, inspect the diff, and include the formatting changes in the next commit.

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test --test side_panel_flourish
cargo test app::panels::tests
```

Expected: PASS.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: PASS. If `src/ui/side_panel/flourish.rs` exceeds the 400-line repo cap, split class parameter helpers into `src/ui/side_panel/feather_motion.rs` and re-export only through `flourish.rs`.

- [ ] **Step 4: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 5: Run release build**

Run:

```bash
cargo build --release
```

Expected: PASS. This is required because the user-reported sluggishness came from a release binary.

- [ ] **Step 6: Check file sizes and whitespace**

Run:

```bash
wc -l src/ui/side_panel/flourish.rs src/ui/side_panel/flourish_view.rs tests/side_panel_flourish.rs
git diff --check
git status --short --branch
```

Expected:

- `src/ui/side_panel/flourish.rs` stays under 400 lines or is split before commit.
- `git diff --check` prints no errors.
- Only intended files are modified.

- [ ] **Step 7: Final commit for verification-only fixes, if any**

If formatting or small verification fixes were needed:

```bash
git add src/ui/side_panel/flourish.rs src/ui/side_panel/flourish_view.rs tests/side_panel_flourish.rs src/app/panels.rs
git commit -m "chore(ui): verify panel feather motion"
```

If no files changed, skip this commit.

- [ ] **Step 8: Push stacked branch and open draft PR**

Run:

```bash
git push -u origin feat/issue-183-smooth-feather-pipeline
gh pr create --draft \
  --repo wrzonance/HonkHonk \
  --base feat/panel-feather-edge-emitter \
  --head feat/issue-183-smooth-feather-pipeline \
  --title "[codex] smooth panel feather motion" \
  --body "## Summary
- Adds Dust/Chunk/Feather panel flourish classes.
- Replaces generic gravity-only motion with deterministic drag and lateral wobble.
- Keeps open-only emission and shared BURST_DURATION cleanup.

## Validation
- \`cargo fmt --check\`
- \`cargo test --test side_panel_flourish\`
- \`cargo test app::panels::tests\`
- \`cargo clippy --all-targets -- -D warnings\`
- \`cargo test\`
- \`cargo build --release\`

Closes #183."
```

Expected: Draft PR opens against `feat/panel-feather-edge-emitter`, not `main`.

---

## Self-Review

- Spec coverage:
  - Particle classes: Task 1.
  - Size-dependent physics: Task 2.
  - Class-specific drawing without assets: Task 3.
  - Shared fade/cleanup and close-only behavior: Task 4.
  - Release-build performance path: Task 5.
- Placeholder scan:
  - The plan intentionally avoids unresolved markers and gives concrete tests, code snippets, commands, and expected outputs for each task.
- Type consistency:
  - `FeatherClass`, `FeatherParticle::class`, and `PanelFlourish::particles()` are used consistently across tasks.
  - The PR base is consistently `feat/panel-feather-edge-emitter`.
