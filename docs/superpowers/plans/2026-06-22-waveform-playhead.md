# Smooth Playhead + Real Waveform Envelope — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the now-playing playhead track playback smoothly at the display refresh rate, and replace the fake id-hash waveform with a real peak-amplitude envelope (groundwork for a future trim function).

**Architecture:** Two sequenced PRs from one spec (`docs/superpowers/specs/2026-06-22-waveform-playhead-design.md`, issue #138). **PR-A** (Tasks 1–2): a `PlayheadClock` that predict-and-corrects between the authoritative 10 Hz `AudioEvent::Progress` anchors, driven by an `iced::window::frames()` subscription gated on playback. **PR-B** (Tasks 3–4): an `Envelope` extracted once per sound from the decoded PCM, cached, and downsampled for display.

**Tech Stack:** Rust, Iced 0.14 (Elm/MVU, `canvas`), PipeWire audio. No new crates.

## Global Constraints

- **File size ≤ 400 lines; functions ≤ 50 lines** (HonkHonk override, stricter than global). `src/app.rs` is already 2,598 lines and over budget — **add only glue, no logic; never extract/restructure it in this plan.**
- **`cargo clippy -- -D warnings` must pass at every commit** (cognitive-complexity 10, too-many-lines 50, too-many-arguments 5). Build stays green between every step.
- **No `.unwrap()` / `panic!()` in non-test code; no `console`/`eprintln` spam in hot paths.** Error context via `thiserror`/`anyhow` where errors cross boundaries (none introduced here).
- **TDD mandatory:** failing test first, then minimal implementation. **Coverage target 80%** (`cargo tarpaulin`).
- **Branch discipline:** never commit to `main`. PR-A on `perf/smooth-playhead`; PR-B on `feat/waveform-envelope` branched from `main` **after PR-A merges** (both touch `app.rs`/`now_playing.rs`). Commits: Conventional Commits + `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. PRs link `#138`; PR-B closes it.
- **No Iced view-render tests** (per CLAUDE.md) — test pure logic and `update()` state transitions only.

---

## File Structure

**PR-A**
- Create `src/ui/playhead.rs` — `PlayheadClock` (predict-and-correct interpolation). Pure, no Iced types. ~90 LOC incl. tests.
- Modify `src/ui/mod.rs` — register `pub mod playhead;`.
- Modify `src/app.rs` — new `Frame(Instant)` message; `playhead`/`display_progress` state; wire play/Progress/Finished; gate `window::frames()` on playback; view + cache-sync read `display_progress`. Glue only.

**PR-B**
- Create `src/audio/envelope.rs` — `Envelope` extraction + downsample. Pure. ~140 LOC incl. tests.
- Modify `src/audio/mod.rs` — register `mod envelope;` + re-export.
- Modify `src/ui/waveform.rs` — `WAVEFORM_BARS` 48→64; delete `samples(id)` + its 3 tests.
- Modify `src/ui/now_playing.rs` — `WaveformProgram.samples: Vec<f32>`; `view_now_playing`/`view_waveform` take the envelope.
- Modify `src/app.rs` — `waveform_cache` field; populate at decode (pre-volume); pass the playing sound's envelope to the view. Glue only.

---

# PR-A — Smooth playhead

## Task 1: `PlayheadClock` interpolation

**Files:**
- Create: `src/ui/playhead.rs`
- Modify: `src/ui/mod.rs` (add `pub mod playhead;` after `pub mod now_playing;`)
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `PlayheadClock::new(duration: Duration, now: Instant) -> Self`; `PlayheadClock::on_progress(&mut self, progress: f32, now: Instant)`; `PlayheadClock::display(&self, now: Instant) -> f32` (clamped `0.0..=1.0`).

- [ ] **Step 1: Write the failing tests**

Create `src/ui/playhead.rs`:

```rust
//! Predict-and-correct clock for the now-playing playhead. The 10 Hz audio
//! `AudioEvent::Progress` events are the authoritative anchor; `display`
//! extrapolates between them from wall-clock elapsed so the line moves at the
//! display refresh rate without drifting (#138, PR-A).

use std::time::{Duration, Instant};

/// Holds the last authoritative progress sample and extrapolates a smooth
/// display position from wall-clock time since that sample.
#[derive(Debug, Clone)]
pub struct PlayheadClock {
    anchor: f32,
    anchor_at: Instant,
    duration: Duration,
}

impl PlayheadClock {
    /// Starts a clock at progress 0 for a sound of length `duration`.
    pub fn new(duration: Duration, now: Instant) -> Self {
        Self {
            anchor: 0.0,
            anchor_at: now,
            duration,
        }
    }

    /// Re-anchors to an authoritative progress sample, snapping out any
    /// accumulated prediction error (forward or backward).
    pub fn on_progress(&mut self, progress: f32, now: Instant) {
        self.anchor = progress.clamp(0.0, 1.0);
        self.anchor_at = now;
    }

    /// Extrapolated display progress at `now`, clamped to `0.0..=1.0`.
    pub fn display(&self, now: Instant) -> f32 {
        extrapolate(
            self.anchor,
            now.saturating_duration_since(self.anchor_at),
            self.duration,
        )
    }
}

/// Pure extrapolation core: `anchor + elapsed/duration`, clamped to `0..=1`.
/// A zero `duration` yields the clamped anchor (no division).
fn extrapolate(anchor: f32, elapsed: Duration, duration: Duration) -> f32 {
    let anchor = anchor.clamp(0.0, 1.0);
    if duration.is_zero() {
        return anchor;
    }
    (anchor + elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrapolate_midpoint() {
        let p = extrapolate(0.0, Duration::from_secs(5), Duration::from_secs(10));
        assert!((p - 0.5).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn extrapolate_clamps_at_end() {
        let p = extrapolate(0.9, Duration::from_secs(100), Duration::from_secs(10));
        assert_eq!(p, 1.0);
    }

    #[test]
    fn extrapolate_zero_duration_returns_anchor() {
        let p = extrapolate(0.4, Duration::from_secs(5), Duration::ZERO);
        assert!((p - 0.4).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn display_advances_with_time() {
        let t0 = Instant::now();
        let clock = PlayheadClock::new(Duration::from_secs(10), t0);
        let early = clock.display(t0 + Duration::from_secs(2));
        let late = clock.display(t0 + Duration::from_secs(8));
        assert!(late > early, "expected {late} > {early}");
        assert!((early - 0.2).abs() < 1e-6, "got {early}");
    }

    #[test]
    fn on_progress_snaps_back_an_overshooting_prediction() {
        let t0 = Instant::now();
        let mut clock = PlayheadClock::new(Duration::from_secs(10), t0);
        let predicted = clock.display(t0 + Duration::from_secs(8));
        assert!((predicted - 0.8).abs() < 1e-6, "got {predicted}");
        // Authoritative sample says we are only at 0.3 — snap back.
        let t1 = t0 + Duration::from_secs(8);
        clock.on_progress(0.3, t1);
        assert!((clock.display(t1) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn display_never_exceeds_one_or_drops_below_zero() {
        let t0 = Instant::now();
        let clock = PlayheadClock::new(Duration::from_secs(1), t0);
        assert_eq!(clock.display(t0 + Duration::from_secs(100)), 1.0);
        assert!(clock.display(t0) >= 0.0);
    }
}
```

Add to `src/ui/mod.rs` (alphabetical, after `pub mod now_playing;`):
```rust
pub mod playhead;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib ui::playhead`
Expected: FAIL — `cannot find ... playhead` until `mod.rs` is saved, then PASS only after Step 3's code compiles. (If the file above is fully pasted, this task's RED is the `mod.rs` edit being absent — confirm a compile error first, then add the `mod.rs` line.)

> Note: this module is self-contained, so the test code and implementation live in the same file. RED here is "module not registered / does not compile"; GREEN is registering it and the tests passing.

- [ ] **Step 3: Confirm implementation compiles clean**

Run: `cargo clippy --lib -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib ui::playhead`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git checkout -b perf/smooth-playhead origin/main   # if not already on it
git add src/ui/playhead.rs src/ui/mod.rs
git commit -m "feat(ui): PlayheadClock predict-and-correct interpolation (#138)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Wire the clock + frame subscription into the app

**Files:**
- Modify: `src/app.rs` (imports, `Message`, `HonkHonk` struct + both constructors, `update` handlers, `play_sound_entry`, `subscription`, `view`, trailing `now_playing.sync`)
- Test: `src/app.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::ui::playhead::PlayheadClock` (Task 1); `crate::audio::DecodedAudio::duration: Duration` (already exists); `iced::window::frames() -> Subscription<Instant>`.
- Produces: `Message::Frame(Instant)`; `HonkHonk.display_progress: f32` (the value the now-playing view and cache-sync read).

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/app.rs` (near `progress_event_updates_progress`):

```rust
#[test]
fn frame_message_advances_display_progress_while_playing() {
    use std::time::{Duration, Instant};
    let mut app = HonkHonk::new_for_test();
    let t0 = Instant::now();
    app.playhead = Some(crate::ui::playhead::PlayheadClock::new(
        Duration::from_secs(10),
        t0,
    ));
    let _ = app.update(Message::Frame(t0 + Duration::from_secs(5)));
    assert!(
        (app.display_progress - 0.5).abs() < 1e-3,
        "got {}",
        app.display_progress
    );
}

#[test]
fn frame_message_is_noop_when_idle() {
    use std::time::{Duration, Instant};
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::Frame(Instant::now() + Duration::from_secs(1)));
    assert_eq!(app.display_progress, 0.0);
}

#[test]
fn progress_event_sets_display_progress() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.65)));
    assert!((app.display_progress - 0.65).abs() < f32::EPSILON);
}

#[test]
fn playback_finished_clears_playhead_and_display_progress() {
    use std::time::{Duration, Instant};
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "test".into(),
    }));
    app.playhead = Some(crate::ui::playhead::PlayheadClock::new(
        Duration::from_secs(5),
        Instant::now(),
    ));
    let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.8)));
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        sound_id: "test".into(),
    }));
    assert!(app.playhead.is_none());
    assert_eq!(app.display_progress, 0.0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::tests::frame_message_advances_display_progress_while_playing`
Expected: FAIL — `no variant Frame`, `no field playhead`, `no field display_progress`.

- [ ] **Step 3: Write the implementation**

**3a. Imports** — add near the top of `src/app.rs` (with the other `use` lines):
```rust
use std::time::Instant;
```

**3b. `Message` enum** — add under the `// Window / cursor` group:
```rust
    /// Per-frame redraw tick (vsync-paced via `window::frames()`), carrying the
    /// frame time. Only subscribed while a sound plays. Drives playhead interpolation.
    Frame(Instant),
```

**3c. `HonkHonk` struct** — add after the `now_playing: ...` field:
```rust
    /// Predict-and-correct clock driving the smooth playhead; `Some` while a
    /// sound plays. Authoritative anchor is the 10 Hz `AudioEvent::Progress`.
    playhead: Option<crate::ui::playhead::PlayheadClock>,
    /// Frame-interpolated playhead position fed to the now-playing view and the
    /// waveform cache-sync. Distinct from `progress` (the raw 10 Hz anchor).
    display_progress: f32,
```

**3d. Both `HonkHonk { ... }` constructor literals** (two sites, near lines 312 and 355) — add:
```rust
            playhead: None,
            display_progress: 0.0,
```

**3e. `update` — new `Frame` arm** (add inside the `match message` in `update`):
```rust
            Message::Frame(now) => {
                if let Some(ref clock) = self.playhead {
                    self.display_progress = clock.display(now);
                }
                Task::none()
            }
```

**3f. `update` — `AudioEvent::Progress(p)` arm** — replace the existing `AudioEvent::Progress(p) => { self.progress = p; }` body with:
```rust
                    AudioEvent::Progress(p) => {
                        self.progress = p;
                        let now = Instant::now();
                        if let Some(ref mut clock) = self.playhead {
                            clock.on_progress(p, now);
                            self.display_progress = clock.display(now);
                        } else {
                            self.display_progress = p;
                        }
                    }
```

**3g. `update` — `AudioEvent::PlaybackFinished` arm** — inside the existing `if self.playing.as_deref() == Some(sound_id.as_str()) { ... }` block (which already sets `self.playing = None; self.progress = 0.0;`), add:
```rust
                            self.playhead = None;
                            self.display_progress = 0.0;
```

**3h. `play_sound_entry`** — immediately after the decode match succeeds (after `let decoded = match crate::audio::decode(&sound.path) { ... };`, before `if let Some(ref audio) = self.audio {`), add:
```rust
        // Start the smooth-playhead clock; duration is exact from the decoded
        // PCM (avoids depending on the lazy duration scan). PR-B inserts the
        // waveform envelope here too, from `decoded.samples` before per-vol.
        self.playhead = Some(crate::ui::playhead::PlayheadClock::new(
            decoded.duration,
            Instant::now(),
        ));
        self.display_progress = 0.0;
```

**3i. `subscription`** — after `let mut subs = vec![shortcuts, tray_poll, events];`, add:
```rust
        // Vsync-paced playhead animation — subscribed ONLY while a sound plays so
        // an idle tray app never repaints. `window::frames()` yields one `Instant`
        // per refresh; subscriptions are re-evaluated each update, so this drops
        // out automatically when playback ends. No fps cap (let it fly at refresh).
        if self.playing.is_some() {
            subs.push(iced::window::frames().map(Message::Frame));
        }
```

**3j. `view`** — in the `now_playing::view_now_playing(...)` call, change `self.progress` to `self.display_progress`:
```rust
        let now_playing = now_playing::view_now_playing(
            &self.now_playing,
            self.playing.as_deref(),
            &self.sounds,
            self.display_progress,
            self.config.volume,
        );
```

**3k. Trailing cache-sync** (the `self.now_playing.sync(self.playing.as_deref(), self.progress);` at the end of `update`) — change to:
```rust
        self.now_playing
            .sync(self.playing.as_deref(), self.display_progress);
```

- [ ] **Step 4: Run tests + clippy to verify green**

Run: `cargo test --lib app::tests`
Expected: PASS (incl. the 4 new tests; existing `progress_event_updates_progress` / `playback_finished_resets_progress` still pass).
Run: `cargo clippy --lib -- -D warnings`
Expected: no warnings (no unused field — `display_progress` is read by `view`/`sync`).

- [ ] **Step 5: Manual smoke (optional but recommended)**

Run: `cargo run` → play a sound → the playhead line should glide smoothly (not step ~10×/sec). On a high-refresh monitor it tracks the refresh rate.

- [ ] **Step 6: Commit + open PR-A**

```bash
git add src/app.rs
git commit -m "perf(ui): smooth playhead via window::frames + predict-and-correct (#138)

Repaint at the display refresh rate only while playing and interpolate the
playhead from wall-clock elapsed between the 10 Hz Progress anchors. Idle when
nothing plays. Refs #138 (PR-A).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
gh pr create --base main --head perf/smooth-playhead \
  --title "perf(ui): smooth playhead (PR-A of #138)" \
  --body "## Why
Playhead stepped ~10 fps (10 Hz Progress + 10 Hz TrayPoll, no frame subscription).

## What
\`PlayheadClock\` predict-and-correct, driven by \`window::frames()\` gated on playback. Refs #138.

## Testing
- [ ] Unit tests pass (\`cargo test\`)
- [ ] clippy clean
- [ ] Manual: line glides smoothly during playback, app idle when stopped
- [ ] CI green

🤖 Co-authored by Claude Opus 4.8. Refs #138 (PR-A)."
```

> **PR-B starts only after PR-A merges.** Then: `git checkout main && git pull && git checkout -b feat/waveform-envelope`.

---

# PR-B — Real waveform envelope

## Task 3: `Envelope` extraction

**Files:**
- Create: `src/audio/envelope.rs`
- Modify: `src/audio/mod.rs` (add `mod envelope;` after `mod decoder;` and `pub use envelope::{Envelope, ENVELOPE_BUCKETS};` with the other re-exports)
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `pub const ENVELOPE_BUCKETS: usize = 1024;`; `Envelope::from_samples(samples: &[f32], channels: u16, buckets: usize) -> Envelope`; `Envelope::bars(&self, n: usize) -> Vec<f32>` (always returns exactly `n` finite values in `0.0..=1.0`).

- [ ] **Step 1: Write the failing tests + implementation**

Create `src/audio/envelope.rs`:

```rust
//! Peak-amplitude envelope of a decoded sound, for the now-playing waveform and
//! the future trim editor. Computed once per sound from the decoded PCM; stored
//! hi-res and downsampled for display (#138, PR-B). Pure — no audio I/O.

/// Hi-res bucket count stored per sound. Downsampled to the display bar count by
/// [`Envelope::bars`]; the future trim editor reads the full resolution.
pub const ENVELOPE_BUCKETS: usize = 1024;

/// Normalized peak-amplitude envelope; every value in `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    peaks: Vec<f32>,
}

impl Envelope {
    /// Builds an envelope from interleaved `samples` with `channels` lanes.
    /// Mono-sums each frame, splits the frames into `buckets` contiguous groups,
    /// takes the peak `|amplitude|` per group, then normalizes by the global
    /// peak so the tallest bar is `1.0`. Silent or empty input → all-zero peaks
    /// (no divide-by-zero).
    pub fn from_samples(samples: &[f32], channels: u16, buckets: usize) -> Self {
        let ch = channels as usize;
        if buckets == 0 || ch == 0 || samples.len() < ch {
            return Self {
                peaks: vec![0.0; buckets],
            };
        }
        let frames = samples.len() / ch;

        let mut peaks = vec![0.0_f32; buckets];
        for (frame_idx, frame) in samples.chunks_exact(ch).enumerate() {
            let mono = frame.iter().copied().sum::<f32>() / ch as f32;
            // u64 math avoids overflow on long inputs (frame_idx * buckets).
            let bucket = (frame_idx as u64 * buckets as u64 / frames as u64) as usize;
            let mag = mono.abs();
            if mag > peaks[bucket] {
                peaks[bucket] = mag;
            }
        }

        let max = peaks.iter().copied().fold(0.0_f32, f32::max);
        if max > f32::EPSILON {
            for p in &mut peaks {
                *p /= max;
            }
        }
        Self { peaks }
    }

    /// Max-pools the hi-res peaks down to `n` display bars. When `n >=
    /// peaks.len()` the peaks are returned padded with zeros to length `n`.
    /// Never panics; always returns exactly `n` values.
    pub fn bars(&self, n: usize) -> Vec<f32> {
        if n == 0 {
            return Vec::new();
        }
        let len = self.peaks.len();
        if len == 0 {
            return vec![0.0; n];
        }
        if n >= len {
            let mut out = self.peaks.clone();
            out.resize(n, 0.0);
            return out;
        }
        (0..n)
            .map(|i| {
                let start = i * len / n;
                let end = (((i + 1) * len / n).max(start + 1)).min(len);
                self.peaks[start..end]
                    .iter()
                    .copied()
                    .fold(0.0_f32, f32::max)
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn peaks(&self) -> &[f32] {
        &self.peaks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_amplitude_is_flat_after_normalize() {
        let samples = vec![0.5_f32; 4096];
        let env = Envelope::from_samples(&samples, 1, 64);
        assert_eq!(env.peaks().len(), 64);
        for p in env.peaks() {
            assert!((p - 1.0).abs() < 1e-6, "got {p}");
        }
    }

    #[test]
    fn silence_is_all_zero() {
        let env = Envelope::from_samples(&vec![0.0_f32; 4096], 1, 64);
        assert!(env.peaks().iter().all(|&p| p == 0.0));
    }

    #[test]
    fn empty_input_is_zero_filled() {
        let env = Envelope::from_samples(&[], 2, 32);
        assert_eq!(env.peaks().len(), 32);
        assert!(env.peaks().iter().all(|&p| p == 0.0));
    }

    #[test]
    fn half_and_full_regions_preserve_ratio() {
        let mut samples = vec![0.25_f32; 2048];
        samples.extend(vec![0.5_f32; 2048]);
        let env = Envelope::from_samples(&samples, 1, 2);
        assert!((env.peaks()[0] - 0.5).abs() < 1e-6, "got {}", env.peaks()[0]);
        assert!((env.peaks()[1] - 1.0).abs() < 1e-6, "got {}", env.peaks()[1]);
    }

    #[test]
    fn stereo_is_mono_summed_not_concatenated() {
        // L full, R silent → mono 0.5 everywhere → normalized 1.0; length == frames.
        let mut samples = Vec::new();
        for _ in 0..2048 {
            samples.push(1.0_f32);
            samples.push(0.0_f32);
        }
        let env = Envelope::from_samples(&samples, 2, 16);
        assert_eq!(env.peaks().len(), 16);
        for p in env.peaks() {
            assert!((p - 1.0).abs() < 1e-6, "got {p}");
        }
    }

    #[test]
    fn bars_max_pools_a_loud_region() {
        let mut samples = vec![1.0_f32; 1024]; // loud first quarter
        samples.extend(vec![0.0_f32; 3072]); // silent rest
        let env = Envelope::from_samples(&samples, 1, 64);
        let bars = env.bars(4);
        assert_eq!(bars.len(), 4);
        assert!((bars[0] - 1.0).abs() < 1e-6, "loud bar: {}", bars[0]);
        assert!(bars[3] < 1e-6, "silent bar: {}", bars[3]);
    }

    #[test]
    fn bars_pads_when_n_exceeds_resolution() {
        let env = Envelope::from_samples(&vec![0.5; 100], 1, 4);
        assert_eq!(env.bars(10).len(), 10);
    }

    #[test]
    fn bars_zero_n_is_empty() {
        let env = Envelope::from_samples(&vec![0.5; 100], 1, 4);
        assert!(env.bars(0).is_empty());
    }
}
```

Add to `src/audio/mod.rs`:
```rust
mod envelope;
```
and with the other `pub use` re-exports:
```rust
pub use envelope::{Envelope, ENVELOPE_BUCKETS};
```

- [ ] **Step 2: Run tests to verify they fail (then pass)**

Run: `cargo test --lib audio::envelope`
Expected: compile error until `mod.rs` is updated; then PASS (8 tests).

- [ ] **Step 3: Clippy**

Run: `cargo clippy --lib -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add src/audio/envelope.rs src/audio/mod.rs
git commit -m "feat(audio): peak-amplitude Envelope extraction + downsample (#138)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Use the envelope in the now-playing waveform

**Files:**
- Modify: `src/ui/waveform.rs` (`WAVEFORM_BARS` 48→64; delete `samples` + its 3 tests + the now-stale doc line)
- Modify: `src/ui/now_playing.rs` (`WaveformProgram.samples: Vec<f32>`; `view_now_playing`/`view_waveform` take the envelope)
- Modify: `src/app.rs` (`waveform_cache` field + both constructors; populate at decode; pass the playing sound's envelope to the view)
- Test: `src/app.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::audio::Envelope` + `ENVELOPE_BUCKETS` (Task 3); `crate::ui::waveform::WAVEFORM_BARS`.
- Produces: `HonkHonk.waveform_cache: HashMap<String, Arc<Envelope>>`; new `view_now_playing(now_playing, playing, sounds, progress, vol, envelope: Option<&Envelope>)` signature.

- [ ] **Step 1: Write the failing app-level test**

Add to `src/app.rs` `#[cfg(test)] mod tests` (the `write_test_wav` + `tempfile` helpers already exist in this module):

```rust
#[test]
fn playing_a_sound_caches_its_waveform_envelope() {
    let mut app = HonkHonk::new_for_test();
    let (handle, _evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);

    let dir = tempfile::tempdir().expect("tempdir");
    let wav_path = dir.path().join("honk.wav");
    write_test_wav(&wav_path);
    app.sounds = vec![SoundEntry {
        id: "wav1".into(),
        name: "Honk".into(),
        path: wav_path,
        format: crate::state::AudioFormat::Wav,
        duration_ms: Some(100),
        category: "Test".into(),
    }];

    let _ = app.update(Message::PlaySound("wav1".into()));
    assert!(
        app.waveform_cache.contains_key("wav1"),
        "envelope should be cached after play"
    );
    assert_eq!(
        app.waveform_cache["wav1"].bars(crate::ui::waveform::WAVEFORM_BARS).len(),
        crate::ui::waveform::WAVEFORM_BARS
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib app::tests::playing_a_sound_caches_its_waveform_envelope`
Expected: FAIL — `no field waveform_cache`.

- [ ] **Step 3a: `src/ui/waveform.rs`**

Change the constant:
```rust
/// Number of vertical bars in the now-playing waveform. Downsampled from the
/// hi-res `Envelope` (`ENVELOPE_BUCKETS`) at view time.
pub const WAVEFORM_BARS: usize = 64;
```
Delete the `pub fn samples(id: &str) -> [f32; WAVEFORM_BARS] { ... }` function entirely, and delete its three tests: `samples_are_deterministic_for_same_id`, `samples_differ_across_ids`, `samples_stay_in_visible_range`. Update the top-of-file doc comment first line to drop "waveform-sample generation" (it now only owns the render-key/bucket logic), e.g.:
```rust
//! The now-playing waveform's render-key (the `canvas::Cache` invalidation key)
//! and progress bucketing. Real bar heights come from `audio::Envelope` (#138).
```
Keep `RenderKey`, `render_key`, `progress_bucket`, `PROGRESS_BUCKETS` (it stays `= WAVEFORM_BARS`, now 64) and all their tests unchanged.

- [ ] **Step 3b: `src/ui/now_playing.rs`**

Change the `WaveformProgram` struct field:
```rust
struct WaveformProgram<'a> {
    cache: &'a canvas::Cache,
    samples: Vec<f32>,
    progress: f32,
    bar: iced::Color,
    bar_dim: iced::Color,
    accent: iced::Color,
}
```
Add the import at the top:
```rust
use crate::audio::Envelope;
```
Change `view_now_playing` to accept and thread the envelope (add the `envelope` param last):
```rust
pub fn view_now_playing<'a>(
    now_playing: &'a NowPlaying,
    playing: Option<&'a str>,
    sounds: &'a [SoundEntry],
    progress: f32,
    vol: f32,
    envelope: Option<&Envelope>,
) -> Element<'a, Message> {
    let t = Theme::Dark;

    let sound = match playing.and_then(|id| sounds.iter().find(|s| s.id == id)) {
        Some(s) => s,
        None => return Space::new().into(),
    };

    let content = row![
        view_placeholder(t),
        view_sound_info(sound, t),
        view_waveform(now_playing, envelope, progress, t),
        space::horizontal(),
        volume::view_volume(vol),
    ]
    .spacing(theme::space::LG)
    .align_y(iced::Alignment::Center);
    // ... container(...) unchanged ...
}
```
Replace `view_waveform` (drop the `id` param; take the envelope; build display bars):
```rust
/// Builds the canvas widget backed by the persistent cache. Display bars are
/// max-pooled from the cached `Envelope`; a missing envelope renders a flat
/// baseline (never fake bars). The cache is owned by `now_playing`.
fn view_waveform<'a>(
    now_playing: &'a NowPlaying,
    envelope: Option<&Envelope>,
    progress: f32,
    t: Theme,
) -> Element<'a, Message> {
    use crate::ui::waveform::WAVEFORM_BARS;
    let samples = match envelope {
        Some(env) => env.bars(WAVEFORM_BARS),
        None => vec![0.0; WAVEFORM_BARS],
    };
    let program = WaveformProgram {
        cache: &now_playing.cache,
        samples,
        progress,
        bar: t.accent(),
        bar_dim: t.ink_faint(),
        accent: t.ink(),
    };
    canvas::Canvas::new(program)
        .width(320.0)
        .height(theme::component::ARTWORK_SQ)
        .into()
}
```
In `WaveformProgram::draw`, the line `let n = self.samples.len() as f32;` already works with `Vec<f32>` — no other change needed there (`self.samples.iter().enumerate()` is unchanged).

- [ ] **Step 3c: `src/app.rs`**

Add the field after `now_playing: ...` (or after the PR-A fields):
```rust
    /// Per-sound peak-amplitude envelopes for the now-playing waveform, computed
    /// once at decode and reused across frames (#138, PR-B). Session-lifetime.
    waveform_cache: std::collections::HashMap<String, std::sync::Arc<crate::audio::Envelope>>,
```
Add to both `HonkHonk { ... }` constructor literals:
```rust
            waveform_cache: std::collections::HashMap::new(),
```
In `play_sound_entry`, right after the PR-A playhead block (still before `if let Some(ref audio) = self.audio {`, and before `decoded.samples` is moved), add:
```rust
        // Real waveform envelope from the PRE-volume PCM (waveform must not shift
        // with the volume slider). Computed once; reused across frames.
        self.waveform_cache
            .entry(sound.id.clone())
            .or_insert_with(|| {
                std::sync::Arc::new(crate::audio::Envelope::from_samples(
                    &decoded.samples,
                    decoded.channels,
                    crate::audio::ENVELOPE_BUCKETS,
                ))
            });
```
In `view`, resolve the playing sound's envelope and pass it to `view_now_playing`:
```rust
        let envelope = self
            .playing
            .as_deref()
            .and_then(|id| self.waveform_cache.get(id))
            .map(|arc| arc.as_ref());
        let now_playing = now_playing::view_now_playing(
            &self.now_playing,
            self.playing.as_deref(),
            &self.sounds,
            self.display_progress,
            self.config.volume,
            envelope,
        );
```

- [ ] **Step 4: Run tests + clippy**

Run: `cargo test --lib`
Expected: PASS — new `playing_a_sound_caches_its_waveform_envelope`; the deleted `waveform::samples` tests are gone; everything else green.
Run: `cargo clippy --lib -- -D warnings`
Expected: no warnings. (Verify nothing else references `waveform::samples` — `grep -rn "waveform::samples\|samples(" src/ui` should return nothing.)

- [ ] **Step 5: Manual smoke**

Run: `cargo run` → play sounds of different loudness/shape → the bars now reflect the real audio (loud passages tall, silence flat), denser (64 bars), and the played portion still fills as the smooth playhead crosses it.

- [ ] **Step 6: Commit + open PR-B**

```bash
git add src/ui/waveform.rs src/ui/now_playing.rs src/app.rs
git commit -m "feat(ui): render the real waveform envelope in now-playing (#138)

Replace the id-hash placeholder with a real peak-amplitude envelope computed
once per sound at decode (pre-volume) and cached; downsample to 64 display bars.
Groundwork for a trim function. Closes #138 (PR-B).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
gh pr create --base main --head feat/waveform-envelope \
  --title "feat(ui): real waveform envelope (PR-B of #138)" \
  --body "## Why
The now-playing bars were id-hash noise, blocking a usable trim function.

## What
Real peak-amplitude \`Envelope\` from the decoded PCM, hi-res (1024) + cached, downsampled to 64 bars. No height floor (silence reads flat). Closes #138.

## Testing
- [ ] Unit tests pass (\`cargo test\`)
- [ ] clippy clean
- [ ] Manual: bars reflect real audio; loud tall, silence flat
- [ ] CI green

🤖 Co-authored by Claude Opus 4.8. Closes #138 (PR-B)."
```

---

## Self-Review

**Spec coverage:**
- PR-A render driver `window::frames()` gated on playback → Task 2 (3i). ✓
- Predict-and-correct interpolation + duration-from-PCM → Task 1 + Task 2 (3e–3h). ✓
- `display_progress` separate from authoritative `progress`; view + sync switched → Task 2 (3c, 3f, 3j, 3k). ✓
- Peak envelope, hi-res 1024, normalize, no floor, zero-guard → Task 3. ✓
- `WAVEFORM_BARS` 48→64; delete `samples(id)` → Task 4 (3a). ✓
- Cache at decode, pre-volume, by id → Task 4 (3c). ✓
- View reads cached envelope, flat baseline fallback → Task 4 (3b). ✓
- Error tables (zero/unknown duration, non-monotonic clock, empty/silent PCM, missing envelope) → covered by `extrapolate` zero-guard + `saturating_duration_since` (Task 1) and `from_samples`/`bars` guards + flat-baseline fallback (Tasks 3–4). ✓
- Out of scope (trim, async compute, eviction) → untouched. ✓

**Placeholder scan:** no TBD/TODO/"handle edge cases"; every code step shows full code. ✓

**Type consistency:** `PlayheadClock::{new,on_progress,display}` signatures identical across Tasks 1–2; `display_progress: f32` written (3f/3e) and read (3j/3k); `Envelope::{from_samples,bars}` + `ENVELOPE_BUCKETS` identical across Tasks 3–4; `view_now_playing` 6-arg signature matches its call site; `WaveformProgram.samples: Vec<f32>` consistent with `env.bars(...) -> Vec<f32>`. ✓
