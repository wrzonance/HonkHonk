# Fix: Volume Level Not Applied to New Playback (Issue #51)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure the user's volume setting persists across sound playbacks — when a user sets volume to 15%, every subsequent sound plays at 15%, not the default 100%.

**Architecture:** Add engine-level volume state (`Rc<Cell<f32>>`) to the PipeWire main loop. `SetVolume` updates both the engine-level value AND any active playback. `handle_play` reads the engine-level volume and applies it to newly created `PlaybackState` instances. Single-threaded PipeWire loop means `Rc<Cell<f32>>` is safe — no `Arc`/`Mutex` needed.

**Tech Stack:** Rust, PipeWire (pipewire-rs), Iced 0.13

**Root Cause:** `handle_play()` in `engine.rs` creates fresh `PlaybackState::new()` with hardcoded `volume: 1.0`. The `SetVolume` command only mutates the *currently active* playback's state. When a new sound starts, the engine has nowhere to read the user's chosen volume from — it's lost.

**Fix Scope:**
- `src/audio/engine.rs` — add `engine_volume` field, thread it through `handle_play`
- `src/audio/playback.rs` — add `PlaybackState::with_volume(f32)` constructor
- Tests for both layers
- **NOT** in scope: UI changes, config changes, new messages

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src/audio/playback.rs` | Modify | Add `with_volume()` constructor, add tests |
| `src/audio/engine.rs` | Modify | Add `engine_volume: Rc<Cell<f32>>`, use in `handle_play` and `SetVolume` |
| `src/app.rs` | Modify | Add test proving volume survives across PlaySound messages |

---

### Task 1: Add `PlaybackState::with_volume()` constructor

**Files:**
- Modify: `src/audio/playback.rs:211-222` (impl block)
- Test: `src/audio/playback.rs` (inline tests module)

- [ ] **Step 1: Write failing test for `with_volume` constructor**

Add to the existing `mod tests` block in `src/audio/playback.rs`:

```rust
#[test]
fn with_volume_sets_initial_volume() {
    let state = PlaybackState::with_volume(0.42);
    assert!((state.volume() - 0.42).abs() < f32::EPSILON);
    assert!(!state.is_active());
}

#[test]
fn with_volume_clamps_above_one() {
    let state = PlaybackState::with_volume(1.5);
    assert!((state.volume() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn with_volume_clamps_below_zero() {
    let state = PlaybackState::with_volume(-0.3);
    assert!((state.volume() - 0.0).abs() < f32::EPSILON);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib playback::tests::with_volume -- --nocapture`
Expected: FAIL — `with_volume` method does not exist.

- [ ] **Step 3: Implement `with_volume` constructor**

Add to `impl PlaybackState` in `src/audio/playback.rs`, directly after `new()`:

```rust
pub fn with_volume(volume: f32) -> Self {
    Self {
        volume: volume.clamp(0.0, 1.0),
        ..Self::new()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib playback::tests::with_volume -- --nocapture`
Expected: 3 tests PASS.

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All existing tests still pass. No regressions.

- [ ] **Step 6: Commit**

```bash
git add src/audio/playback.rs
git commit -m "feat(audio): add PlaybackState::with_volume() constructor"
```

---

### Task 2: Add engine-level volume state and apply to new playbacks

**Files:**
- Modify: `src/audio/engine.rs:84-89` (EngineCtx struct)
- Modify: `src/audio/engine.rs:165-233` (run_engine function)
- Modify: `src/audio/engine.rs:235-294` (handle_play function)

- [ ] **Step 1: Write failing test proving volume is NOT applied (regression test)**

Add to the existing `mod tests` in `src/audio/playback.rs` — this test documents the bug:

```rust
#[test]
fn fill_buffer_respects_initial_volume() {
    let samples = Arc::new(vec![1.0_f32; 100]);
    let mut state = PlaybackState::with_volume(0.5);
    state.start("test".into(), samples, 48000, 1);

    let mut buf = vec![0.0_f32; 10];
    let wrote = state.fill_buffer(&mut buf);

    assert_eq!(wrote, 10);
    for &s in &buf[..wrote] {
        assert!(
            (s - 0.5).abs() < f32::EPSILON,
            "expected 0.5 (1.0 * 0.5 volume), got {s}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it passes (this test already works with `with_volume`)**

Run: `cargo test --lib playback::tests::fill_buffer_respects_initial_volume -- --nocapture`
Expected: PASS — `with_volume` sets `volume` field, `fill_buffer` multiplies by it. This confirms the `PlaybackState` layer is correct and the bug is purely in `engine.rs` not using it.

- [ ] **Step 3: Add `engine_volume` to `EngineCtx`**

In `src/audio/engine.rs`, add import at top of file:

```rust
use std::cell::Cell;
```

Note: `Cell` is already imported — verify before adding duplicate.

Modify the `EngineCtx` struct:

```rust
struct EngineCtx {
    registry_sink_id: Rc<Cell<Option<u32>>>,
    core: pipewire::core::CoreRc,
    active: Rc<RefCell<Option<ActivePlayback>>>,
    evt_tx: mpsc::Sender<AudioEvent>,
    engine_volume: Rc<Cell<f32>>,
}
```

- [ ] **Step 4: Initialize `engine_volume` in `run_engine`**

In `run_engine()`, after creating `active` and before building `EngineCtx`, add:

```rust
let engine_volume: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));
```

Update the `EngineCtx` construction to include it:

```rust
let ctx = EngineCtx {
    registry_sink_id,
    core: core.clone(),
    active: active.clone(),
    evt_tx: evt_tx.clone(),
    engine_volume,
};
```

- [ ] **Step 5: Update `SetVolume` handler to store engine-level volume**

In `run_engine()`, modify the `AudioCommand::SetVolume` match arm:

```rust
AudioCommand::SetVolume(v) => {
    ctx.engine_volume.set(v.clamp(0.0, 1.0));
    if let Some(ref ap) = *ctx.active.borrow() {
        ap.sink_state.borrow_mut().set_volume(v);
        ap.monitor_state.borrow_mut().set_volume(v);
    }
}
```

- [ ] **Step 6: Update `handle_play` to use engine volume for new playbacks**

In `handle_play()`, change the two `PlaybackState::new()` calls to use `with_volume`:

Replace:
```rust
let sink_state = Rc::new(RefCell::new(PlaybackState::new()));
```
With:
```rust
let vol = ctx.engine_volume.get();
let sink_state = Rc::new(RefCell::new(PlaybackState::with_volume(vol)));
```

Replace:
```rust
let mon_state = Rc::new(RefCell::new(PlaybackState::new()));
```
With:
```rust
let mon_state = Rc::new(RefCell::new(PlaybackState::with_volume(vol)));
```

- [ ] **Step 7: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors or warnings.

- [ ] **Step 8: Run full test suite**

Run: `cargo test`
Expected: All tests pass, including the new ones from Task 1.

- [ ] **Step 9: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 10: Commit**

```bash
git add src/audio/engine.rs
git commit -m "fix(audio): persist volume across playbacks via engine-level state

Engine now stores volume in Rc<Cell<f32>>. SetVolume updates both the
stored value and any active playback. New playbacks inherit the stored
volume via PlaybackState::with_volume().

Fixes #51"
```

---

### Task 3: Add app-level regression test

**Files:**
- Modify: `src/app.rs` (inline tests module)

- [ ] **Step 1: Write test proving volume persists across playback cycles**

Add to `mod tests` in `src/app.rs`:

```rust
#[test]
fn volume_changed_persists_in_config_across_sounds() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::VolumeChanged(0.15));
    assert!((app.config.volume - 0.15).abs() < f32::EPSILON);

    // Simulate playback finishing and starting new sound
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        sound_id: "old".into(),
    }));

    // Volume should still be in config (engine would read this on next play)
    assert!(
        (app.config.volume - 0.15).abs() < f32::EPSILON,
        "config.volume should survive playback cycle"
    );
}
```

This test validates that `config.volume` isn't reset by playback lifecycle events. The engine-level fix (Task 2) ensures the engine reads this value; this test guards the app-layer contract.

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --lib tests::volume_changed_persists_in_config_across_sounds -- --nocapture`
Expected: PASS — app layer already preserves `config.volume` correctly. The bug was engine-side.

- [ ] **Step 3: Run full test suite + clippy**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All green.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "test(app): add regression test for volume persistence across playbacks"
```

---

### Task 4: Format, push, update PR

- [ ] **Step 1: Run cargo fmt**

Run: `cargo fmt`

- [ ] **Step 2: Verify everything**

Run: `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`
Expected: All pass.

- [ ] **Step 3: Commit formatting if needed**

```bash
git add -A
git commit -m "style: apply cargo fmt"
```

- [ ] **Step 4: Push to remote**

```bash
git push origin feat/search-volume-nowplaying
```

- [ ] **Step 5: Comment on PR #50 and Issue #51**

Post comment on PR #50 noting the fix:
```
Fixed #51 — volume was not applied to new playbacks because `handle_play()` created fresh `PlaybackState` with default volume (1.0). Engine now stores volume in `Rc<Cell<f32>>` and applies it to all new playback states via `PlaybackState::with_volume()`.
```

Close Issue #51 via the commit message `Fixes #51` (already included in Task 2 commit).

---

## Verification Checklist

- [ ] `PlaybackState::with_volume(0.15)` creates state with `volume == 0.15`
- [ ] `fill_buffer` on a `with_volume(0.5)` state outputs samples at half amplitude
- [ ] `SetVolume` command stores value in engine AND updates active playback
- [ ] New playback after `SetVolume(0.15)` plays at 0.15, not 1.0
- [ ] `config.volume` survives playback finish/start cycle in app layer
- [ ] All existing tests still pass
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt -- --check` clean
