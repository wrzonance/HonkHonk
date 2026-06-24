# Async Decode + PCM Cache (Slice 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the synchronous full-file decode off Iced's `update()` loop into a generation-keyed background `Task`, backed by a byte-capped LRU decoded-PCM cache, so firing playback never blocks the UI regardless of file length.

**Architecture:** A new pure `audio::store::AudioStore` owns a byte-capped LRU of `Arc<CachedPcm>` plus the in-memory envelope map (subsuming the current `waveform_cache`). `play_sound_entry` becomes `request_play`: a warm cache hit fires synchronously; a miss returns a `Task` that decodes on a blocking thread and yields `Message::Decoded`, which the app applies only if its generation is still current (#149 token). Per-sound volume moves into the engine so the cached canonical PCM `Arc` is shared with the engine without a per-play copy.

**Tech Stack:** Rust, Iced 0.14 (`Task::perform` + `tokio::task::spawn_blocking`), symphonia (`crate::audio::decode`), PipeWire playback.

## Global Constraints

- **Branch off PR #150** (`fix/playhead-same-tile-retrigger`) or `main` after it merges — this plan relies on `play_generation` and `AudioCommand::Play { generation }`.
- **Files ≤ 400 lines; functions ≤ 50 lines.** `src/app.rs` is a known violation (~2,900 lines) — do **not** grow it net; new logic lives in `audio::store`, and `play_sound_entry`'s decode/envelope/volume bodies move OUT into the store + a small handler.
- **No `.unwrap()` / `panic!()` in non-test code.** Errors use `thiserror`/`anyhow` per module; the decode task carries errors across the boundary as a `String`.
- **`cargo clippy -- -D warnings` clean** (cognitive-complexity 10, too-many-arguments 5, too-many-lines 50).
- **TDD:** failing test first; default `cargo test` (no `pipewire-test`) must stay green; ≥80% coverage target.
- **Immutability / no mutation of inputs; many small files.**

---

### Task 1: `AudioStore` — byte-capped PCM LRU + envelope map

**Files:**
- Create: `src/audio/store.rs`
- Modify: `src/audio/mod.rs` (add `mod store;` + re-exports)
- Test: inline `#[cfg(test)] mod tests` in `src/audio/store.rs`

**Interfaces:**
- Produces:
  - `pub struct CachedPcm { pub samples: Arc<Vec<f32>>, pub sample_rate: u32, pub channels: u16, pub duration: Duration }` (derives `Debug, Clone, PartialEq`).
  - `pub const DEFAULT_PCM_CAP_BYTES: usize = 256 * 1024 * 1024;`
  - `pub struct AudioStore` with:
    - `pub fn new(cap_bytes: usize) -> Self`
    - `pub fn get_pcm(&mut self, id: &str) -> Option<Arc<CachedPcm>>` (bumps recency)
    - `pub fn insert_pcm(&mut self, id: String, pcm: Arc<CachedPcm>) -> Vec<String>` (returns evicted ids)
    - `pub fn envelope(&self, id: &str) -> Option<Arc<Envelope>>`
    - `pub fn insert_envelope(&mut self, id: String, env: Arc<Envelope>)`
    - `pub fn pcm_bytes(&self) -> usize` (test/diagnostic)
- Consumes: `crate::audio::Envelope`.

- [ ] **Step 1: Write the failing tests**

Create `src/audio/store.rs` with the module skeleton and tests:

```rust
//! In-memory caches for the playback hot path: a byte-capped LRU of decoded
//! PCM and the per-sound waveform envelope. Pure — no audio I/O. Eviction never
//! affects playback (the engine holds its own `Arc`). See spec #151.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::Envelope;

/// Default LRU cap: ~one long stereo song plus many short clips.
pub const DEFAULT_PCM_CAP_BYTES: usize = 256 * 1024 * 1024;

/// Decoded PCM plus the metadata the engine and playhead need. `samples` is
/// `Arc`-wrapped so cloning (into the engine `Play` command and the cache) is
/// O(1) and a single canonical buffer is shared.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedPcm {
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Duration,
}

impl CachedPcm {
    fn bytes(&self) -> usize {
        self.samples.len() * std::mem::size_of::<f32>()
    }
}

struct PcmEntry {
    pcm: Arc<CachedPcm>,
    last_used: u64,
}

pub struct AudioStore {
    pcm: HashMap<String, PcmEntry>,
    envelopes: HashMap<String, Arc<Envelope>>,
    bytes: usize,
    cap_bytes: usize,
    tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm(n: usize) -> Arc<CachedPcm> {
        Arc::new(CachedPcm {
            samples: Arc::new(vec![0.0_f32; n]),
            sample_rate: 48_000,
            channels: 2,
            duration: Duration::from_secs(1),
        })
    }

    #[test]
    fn insert_then_get_returns_pcm() {
        let mut store = AudioStore::new(1024);
        assert!(store.insert_pcm("a".into(), pcm(4)).is_empty());
        assert_eq!(store.get_pcm("a"), Some(pcm(4)));
        assert_eq!(store.get_pcm("missing"), None);
    }

    #[test]
    fn insert_past_cap_evicts_least_recently_used() {
        // cap = 32 bytes = 8 f32. Each pcm(4) = 16 bytes. Two fit; a third evicts.
        let mut store = AudioStore::new(32);
        store.insert_pcm("a".into(), pcm(4));
        store.insert_pcm("b".into(), pcm(4));
        // Touch "a" so "b" is now least-recently-used.
        let _ = store.get_pcm("a");
        let evicted = store.insert_pcm("c".into(), pcm(4));
        assert_eq!(evicted, vec!["b".to_string()]);
        assert!(store.get_pcm("b").is_none());
        assert!(store.get_pcm("a").is_some());
        assert!(store.get_pcm("c").is_some());
        assert!(store.pcm_bytes() <= 32);
    }

    #[test]
    fn single_entry_larger_than_cap_is_kept() {
        let mut store = AudioStore::new(8);
        let evicted = store.insert_pcm("big".into(), pcm(100)); // 400 bytes > cap
        assert!(evicted.is_empty(), "a lone oversized entry must not evict itself");
        assert!(store.get_pcm("big").is_some());
    }

    #[test]
    fn reinserting_same_id_replaces_without_double_counting() {
        let mut store = AudioStore::new(1024);
        store.insert_pcm("a".into(), pcm(4));
        store.insert_pcm("a".into(), pcm(8));
        assert_eq!(store.pcm_bytes(), 8 * std::mem::size_of::<f32>());
        assert_eq!(store.get_pcm("a").map(|p| p.samples.len()), Some(8));
    }

    #[test]
    fn envelope_round_trips() {
        let mut store = AudioStore::new(1024);
        assert!(store.envelope("a").is_none());
        let env = Arc::new(Envelope::from_samples(&[0.5_f32; 64], 1, 16));
        store.insert_envelope("a".into(), env.clone());
        assert_eq!(store.envelope("a"), Some(env));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib audio::store`
Expected: FAIL — `AudioStore::new` / methods not found (won't compile).

- [ ] **Step 3: Implement `AudioStore`**

Add to `src/audio/store.rs` (above the `#[cfg(test)]` block):

```rust
impl AudioStore {
    pub fn new(cap_bytes: usize) -> Self {
        Self {
            pcm: HashMap::new(),
            envelopes: HashMap::new(),
            bytes: 0,
            cap_bytes,
            tick: 0,
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.wrapping_add(1);
        self.tick
    }

    pub fn get_pcm(&mut self, id: &str) -> Option<Arc<CachedPcm>> {
        let tick = self.next_tick();
        let entry = self.pcm.get_mut(id)?;
        entry.last_used = tick;
        Some(Arc::clone(&entry.pcm))
    }

    pub fn insert_pcm(&mut self, id: String, pcm: Arc<CachedPcm>) -> Vec<String> {
        let tick = self.next_tick();
        let new_bytes = pcm.bytes();
        if let Some(old) = self.pcm.insert(
            id,
            PcmEntry {
                pcm,
                last_used: tick,
            },
        ) {
            self.bytes -= old.pcm.bytes();
        }
        self.bytes += new_bytes;
        self.evict_to_cap()
    }

    /// Evicts least-recently-used entries until at or below the cap. A single
    /// entry larger than the cap is kept (evicting it would free nothing useful
    /// and stop a legitimately-requested sound from playing).
    fn evict_to_cap(&mut self) -> Vec<String> {
        let mut evicted = Vec::new();
        while self.bytes > self.cap_bytes && self.pcm.len() > 1 {
            let Some(victim) = self
                .pcm
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            if let Some(entry) = self.pcm.remove(&victim) {
                self.bytes -= entry.pcm.bytes();
            }
            evicted.push(victim);
        }
        evicted
    }

    pub fn envelope(&self, id: &str) -> Option<Arc<Envelope>> {
        self.envelopes.get(id).map(Arc::clone)
    }

    pub fn insert_envelope(&mut self, id: String, env: Arc<Envelope>) {
        self.envelopes.insert(id, env);
    }

    pub fn pcm_bytes(&self) -> usize {
        self.bytes
    }
}
```

- [ ] **Step 4: Wire the module**

In `src/audio/mod.rs`, add `mod store;` (after `mod registry;`) and a re-export line:

```rust
pub use store::{AudioStore, CachedPcm, DEFAULT_PCM_CAP_BYTES};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib audio::store` then `cargo clippy --all-targets -- -D warnings`
Expected: PASS; clippy clean.

- [ ] **Step 6: Commit**

```bash
git add src/audio/store.rs src/audio/mod.rs
git commit -m "feat(audio): byte-capped LRU PCM cache + envelope store (#151)"
```

---

### Task 2: Per-sound volume in the engine (share the canonical PCM `Arc`)

**Files:**
- Modify: `src/audio/playback.rs` (PlaybackState gain), `src/audio/engine.rs` (`AudioCommand::Play { volume }`, `handle_play` threading)
- Test: inline tests in `src/audio/playback.rs`; integration `Play` sites in `tests/pipewire_integration.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `PlaybackState::start(&mut self, sound_id: String, samples: Arc<Vec<f32>>, sample_rate: u32, channels: u16, gain: f32)` — adds the trailing `gain` (per-sound) param.
  - `AudioCommand::Play { sound_id, samples, sample_rate, channels, generation, volume: f32 }` — adds `volume` (per-sound).

- [ ] **Step 1: Write the failing test (PlaybackState gain)**

Add to the `#[cfg(test)] mod tests` in `src/audio/playback.rs`:

```rust
#[test]
fn fill_buffer_multiplies_master_and_per_sound_gain() {
    // master 0.5 (with_volume) * per-sound gain 0.5 = 0.25 effective.
    let samples = Arc::new(vec![1.0_f32; 100]);
    let mut state = PlaybackState::with_volume(0.5);
    state.start("test".into(), samples, 48_000, 1, 0.5);

    let mut buf = vec![0.0_f32; 10];
    let wrote = state.fill_buffer(&mut buf);

    assert_eq!(wrote, 10);
    for &s in &buf[..wrote] {
        assert!((s - 0.25).abs() < f32::EPSILON, "expected 0.25, got {s}");
    }
}
```

Also update the existing `start(...)` calls in this file's tests to pass a trailing `1.0` gain (e.g. `state.start("test".into(), samples, 48000, 2, 1.0);` in `progress_at_start_is_zero`, `progress_at_midpoint`, `progress_at_end_is_one`, `fill_buffer_respects_initial_volume`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib audio::playback`
Expected: FAIL — `start` takes 4 args, not 5 (won't compile).

- [ ] **Step 3: Implement the gain field**

In `src/audio/playback.rs`, add the field and thread it:

```rust
pub struct PlaybackState {
    sound_id: Option<String>,
    samples: Option<Arc<Vec<f32>>>,
    cursor: usize,
    volume: f32,
    gain: f32,
    sample_rate: u32,
    channels: u16,
    active: bool,
}
```

In `new()` add `gain: 1.0,`. Change `start`:

```rust
    pub fn start(
        &mut self,
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
        gain: f32,
    ) {
        self.sound_id = Some(sound_id);
        self.samples = Some(samples);
        self.cursor = 0;
        self.gain = gain.clamp(0.0, 1.0);
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.active = true;
    }
```

In `fill_buffer`, change the multiply:

```rust
        let g = self.volume * self.gain;
        for (dst, &sample) in buf[..to_write].iter_mut().zip(src.iter()) {
            *dst = sample * g;
        }
```

- [ ] **Step 4: Add `volume` to `AudioCommand::Play` and thread it through `handle_play`**

In `src/audio/engine.rs`, add to the `Play` variant (after `generation`):

```rust
        /// Per-sound volume multiplier, applied alongside the master volume in
        /// `PlaybackState`. Lets the app send the canonical (pre-volume) PCM Arc
        /// without an O(n) copy per play (#151).
        volume: f32,
```

In the dispatch arm, destructure `volume` and pass it into `PlayRequest` (add a `volume: f32` field to `PlayRequest`). In `handle_play`, destructure `volume` from `PlayRequest` and pass it as the trailing `gain` to **both** `sink_state.borrow_mut().start(...)` and `mon_state.borrow_mut().start(...)`.

- [ ] **Step 5: Update integration `Play` sites**

In `tests/pipewire_integration.rs`, add `volume: 1.0,` to each `AudioCommand::Play { ... }` (the 5 sites at the `sound_id`/`samples`/… blocks).

- [ ] **Step 6: Run tests + clippy**

Run: `cargo test --lib audio::playback` then `cargo test` then `cargo test --features pipewire-test --no-run` then `cargo clippy --all-targets --features pipewire-test -- -D warnings`
Expected: PASS / compiles / clippy clean.

- [ ] **Step 7: Commit**

```bash
git add src/audio/playback.rs src/audio/engine.rs tests/pipewire_integration.rs
git commit -m "feat(audio): per-sound volume as engine gain (#151)"
```

---

### Task 3: Async decode wiring + `AudioStore` integration in the app

**Files:**
- Modify: `src/app.rs` (replace `waveform_cache` field with `audio_store`; add `Message::Decoded`; refactor `play_sound_entry` → `request_play` + `start_playback`; remove the synchronous decode), `src/ui/now_playing.rs` callers unchanged (still receive `Option<&Envelope>`).
- Test: inline tests in `src/app.rs`.

**Interfaces:**
- Consumes: `audio::{AudioStore, CachedPcm, DEFAULT_PCM_CAP_BYTES}`; `AudioCommand::Play { …, generation, volume }`; the existing `play_generation`.
- Produces:
  - `Message::Decoded { generation: u64, id: String, result: Result<CachedPcm, String> }`.
  - `HonkHonk::request_play(&mut self, sound: &SoundEntry, stop_before: bool) -> Task<Message>` (replaces `play_sound_entry`'s body; callers in `Message::PlaySound` and `Message::ShortcutActivated` return its `Task`).
  - `HonkHonk::start_playback(&mut self, id: &str, pcm: Arc<CachedPcm>, volume: f32, generation: u64)` — sets the playhead from `pcm.duration`, ensures the envelope, sends `Play`.

- [ ] **Step 1: Write the failing tests**

Add to `#[cfg(test)] mod tests` in `src/app.rs`:

```rust
#[test]
fn stale_decoded_is_dropped() {
    // A Decoded carrying an older generation than the current play must not
    // start a playhead or change `playing` (a newer press superseded it, #149/#151).
    let mut app = HonkHonk::new_for_test();
    let (handle, _evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);
    app.play_generation = 5;
    app.playing = Some("newer".into());

    let pcm = std::sync::Arc::new(crate::audio::CachedPcm {
        samples: std::sync::Arc::new(vec![0.0_f32; 8]),
        sample_rate: 48_000,
        channels: 2,
        duration: std::time::Duration::from_secs(1),
    });
    let _ = app.update(Message::Decoded {
        generation: 4,
        id: "older".into(),
        result: Ok((*pcm).clone()),
    });

    assert!(app.playhead.is_none(), "stale decode must not start a playhead");
    assert_eq!(app.playing(), Some("newer"));
}

#[test]
fn current_decoded_starts_playhead_and_caches_pcm() {
    let mut app = HonkHonk::new_for_test();
    let (handle, _evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);
    app.play_generation = 2;
    app.playing = Some("snd".into());

    let pcm = crate::audio::CachedPcm {
        samples: std::sync::Arc::new(vec![0.25_f32; 64]),
        sample_rate: 48_000,
        channels: 2,
        duration: std::time::Duration::from_secs(3),
    };
    let _ = app.update(Message::Decoded {
        generation: 2,
        id: "snd".into(),
        result: Ok(pcm),
    });

    assert!(app.playhead.is_some(), "current decode must start the playhead");
    assert!(
        app.audio_store.get_pcm("snd").is_some(),
        "decode result must be cached for instant re-fire"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::tests::stale_decoded_is_dropped app::tests::current_decoded_starts_playhead_and_caches_pcm`
Expected: FAIL — `Message::Decoded`, `app.audio_store`, `CachedPcm` not found (won't compile).

- [ ] **Step 3: Replace the `waveform_cache` field with `audio_store`**

In the `HonkHonk` struct, replace:

```rust
    waveform_cache: std::collections::HashMap<String, std::sync::Arc<crate::audio::Envelope>>,
```

with:

```rust
    /// Hot-path caches: byte-capped decoded-PCM LRU + waveform envelope map
    /// (#151). Subsumes the former `waveform_cache`.
    audio_store: crate::audio::AudioStore,
```

In **both** constructors (`new` and `new_for_test`), replace the `waveform_cache: …HashMap::new(),` initializer with:

```rust
            audio_store: crate::audio::AudioStore::new(crate::audio::DEFAULT_PCM_CAP_BYTES),
```

- [ ] **Step 4: Add the `Message::Decoded` variant**

In `enum Message`, add:

```rust
    /// A background decode completed for play generation `generation`. Applied
    /// only if still the current generation (#149/#151).
    Decoded {
        generation: u64,
        id: String,
        result: Result<crate::audio::CachedPcm, String>,
    },
```

- [ ] **Step 5: Refactor `play_sound_entry` → `request_play` + `start_playback` + `Decoded` handler**

Replace the whole `play_sound_entry` method with:

```rust
    /// Begins playing `sound`. A warm PCM cache hit fires synchronously; a miss
    /// returns a `Task` that decodes off the UI thread and yields
    /// `Message::Decoded`. The play generation is bumped here so a stale decode
    /// (superseded by a newer press) is dropped on arrival (#149/#151).
    fn request_play(&mut self, sound: &SoundEntry, stop_before: bool) -> Task<Message> {
        self.play_generation = self.play_generation.wrapping_add(1);
        let generation = self.play_generation;
        self.playing = Some(sound.id.clone());
        if stop_before {
            if let Some(ref audio) = self.audio {
                audio.send(AudioCommand::Stop);
            }
        }
        if let Some(pcm) = self.audio_store.get_pcm(&sound.id) {
            self.start_playback(&sound.id, pcm, self.sound_meta.volume_for(&sound.id), generation);
            return Task::none();
        }
        let id = sound.id.clone();
        let path = sound.path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || crate::audio::decode(&path))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.map_err(|e| e.to_string()))
                    .map(|d| crate::audio::CachedPcm {
                        samples: std::sync::Arc::new(d.samples),
                        sample_rate: d.sample_rate,
                        channels: d.channels,
                        duration: d.duration,
                    })
            },
            move |result| Message::Decoded {
                generation,
                id: id.clone(),
                result,
            },
        )
    }

    /// Starts the playhead from the decoded duration, ensures the waveform
    /// envelope is cached, and dispatches the engine `Play`. Shared by the warm
    /// cache-hit path and the `Decoded` handler.
    fn start_playback(
        &mut self,
        id: &str,
        pcm: std::sync::Arc<crate::audio::CachedPcm>,
        volume: f32,
        generation: u64,
    ) {
        self.playhead = Some(crate::ui::playhead::PlayheadClock::new(
            pcm.duration,
            Instant::now(),
        ));
        self.display_progress = 0.0;
        if self.audio_store.envelope(id).is_none() {
            let env = std::sync::Arc::new(crate::audio::Envelope::from_samples(
                &pcm.samples,
                pcm.channels,
                crate::audio::ENVELOPE_BUCKETS,
            ));
            self.audio_store.insert_envelope(id.to_string(), env);
        }
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::Play {
                sound_id: id.to_string(),
                samples: std::sync::Arc::clone(&pcm.samples),
                sample_rate: pcm.sample_rate,
                channels: pcm.channels,
                generation,
                volume,
            });
            self.playing = Some(id.to_string());
        }
    }
```

Add the `Decoded` arm in `update`:

```rust
            Message::Decoded {
                generation,
                id,
                result,
            } => {
                if generation != self.play_generation {
                    return Task::none();
                }
                match result {
                    Ok(pcm) => {
                        let volume = self.sound_meta.volume_for(&id);
                        let pcm = std::sync::Arc::new(pcm);
                        self.audio_store.insert_pcm(id.clone(), std::sync::Arc::clone(&pcm));
                        self.start_playback(&id, pcm, volume, generation);
                    }
                    Err(e) => {
                        eprintln!("honkhonk: decode error: {e}");
                        self.clear_playback_state();
                    }
                }
                Task::none()
            }
```

- [ ] **Step 6: Update the two callers to return the `Task`**

In `Message::PlaySound`:

```rust
            Message::PlaySound(sound_id) => {
                if let Some(sound) = self.sounds.iter().find(|s| s.id == sound_id).cloned() {
                    self.request_play(&sound, false)
                } else {
                    Task::none()
                }
            }
```

In `Message::ShortcutActivated`, replace `self.play_sound_entry(&sound, true);` with `return self.request_play(&sound, true);` (keep the surrounding stale-slot `else` branch returning `Task::none()`).

- [ ] **Step 7: Point the now-playing view at the store**

Where `view` builds `envelope` for `now_playing::view_now_playing`, replace the `self.waveform_cache.get(...)` lookup with `self.audio_store.envelope(playing_sound_id)` (the view takes `Option<&Envelope>`; call `.as_deref()` on the `Arc` as needed). Update the test `playing_a_sound_caches_its_waveform_envelope` to assert via `app.audio_store.envelope(...)` and to drive playback through `request_play` + a `Message::Decoded` (no synchronous decode remains).

- [ ] **Step 8: Run the full suite + clippy + fmt**

Run: `cargo test` then `cargo test --features pipewire-test --no-run` then `cargo clippy --all-targets --features pipewire-test -- -D warnings` then `cargo fmt --all --check`
Expected: all green; #111/#149 suites still pass.

- [ ] **Step 9: Commit**

```bash
git add src/app.rs src/ui/now_playing.rs
git commit -m "feat(audio): async decode off the UI thread via AudioStore (#151)"
```

---

## Self-Review

**Spec coverage:**
- Async decode off UI thread → Task 3 (`request_play` + `Task::perform`). ✓
- Generation-keyed stale-drop → Task 3 (`Decoded` guard) + `stale_decoded_is_dropped` test. ✓
- Byte-capped LRU PCM cache → Task 1. ✓
- Per-sound volume in engine / shared `Arc` → Task 2. ✓
- Envelope cache subsumed into `AudioStore` → Task 1 + Task 3 migration. ✓
- *Persisted* envelope (disk), `Fingerprint`, lazy load → **Slice 2** (separate plan), explicitly out of this plan. Noted.
- Cache-hit fires without decode → `current_decoded_starts_playhead_and_caches_pcm` + the warm-path branch (a dedicated "no decode on warm hit" assertion can be added when a decode seam exists; the warm branch returns `Task::none()` and never constructs a decode `Task`).

**Placeholder scan:** none — every code step shows full code. The only deferred item (a decode-call-count seam) is explicitly described, not a silent TODO.

**Type consistency:** `CachedPcm { samples: Arc<Vec<f32>>, sample_rate, channels, duration }` is identical across Tasks 1/3; `start(.., gain)` and `Play { .., volume }` match between Task 2 and Task 3's `start_playback`; `Message::Decoded { generation, id, result: Result<CachedPcm, String> }` matches between producer (`request_play`) and consumer (`update`).

## Out of Scope (Slice 2 — separate plan)

Persisted waveform envelope (`audio::waveform_store`, `Fingerprint { size, mtime }`, versioned `.bin` under `$XDG_DATA_HOME/honkhonk/waveforms/`, lazy load on startup). Builds directly on this slice's `AudioStore::insert_envelope` / `envelope`.
