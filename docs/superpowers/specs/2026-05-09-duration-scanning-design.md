# Duration Scanning at Library Load Time

**Issue:** #67  
**Date:** 2026-05-09  
**Status:** Approved

## Problem

`Library::scan` populates `SoundEntry` but leaves `duration_ms: None` for all entries. The now-playing bar shows `—` for every sound's duration. The slot manager sidebar also shows no duration.

## Approach

Background thread (Approach A): `Library::scan` returns entries with `duration_ms: None` instantly. A one-shot Iced subscription offloads file probing to `tokio::task::spawn_blocking`, then emits one `Message::DurationsLoaded(HashMap<String, u64>)` when all probing is complete. App rebuilds `sounds` in one `update()` pass.

Symphonia was considered for probing but rejected: unreliable for VBR MP3 (no Xing header) and OGG Vorbis (requires end-of-stream seek, not just header probe). `lofty` handles all formats correctly via format-specific strategies.

## Data Flow

```
startup
  Library::scan() → Vec<SoundEntry> (duration_ms: None)   ← fast, unchanged
  subscription() fires → spawn_blocking probes all files via lofty
  ~1-3s later → Message::DurationsLoaded(HashMap<String, u64>)
  update() → self.sounds rebuilt with duration_ms filled in
  UI re-renders → now-playing bar + slot sidebar show real M:SS
```

## Components

### `Cargo.toml`

```toml
lofty = "0.24"
```

Justification: symphonia (existing dep) is unreliable for VBR MP3 and OGG without additional seek operations. `lofty` is purpose-built for audio metadata reading, handles all formats correctly with a 3-line API surface.

### `src/state/library.rs`

**New: `probe_duration(path: &Path) -> Option<u64>`**

```rust
fn probe_duration(path: &Path) -> Option<u64> {
    let tagged = lofty::read_from_path(path).ok()?;
    Some(tagged.properties().duration().as_millis() as u64)
}
```

Returns `None` on any error (corrupt file, unsupported codec, zero-length file).

**New: `pub fn probe_durations(pairs: Vec<(String, PathBuf)>) -> HashMap<String, u64>`**

Takes `(id, path)` pairs — owned, `'static`-compatible for `spawn_blocking`. Calls `probe_duration` per path, collects successes. Failed probes silently skipped — those entries keep `duration_ms: None`.

Caller (`duration_scan_sub`) constructs pairs from `self.sounds` before entering async context:
```rust
let pairs: Vec<(String, PathBuf)> = sounds
    .iter()
    .map(|s| (s.id.clone(), s.path.clone()))
    .collect();
```

`Library::scan` is unchanged.

### `src/app.rs`

**New `Message` variant:**
```rust
DurationsLoaded(HashMap<String, u64>),
```

**New state field on `HonkHonk`:**
```rust
durations_loaded: bool,  // false on init
```

**New subscription function:**
```rust
fn duration_scan_sub(pairs: Vec<(String, PathBuf)>) -> impl Stream<Item = Message> {
    iced::stream::channel(1, async move |mut tx| {
        let map = tokio::task::spawn_blocking(move || {
            state::library::probe_durations(pairs)
        })
        .await
        .unwrap_or_default();
        let _ = tx.send(Message::DurationsLoaded(map)).await;
        iced::futures::future::pending::<()>().await;
    })
}
```

**`subscription()` change:**  
Include `duration_scan_sub` when `!self.durations_loaded`. Excluded after first `DurationsLoaded` message.

**`update(DurationsLoaded(map))`:**  
Rebuild `self.sounds`: for each `SoundEntry`, if `map` contains its `id`, set `duration_ms = Some(ms)`. Set `self.durations_loaded = true`.

### `src/ui/now_playing.rs`

**New: `fn fmt_duration(ms: u64) -> String`**  
Formats `M:SS`. Example: `63_000ms → "1:03"`.

`view_sound_info` uses `fmt_duration` when `sound.duration_ms` is `Some`; keeps `—` when `None` (covers the pre-scan window).

### `src/ui/slot_manager.rs`

Bound tile and detail panel use `fmt_duration` for duration display. Function re-exported from `ui/mod.rs` to avoid duplication.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Corrupt / malformed file | `lofty::read_from_path` returns `Err` → `probe_duration` returns `None` → entry keeps `duration_ms: None` |
| Unsupported format | Same as above |
| Zero-length file | Same as above |
| `spawn_blocking` panic | `.unwrap_or_default()` returns empty `HashMap` — all sounds keep `None` |
| File deleted between scan and probe | `lofty` returns `Err` → `None` — silent skip |

No log spam. No panics. No user-visible errors.

## Testing

### `src/state/library.rs`

- `probe_duration` returns `Some(ms)` for valid WAV fixture (tiny synthetic: 44-byte header + silence)
- `probe_duration` returns `None` for empty file
- `probe_duration` returns `None` for non-audio file (e.g. `.txt`)
- `probe_durations` returns correct count for mixed-validity batch (some valid, some corrupt)
- `probe_durations` returns empty map for all-invalid batch

### `src/app.rs`

- `update(DurationsLoaded(map))` fills `duration_ms` for matching sound IDs
- `update(DurationsLoaded(map))` leaves unmatched entries unchanged
- `update(DurationsLoaded(map))` sets `durations_loaded = true`
- Calling `subscription()` after `durations_loaded = true` excludes duration sub

### Test fixtures

Tiny synthetic WAV (44-byte PCM header + 1 frame of silence) generated inline in tests via `tempfile` — no binary fixtures checked in.

## Out of Scope

- Reading artist/album/title tags (future issue)
- Displaying duration on sound grid tiles (future)
- Updating duration if files change on disk (future)
- Progress bar using duration for fraction calculation (already uses `progress: f32` from audio engine — unaffected)
