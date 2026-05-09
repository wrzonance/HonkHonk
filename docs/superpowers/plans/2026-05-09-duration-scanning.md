# Duration Scanning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Probe audio file durations at startup via `lofty` and display real `M:SS` times in the now-playing bar and slot manager sidebar.

**Architecture:** `Library::scan` stays fast (returns `duration_ms: None`). A one-shot Iced subscription fires immediately, offloads lofty probing to `tokio::task::spawn_blocking`, then emits one `Message::DurationsLoaded(HashMap<String, u64>)`. The app rebuilds `self.sounds` in a single `update()` pass. The slot manager sidebar already renders duration — it will show real times automatically once `duration_ms` is populated.

**Tech Stack:** `lofty 0.24` (audio metadata), `tokio` (spawn_blocking), `iced::stream::channel` (subscription pattern matching existing shortcuts stream), `tempfile` (test fixtures, already a dev-dep).

---

## File Map

| File | Change |
|------|--------|
| `Cargo.toml` | Add `lofty = "0.24"`, `tokio = { version = "1", features = ["rt"] }` |
| `src/state/library.rs` | Add `probe_duration`, `probe_durations` (pub) |
| `src/app.rs` | Add `Message::DurationsLoaded`, `durations_loaded: bool` field, `duration_scan_sub`, wire into `subscription()`, handle in `update()` |
| `src/ui/mod.rs` | Add `pub fn fmt_duration(ms: Option<u64>) -> String` |
| `src/ui/slot_manager.rs` | Remove private `fmt_duration` (line 8-11), use `crate::ui::fmt_duration` |
| `src/ui/now_playing.rs` | Use `crate::ui::fmt_duration` in `view_sound_info` subtitle |

---

## Task 1: Add lofty + probe_duration

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/state/library.rs`

- [ ] **Step 1: Add lofty to Cargo.toml**

Open `Cargo.toml` and add to `[dependencies]`:
```toml
lofty = "0.24"
```

- [ ] **Step 2: Write failing test for probe_duration**

Add to the `#[cfg(test)]` block in `src/state/library.rs`:

```rust
fn make_wav_1sec() -> Vec<u8> {
    let sample_rate: u32 = 8000;
    let num_samples: u32 = 8000; // 1 second of 8-bit mono PCM
    let data_size = num_samples;
    let mut v = Vec::with_capacity(44 + data_size as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36u32 + data_size).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&sample_rate.to_le_bytes()); // byte_rate
    v.extend_from_slice(&1u16.to_le_bytes()); // block_align
    v.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_size.to_le_bytes());
    v.extend(vec![128u8; data_size as usize]); // silence
    v
}

#[test]
fn probe_duration_returns_some_for_valid_wav() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wav");
    std::fs::write(&path, make_wav_1sec()).unwrap();
    let ms = probe_duration(&path).unwrap();
    assert!((900..=1100).contains(&ms), "expected ~1000ms, got {ms}");
}

#[test]
fn probe_duration_returns_none_for_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.wav");
    std::fs::write(&path, b"").unwrap();
    assert!(probe_duration(&path).is_none());
}

#[test]
fn probe_duration_returns_none_for_non_audio_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("readme.txt");
    std::fs::write(&path, b"hello world").unwrap();
    assert!(probe_duration(&path).is_none());
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test probe_duration 2>&1 | tail -20
```

Expected: compile error — `probe_duration` not defined.

- [ ] **Step 4: Implement probe_duration**

Add to `src/state/library.rs` (before the `Library` struct, after the imports):

```rust
use lofty::prelude::AudioFile;
use lofty::probe::Probe;
```

Add the function (not `pub` — only `probe_durations` is public):

```rust
fn probe_duration(path: &Path) -> Option<u64> {
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    Some(tagged_file.properties().duration().as_millis() as u64)
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test probe_duration 2>&1 | tail -20
```

Expected: 3 tests pass.

- [ ] **Step 6: Run full test suite + clippy**

```bash
cargo test 2>&1 | tail -5
cargo clippy -- -D warnings 2>&1 | tail -10
```

Expected: all pass, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/state/library.rs
git commit -m "feat(state): add probe_duration via lofty (#67)"
```

---

## Task 2: Add probe_durations batch function

**Files:**
- Modify: `src/state/library.rs`

- [ ] **Step 1: Write failing test**

Add to the `#[cfg(test)]` block in `src/state/library.rs`:

```rust
#[test]
fn probe_durations_returns_map_for_valid_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    std::fs::write(&path, make_wav_1sec()).unwrap();
    let pairs = vec![("id-a".to_string(), path.clone())];
    let map = probe_durations(pairs);
    assert_eq!(map.len(), 1);
    assert!((900..=1100).contains(map.get("id-a").unwrap()));
}

#[test]
fn probe_durations_skips_invalid_files() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.wav");
    let bad = dir.path().join("bad.txt");
    std::fs::write(&good, make_wav_1sec()).unwrap();
    std::fs::write(&bad, b"not audio").unwrap();
    let pairs = vec![
        ("good".to_string(), good),
        ("bad".to_string(), bad),
    ];
    let map = probe_durations(pairs);
    assert_eq!(map.len(), 1);
    assert!(map.contains_key("good"));
    assert!(!map.contains_key("bad"));
}

#[test]
fn probe_durations_empty_input_returns_empty_map() {
    let map = probe_durations(vec![]);
    assert!(map.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test probe_durations 2>&1 | tail -20
```

Expected: compile error — `probe_durations` not defined.

- [ ] **Step 3: Implement probe_durations**

Add to `src/state/library.rs` (after `probe_duration`). Add `HashMap` to the existing `use std::path::{Path, PathBuf};` imports section:

```rust
use std::collections::HashMap;
```

Add the function:

```rust
pub fn probe_durations(pairs: Vec<(String, PathBuf)>) -> HashMap<String, u64> {
    pairs
        .into_iter()
        .filter_map(|(id, path)| probe_duration(&path).map(|ms| (id, ms)))
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test probe_durations 2>&1 | tail -20
```

Expected: 3 tests pass.

- [ ] **Step 5: Run full suite + clippy**

```bash
cargo test 2>&1 | tail -5
cargo clippy -- -D warnings 2>&1 | tail -10
```

Expected: all pass, zero warnings.

- [ ] **Step 6: Commit**

```bash
git add src/state/library.rs
git commit -m "feat(state): add probe_durations batch fn (#67)"
```

---

## Task 3: DurationsLoaded message + app state + update handler

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block in `src/app.rs`:

```rust
#[test]
fn durations_loaded_fills_matching_sound_entries() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        crate::state::SoundEntry {
            id: "abc123".into(),
            name: "Honk".into(),
            path: std::path::PathBuf::from("/tmp/honk.wav"),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            category: "Honk".into(),
        },
    ];
    let map = std::collections::HashMap::from([("abc123".to_string(), 1500u64)]);
    let _ = app.update(Message::DurationsLoaded(map));
    assert_eq!(app.sounds[0].duration_ms, Some(1500));
    assert!(app.durations_loaded);
}

#[test]
fn durations_loaded_ignores_unmatched_ids() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        crate::state::SoundEntry {
            id: "abc123".into(),
            name: "Honk".into(),
            path: std::path::PathBuf::from("/tmp/honk.wav"),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            category: "Honk".into(),
        },
    ];
    let map = std::collections::HashMap::from([("no-match".to_string(), 999u64)]);
    let _ = app.update(Message::DurationsLoaded(map));
    assert_eq!(app.sounds[0].duration_ms, None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test durations_loaded 2>&1 | tail -20
```

Expected: compile errors — `Message::DurationsLoaded`, `durations_loaded` field not defined.

- [ ] **Step 3: Add Message variant**

In `src/app.rs`, find the `Message` enum and add after `ShortcutBindingsUpdated`:

```rust
DurationsLoaded(std::collections::HashMap<String, u64>),
```

- [ ] **Step 4: Add durations_loaded field**

In `src/app.rs`, find the `HonkHonk` struct and add after `shortcuts_warning_dismissed`:

```rust
durations_loaded: bool,
```

In `HonkHonk::new`, add to the `Self { ... }` initializer:

```rust
durations_loaded: false,
```

In `HonkHonk::new_for_test`, add to the `Self { ... }` initializer:

```rust
durations_loaded: false,
```

Tests in `src/app.rs` live in `mod tests` inside the same file. Rust allows `mod tests` blocks to access private fields of types in the same module — no `pub(crate)` annotation needed. `app.sounds = vec![...]` and `app.durations_loaded` both compile fine from inside `mod tests { ... }` in `src/app.rs`.

- [ ] **Step 5: Add update arm**

In `pub fn update`, add match arm (place after `ShortcutBindingsUpdated` arm):

```rust
Message::DurationsLoaded(map) => {
    for sound in &mut self.sounds {
        if let Some(&ms) = map.get(&sound.id) {
            sound.duration_ms = Some(ms);
        }
    }
    self.durations_loaded = true;
    Task::none()
}
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test durations_loaded 2>&1 | tail -20
```

Expected: 2 tests pass.

- [ ] **Step 7: Run full suite + clippy**

```bash
cargo test 2>&1 | tail -5
cargo clippy -- -D warnings 2>&1 | tail -10
```

Expected: all pass, zero warnings.

- [ ] **Step 8: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): DurationsLoaded message + update handler (#67)"
```

---

## Task 4: Wire duration scan subscription

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app.rs`

- [ ] **Step 1: Add tokio to Cargo.toml**

Add to `[dependencies]` in `Cargo.toml`:

```toml
tokio = { version = "1", features = ["rt"] }
```

Justification: `tokio` is already pulled transitively by `iced`'s `"tokio"` feature. Added directly to use `tokio::task::spawn_blocking` for offloading synchronous lofty probing without blocking the async executor.

- [ ] **Step 2: Add duration_scan_sub function**

In `src/app.rs`, add after the `shortcuts_stream_sub_none` function:

```rust
fn duration_scan_sub(sounds: &[crate::state::SoundEntry]) -> iced::Subscription<Message> {
    let pairs: Vec<(String, std::path::PathBuf)> = sounds
        .iter()
        .map(|s| (s.id.clone(), s.path.clone()))
        .collect();
    let pairs = std::sync::Arc::new(pairs);
    iced::Subscription::run_with_id(
        "honkhonk-duration-scan",
        {
            let pairs = std::sync::Arc::clone(&pairs);
            move || {
                let pairs = std::sync::Arc::clone(&pairs);
                iced::stream::channel(1, async move |mut tx| {
                    let pairs = (*pairs).clone();
                    let map = tokio::task::spawn_blocking(move || {
                        crate::state::library::probe_durations(pairs)
                    })
                    .await
                    .unwrap_or_default();
                    let _ = tx.send(Message::DurationsLoaded(map)).await;
                    iced::futures::future::pending::<()>().await;
                })
            }
        },
    )
}
```

- [ ] **Step 3: Wire into subscription()**

In `pub fn subscription`, find the `Subscription::batch([shortcuts, tray_poll, events])` line and replace with:

```rust
let mut subs = vec![shortcuts, tray_poll, events];

if !self.durations_loaded {
    subs.push(duration_scan_sub(&self.sounds));
}

Subscription::batch(subs)
```

- [ ] **Step 4: Build to verify compilation**

```bash
cargo build 2>&1 | tail -20
```

Expected: compiles clean. If `Subscription::run_with_id` is not available in this iced version, the error will name the missing method — look up the equivalent in `cargo doc --open` for the `iced::Subscription` type and use that form instead.

- [ ] **Step 5: Run full suite + clippy**

```bash
cargo test 2>&1 | tail -5
cargo clippy -- -D warnings 2>&1 | tail -10
```

Expected: all pass, zero warnings.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/app.rs
git commit -m "feat(app): background duration scan subscription (#67)"
```

---

## Task 5: fmt_duration in ui/mod.rs + now_playing display

**Files:**
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/slot_manager.rs`
- Modify: `src/ui/now_playing.rs`

- [ ] **Step 1: Write failing test for fmt_duration**

Add a new `#[cfg(test)]` block at the bottom of `src/ui/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_duration_formats_seconds() {
        assert_eq!(fmt_duration(Some(3_500)), "0:03");
    }

    #[test]
    fn fmt_duration_formats_minutes() {
        assert_eq!(fmt_duration(Some(63_000)), "1:03");
    }

    #[test]
    fn fmt_duration_pads_seconds() {
        assert_eq!(fmt_duration(Some(60_000)), "1:00");
    }

    #[test]
    fn fmt_duration_none_returns_dash() {
        assert_eq!(fmt_duration(None), "—");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test ui::tests 2>&1 | tail -20
```

Expected: compile error — `fmt_duration` not defined.

- [ ] **Step 3: Add fmt_duration to ui/mod.rs**

Open `src/ui/mod.rs`. Add:

```rust
pub fn fmt_duration(ms: Option<u64>) -> String {
    match ms {
        Some(ms) => format!("{}:{:02}", ms / 60_000, (ms % 60_000) / 1_000),
        None => "\u{2014}".into(), // —
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test ui::tests 2>&1 | tail -20
```

Expected: 4 tests pass.

- [ ] **Step 5: Remove private fmt_duration from slot_manager.rs**

Open `src/ui/slot_manager.rs`. Delete lines 8-11 (the private `fmt_duration` function):

```rust
// DELETE this block:
fn fmt_duration(ms: Option<u64>) -> String {
    ms.map(|ms| format!("{}:{:02}", ms / 60000, (ms % 60000) / 1000))
        .unwrap_or_else(|| "—".into())
}
```

In all call sites within `slot_manager.rs`, prefix with `crate::ui::`:

```rust
// Change:
fmt_duration(sound.duration_ms)
// To:
crate::ui::fmt_duration(sound.duration_ms)
```

- [ ] **Step 6: Add duration to now_playing subtitle**

Open `src/ui/now_playing.rs`. Find `view_sound_info` and update the subtitle line:

```rust
// Before:
let subtitle = text(format!("HONKING NOW \u{00b7} {}", sound.category))
    .size(10.5)
    .color(t.ink_dim());

// After:
let subtitle = text(format!(
    "HONKING NOW \u{00b7} {} \u{00b7} {}",
    sound.category,
    crate::ui::fmt_duration(sound.duration_ms),
))
.size(10.5)
.color(t.ink_dim());
```

- [ ] **Step 7: Build and run full suite + clippy + fmt**

```bash
cargo build 2>&1 | tail -10
cargo test 2>&1 | tail -5
cargo clippy -- -D warnings 2>&1 | tail -10
cargo fmt -- --check 2>&1 | tail -5
```

Expected: all pass, zero warnings, clean format. If fmt fails, run `cargo fmt` then re-check.

- [ ] **Step 8: Commit**

```bash
git add src/ui/mod.rs src/ui/slot_manager.rs src/ui/now_playing.rs
git commit -m "feat(ui): fmt_duration in ui/mod.rs + now-playing duration display (#67)"
```

---

## Verification

After all tasks complete:

- [ ] `cargo test` passes (all existing + new tests)
- [ ] `cargo clippy -- -D warnings` passes with zero warnings
- [ ] `cargo fmt -- --check` passes
- [ ] Manual smoke test: `cargo run` → wait ~2s → now-playing bar shows real M:SS when a sound plays → slot manager sidebar shows real duration for bound sounds
