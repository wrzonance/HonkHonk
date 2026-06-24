# Async Decode + PCM/Waveform Caching — Design (#151)

**Status:** Approved 2026-06-23.

**Prerequisite:** Builds on the `play_generation` token added in #149 (PR #150,
must merge to `main` first). Slice 1 keys its async decode on that counter.

## Goal

Make firing playback responsive regardless of audio-file length, and stop
redoing length-scaling work on every play — in a memory-conscious way.

## Motivation

`play_sound_entry` (`src/app.rs`) calls `crate::audio::decode()` **synchronously
inside Iced's `update()` loop** on every play, decoding the entire file to PCM
(`decoder.rs:decode_packets`). That cost scales with file length and blocks the
UI thread — the subtle, length-dependent click-to-fire lag. The same hot path
also: regenerates the in-memory-only waveform envelope (lost every restart),
reads the playhead length from `decoded.duration` (only available because we
fully decoded), and copies the whole PCM with an O(n) `map().collect()` to apply
per-sound volume.

The **render** side is already optimal (persistent `canvas::Cache`, ADR-009 /
#137). This work is entirely on the **click → decode → fire** path.

## Non-Goals (designed-around, not built here)

- **Mix / overlap playback** ("mix vs. stop-and-retrigger" setting) — the engine
  stays single-voice (replace). The cache + decode pipeline are voice-model
  agnostic so a future multi-voice engine can reuse them.
- **Content-hash cache keys** — kept as a forward-compatible header field.
- **Eager whole-library pre-generation** of envelopes.
- **Trimming** — consumes the persisted envelope later.

## Architecture

Three new units; `app.rs` (a known 2,888-line violation) only delegates and
gains a thin `Message::Decoded` arm.

### `audio::store::AudioStore` (new module)

In-memory caches, owned by `HonkHonk`:

- **PCM LRU** — `id -> Arc<CachedPcm>` where
  `CachedPcm { samples: Arc<Vec<f32>>, sample_rate: u32, channels: u16, duration: Duration }`.
  Bounded by **total bytes** (`samples.len() * 4`), default cap **256 MB**
  (`const`, tunable); least-recently-used evicted on insert past the cap. A
  freshly played sound is most-recently-used, so a rapidly re-fired sound —
  including a long one — stays resident (instant re-fire), while cold long
  sounds evict. Eviction never affects playback: the engine holds its own `Arc`.
- **Envelope cache** — `id -> Arc<Envelope>`, populated lazily from disk (B) or
  from a fresh decode.

API (all pure, unit-testable): `get_pcm`, `insert_pcm` (returns evicted ids for
logging/tests), `touch` (LRU bump on hit), `envelope`, `insert_envelope`.

### Decode pipeline (`audio::decode` + an `iced::Task`)

A background decode launched with `Task::perform`. `decode()` is unchanged but
runs off the UI thread. Result returns as
`Message::Decoded { generation, id, result: Result<CachedPcm, AudioError-as-String> }`.
(Errors cross the task boundary as a display string; the app logs + surfaces via
the existing `AudioEvent::Error` channel.)

### `audio::waveform_store` (new module — slice 2 / B)

Per-sound disk persistence under `$XDG_DATA_HOME/honkhonk/waveforms/<id>.bin`:

```
[ magic: b"HHWF" ][ version: u32 ][ buckets: u32 ]
[ size: u64 ][ mtime_secs: i64 ]
[ peaks: f32 little-endian * buckets ]      // ~4 KB at buckets = 1024
```

- `Fingerprint { size: u64, mtime_secs: i64 }` from `std::fs::metadata`.
- `load(dir, id, current_fingerprint) -> Option<Envelope>` — `None` if missing,
  wrong magic/version, or fingerprint mismatch (so a file re-exported/trimmed in
  place at the same path is treated as absent and regenerated).
- `store(dir, id, fingerprint, &Envelope) -> io::Result<()>` — atomic write
  (temp + rename).

The header is **versioned**: a future content-hash key adds a field and bumps
`version`. The intended future flow (per project steer) is *cheap stat pre-check
(path/size/mtime) → re-hash bytes only when that detects a change* — the
`Fingerprint` is exactly that pre-check.

## Data Flow

**Click → `request_play(sound, stop_before)` (new module fn; app delegates):**

1. Bump `play_generation = g`; set `playing = Some(id)` (instant highlight, #111).
2. **PCM cache hit** → fire synchronously: start `PlayheadClock` from the cached
   `duration`; send `Play(g)` with the shared `Arc`. No decode, no copy.
3. **Miss** → return an `iced::Task` that decodes on a background thread and
   yields `Message::Decoded { g, id, result }`. The tile is highlighted with a
   static playhead (reads as "loading") until it lands.

**`Message::Decoded { g, id, result }`:**

1. If `g != self.play_generation` → **drop** (a newer press superseded this one;
   the #149 token makes async ordering correct — no out-of-order `Play`).
2. `Ok(pcm)` → insert into PCM LRU; ensure the envelope (disk via B, else compute
   from `pcm.samples` and persist); start the playhead from `pcm.duration`; send
   `Play(g)`.
3. `Err(e)` → log; if `g == self.play_generation`, `clear_playback_state()` so
   the optimistic highlight/playhead does not stick (mirrors the #149 invariant).

**Waveform display / startup** → the envelope is loaded **lazily** from disk on
first need (fingerprint-checked) and cached; first play generates-and-persists.
No whole-library decode pass.

## Engine change — per-sound volume moves into the engine

To avoid an O(n) PCM copy per play (which would defeat the cache), `AudioCommand::Play`
gains `volume: f32` (per-sound). `PlaybackState`'s effective gain becomes
`master * per_sound`. The app then caches and sends **one canonical pre-volume
`Arc`** — also what the envelope requires (the waveform must not shift with the
volume slider). Net: zero per-play PCM copies; cache and engine share the `Arc`.

(Verify during implementation exactly where `PlaybackState` applies gain so the
per-sound factor multiplies at the same point as `engine_volume`.)

## Ordering & correctness

- Rapid re-fire of **different** tiles: each press bumps `g`; only the latest
  generation's `Decoded` fires (others dropped). No out-of-order `Play`.
- Rapid re-fire of the **same** tile before its first decode lands: a second
  cold miss may spawn a second decode of the same file (one wasted decode);
  after the first lands it is cached and subsequent fires hit. In-flight
  decode dedup (by id) is a possible later refinement, **not** in this scope —
  if added, `log()` nothing is dropped silently.
- Single-voice replace semantics unchanged; `StopAll`, shortcut
  `stop_before=true`, and the #149/#111 guards all continue to hold.

## File organization

- `src/audio/store.rs` — `AudioStore`, `CachedPcm`, LRU (target < 250 lines).
- `src/audio/waveform_store.rs` — persistence + `Fingerprint` (target < 250).
- `src/audio/mod.rs` — re-exports.
- `src/app.rs` — delegates; new `Message::Decoded` arm + a thin `request_play`
  that calls into `AudioStore`. **No net growth** beyond the handler: existing
  decode/envelope/volume logic moves OUT of `play_sound_entry` into the modules.

## Testing (all default `cargo test`, no `pipewire-test`)

- **AudioStore**: insert/get/touch; LRU eviction order; byte-cap enforcement;
  eviction does not drop the just-inserted entry.
- **waveform_store**: store→load round-trip; fingerprint mismatch → `None`;
  bad magic/version → `None`; atomic write leaves no partial file on error.
- **Async ordering**: feeding `Message::Decoded` with a **stale** generation
  performs no play / no playhead change; with the **current** generation it
  starts the playhead and would dispatch `Play`.
- **Cache-hit path**: a warm `request_play` starts the playhead without invoking
  decode (assert via a seam / no new envelope computation).
- **Engine gain**: `PlaybackState` applies `master * per_sound`.
- #111 and #149 suites stay green.

## Rollout (two sequenced PRs)

1. **Slice 1 — async decode + PCM LRU + engine per-sound volume** (off #149).
   Fixes the lag. `Message::Decoded`, `AudioStore` (PCM half), engine `volume`.
2. **Slice 2 — persisted waveform envelope** (`waveform_store`, lazy load/store,
   `Fingerprint`). Adds restart-survival + the trimming foundation.

Each is one demonstrable change and should land within the ≤500-LOC guideline.
