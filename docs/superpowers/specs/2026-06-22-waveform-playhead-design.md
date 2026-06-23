# Smooth Playhead + Real Waveform Envelope

**Issue:** TBD (filed at implementation — PR-A: smooth playhead; PR-B: real waveform envelope)
**Date:** 2026-06-22
**Status:** Approved

## Problem

The now-playing waveform has two defects:

1. **The playhead is ~10 fps.** The playhead position is `HonkHonk::progress`, which only advances when an `AudioEvent::Progress` arrives. That event is emitted by a PipeWire loop timer every 100 ms (`engine.rs:311`), and the only steady UI repaint driver is `TrayPoll`, also every 100 ms (`app.rs:1201`). There is no frame/vsync subscription. The line therefore snaps forward in ~10 discrete jumps per second. The `canvas::Cache` from #137 already avoids re-tessellating the bars — the bottleneck is purely how often `progress` changes and how often we repaint.

2. **The waveform is fake.** `waveform::samples(id)` (`waveform.rs:27`) hashes the sound *id string* into 48 pseudo-random bar heights in `0.15..=1.0`. It is stable per sound but bears no relation to the audio. This blocks any usable trim function, which needs the real amplitude envelope to map a screen position to a sample offset.

## Scope

Two independent, sequenced PRs from one spec. Trim itself is explicitly future work (PR-C, not designed here); PR-B only lays the data groundwork.

- **PR-A — Smooth playhead:** new `PlayheadClock`, a per-frame redraw subscription, and the interpolation glue.
- **PR-B — Real waveform envelope:** new `Envelope` extraction + per-sound cache, replacing the hash placeholder as the now-playing bars' data source.

Both add small new modules; `app.rs` (already 2,598 lines — over its 400-line budget, do not grow its logic) gains only a few lines of glue. No logic lands in `app.rs`.

---

## PR-A — Smooth playhead

### Approach: predict-and-correct

Keep the authoritative 10 Hz `AudioEvent::Progress` as the *anchor* of truth. Repaint at frame rate via Iced's per-frame subscription and *extrapolate* the displayed line position from wall-clock elapsed since the last anchor. Every anchor snaps the line back to truth, so it cannot drift. The cached bars are untouched — only the thin overlay line moves per frame.

**Render driver:** `iced::window::frames()` (vsync-paced; "let it fly" at native refresh — simpler and more idiomatic than a manual fps cap, and vsync means it can never busy-loop even on the software renderer). Subscribed **only while a sound is playing** (`playing.is_some()`) — when idle there is zero frame subscription, so a tray-resident app never repaints in the background. This is not an fps cap; it is "do not animate when nothing is moving."

> Planning note: the exact Iced 0.14 helper name/signature for the per-frame subscription will be confirmed in the implementation plan (candidates: `iced::window::frames()`). `iced::time::every(Duration::from_millis(16))` is the fallback if the helper differs; the rest of the design is identical either way since both yield an `Instant` per tick.

### Components

#### `src/ui/playhead.rs` (new, ~80 LOC)

```rust
use std::time::{Duration, Instant};

/// Predict-and-correct clock for the playhead. The 10 Hz audio `Progress`
/// events are the authoritative anchor; `display` extrapolates between them
/// from wall-clock elapsed so the line moves at frame rate without drifting.
pub struct PlayheadClock {
    anchor: f32,        // authoritative progress 0..=1 at `anchor_at`
    anchor_at: Instant,
    duration: Duration, // total sound length; derived from decoded PCM
}

impl PlayheadClock {
    pub fn new(duration: Duration, now: Instant) -> Self { /* anchor 0 */ }

    /// Re-anchor to an authoritative progress sample (snaps out drift).
    pub fn on_progress(&mut self, progress: f32, now: Instant) { /* ... */ }

    /// Extrapolated display progress, clamped to 0..=1.
    pub fn display(&self, now: Instant) -> f32 {
        extrapolate(self.anchor, now.saturating_duration_since(self.anchor_at), self.duration)
    }
}

/// Pure core (unit-tested without instants). Zero duration → returns anchor.
fn extrapolate(anchor: f32, elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return anchor.clamp(0.0, 1.0);
    }
    (anchor + elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}
```

`anchor_at` uses `Instant`; `display` uses `saturating_duration_since` so a non-monotonic `now` can never produce a negative elapsed.

#### `src/app.rs` (glue only)

**New state on `HonkHonk`:**
```rust
playhead: Option<PlayheadClock>, // Some while a sound is playing
display_progress: f32,           // smooth value fed to the view + cache sync
```
`self.progress` stays the authoritative anchor (existing tests untouched).

**New `Message` variant:**
```rust
Frame(std::time::Instant),
```

**Wiring:**
| Trigger | Action |
|---|---|
| `play_sound_entry` (has decoded PCM) | `duration = samples_len / channels / sample_rate`; `playhead = Some(PlayheadClock::new(duration, Instant::now()))` |
| `AudioEvent::Progress(p)` | `self.progress = p` (unchanged) **and** `playhead.on_progress(p, Instant::now())` |
| `AudioEvent::PlaybackFinished` / stop | `playhead = None`; `display_progress = 0.0` |
| `Message::Frame(now)` | `display_progress = playhead.display(now)` (no-op if `None`) |

**Duration source:** derived from the decoded PCM at play time (`decoded.samples.len() / channels / sample_rate`) — exact and always available, avoiding any dependency on the lazy duration-scan (`duration_ms` may still be `None` when a sound is first played).

**`subscription()` change:** when `self.playing.is_some()`, push the per-frame subscription mapped to `Message::Frame`.

**View + cache sync:** `view_now_playing(...)` and the `now_playing.sync(...)` call switch from `self.progress` to `self.display_progress`. The bars still bucket to `PROGRESS_BUCKETS` steps (cache clears ≤ bar-count times per sound — unaffected); only the overlay line reads the smooth value. The line may sit slightly ahead of the coarse bar-fill boundary between bucket steps — expected; the line is the precise indicator.

### Error handling (PR-A)

| Scenario | Behavior |
|---|---|
| Zero / unknown duration | `extrapolate` returns the raw anchor — line steps at 10 Hz, never `NaN` |
| Non-monotonic clock | `saturating_duration_since` → 0 elapsed, no backwards jump |
| Progress event for a replaced sound | Existing `#111` guard already drops stale events before they reach the clock |

No new `Result` crosses a module boundary; nothing to `.unwrap()`.

### Testing (PR-A)

`src/ui/playhead.rs` (pure, no renderer):
- `extrapolate` clamps to `[0, 1]`; reaches exactly `1.0` at/after end.
- `extrapolate` with zero duration returns the anchor.
- `display` is monotonic non-decreasing within a single anchor as `now` advances.
- `on_progress` re-anchors: a later authoritative sample snaps `display` back to truth (including a *backwards* correction if the prediction overshot).
- Instants are synthesized from one `Instant::now()` base plus `Duration::from_millis(..)` — fully deterministic.

No Iced view/subscription tests (per CLAUDE.md).

---

## PR-B — Real waveform envelope

### Approach

Replace the hash placeholder with a real **peak envelope** extracted from the decoded PCM once per sound, cached by id. Store hi-res (~1024 buckets); the now-playing strip downsamples (max-pool) to its display bar count. No artificial height floor — real silence reads flat, which is exactly what a trim function needs.

### Components

#### `src/audio/envelope.rs` (new, ~120 LOC)

```rust
/// Normalized peak amplitude envelope of a decoded sound, 0.0..=1.0.
/// Hi-res (`peaks.len() == buckets`); the view downsamples via `bars`.
pub struct Envelope {
    peaks: Vec<f32>,
}

impl Envelope {
    /// Mono-sum interleaved `samples`, chunk into `buckets` groups, take
    /// peak |sample| per group, normalize by the global max. Silent or empty
    /// input → all-zero peaks (no divide-by-zero).
    pub fn from_samples(samples: &[f32], channels: u16, buckets: usize) -> Self { /* ... */ }

    /// Max-pool the hi-res peaks down to `n` display bars. `n >= peaks.len()`
    /// returns the peaks as-is (padded if needed). Never panics.
    pub fn bars(&self, n: usize) -> Vec<f32> { /* ... */ }
}

/// Hi-res bucket count stored per sound.
pub const ENVELOPE_BUCKETS: usize = 1024;
```

- Mono-sum: average the `channels` interleaved lanes per frame.
- Peak per bucket: `max(|s|)` over the bucket's frames (punchy, "waveformy"; RMS rejected as too smooth for a soundboard).
- Normalize by global max with a zero-guard (`max <= EPSILON` → all zeros).

#### `src/ui/waveform.rs`

- Bump `WAVEFORM_BARS` 48 → 64 (denser, still downsampled from hi-res).
- **Delete `samples(id)`** (the hash placeholder) and its tests.
- `RenderKey { id, bucket }` is **unchanged** — an id's envelope is immutable, so keying the cache on `id` stays correct.

#### `src/app.rs` (glue only)

**New state:**
```rust
waveform_cache: std::collections::HashMap<String, std::sync::Arc<Envelope>>,
```

In `play_sound_entry`, after decode and **before** the per-sound volume multiply (the waveform must not shift with the volume slider), compute `Envelope::from_samples(&decoded.samples, decoded.channels, ENVELOPE_BUCKETS)` and insert by `sound.id` (idempotent — skip if already cached). ~4 KB/sound; session-lifetime cache, no eviction (YAGNI).

#### `src/ui/now_playing.rs`

`view_now_playing` / `view_waveform` read the cached envelope for the playing id and feed `envelope.bars(WAVEFORM_BARS)` into `WaveformProgram` (its `samples` field becomes a `Vec<f32>` instead of `[f32; 48]`). If no envelope is cached yet, render a flat baseline — never fake bars. (For the now-playing sound the envelope is always present, since it is computed synchronously at play-start before the first repaint.)

### Error handling (PR-B)

| Scenario | Behavior |
|---|---|
| Empty / silent PCM | `from_samples` → all-zero peaks; view draws a flat baseline |
| Single-channel vs multi-channel | Mono-sum handles any `channels >= 1` |
| Envelope not yet cached | View falls back to flat baseline (no panic, no fake bars) |
| `buckets == 0` (never passed in prod) | Guarded: returns empty peaks; `bars` returns zeros |

No new `Result` crosses a module boundary.

### Testing (PR-B)

`src/audio/envelope.rs` (pure, no renderer):
- Constant full-amplitude input → all peaks `1.0` after normalize.
- Silence (all zeros) → all peaks `0.0` (no `NaN`/divide-by-zero).
- A half-amplitude region beside a full-amplitude region preserves the `0.5` ratio after normalize.
- Two-channel interleaved input mono-sums correctly (e.g. L full / R silent → half).
- `peaks.len() == buckets`.
- `bars(n)` max-pools correctly for `n < buckets`; `n >= buckets` returns peaks unchanged/padded.
- Empty input → empty/zero peaks; `bars` still returns `n` finite values.

No Iced view tests (per CLAUDE.md).

---

## Out of scope

- **Trim function** (drag start/end handles in the right-click editor) — future PR-C; consumes PR-B's hi-res envelope.
- Async/off-thread envelope computation — synchronous at decode is fine for short soundboard clips; revisit only if a long file hitches.
- Envelope eviction / LRU cache — session-lifetime map is negligible.
- Waveform on grid tiles or in the slot sidebar — now-playing strip only.
- Pause/seek interpolation — no pause/seek feature exists yet; the clock re-anchors on the next `Progress` either way.
