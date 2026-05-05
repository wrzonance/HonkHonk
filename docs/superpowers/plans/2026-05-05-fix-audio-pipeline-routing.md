# Fix: Audio Pipeline Routing to Virtual Source

**Issue:** #53
**Branch:** `fix/audio-pipeline-routing`
**Date:** 2026-05-05

## Problem

Audio played through HonkHonk never reaches the HonkHonk Mic virtual source. KDE shows zero activity. Discord/Steam receive no audio. Monitor playback (user's speakers) works fine.

## Root Causes

1. **Race condition in `registry.rs` link creation** — only FL channel linked, FR dropped
2. **Suspected `target.object` ID mismatch** — PipeWire 1.x expects serial, code sends node ID
3. **Missing `stream.dont-move`** — WirePlumber can re-route sink stream

## Out of Scope

- Full mixer implementation (`mixer.rs`) — #17
- Router module — #27
- Per-source volume — #29
- App source patching — #17

---

## Phase 1: Fix registry link race condition

### Files
- `src/audio/registry.rs`
- `tests/pipewire_integration.rs`

### Red — Write failing tests

**Test 1: `both_stereo_channels_linked_sink_to_source`**

Integration test (`#[cfg(feature = "pipewire-test")]`). Spawn engine, wait for registry, then verify BOTH FL and FR links exist between `honkhonk-mix:capture_*` and `honkhonk-mic:input_*` via `pw-link --links` output.

```rust
#[test]
fn both_stereo_channels_linked_sink_to_source() {
    pipewire::init();
    let handle = honkhonk::audio::spawn().expect("spawn failed");
    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    std::thread::sleep(Duration::from_secs(2));

    let output = Command::new("pw-link")
        .arg("--links")
        .output()
        .expect("pw-link not found");
    let links = String::from_utf8_lossy(&output.stdout);

    assert!(
        links.contains("honkhonk-mix:capture_FL")
            && links.contains("honkhonk-mic:input_FL"),
        "FL link missing between sink and source.\npw-link:\n{links}"
    );
    assert!(
        links.contains("honkhonk-mix:capture_FR")
            && links.contains("honkhonk-mic:input_FR"),
        "FR link missing between sink and source.\npw-link:\n{links}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}
```

**Test 2: `both_stereo_channels_linked_mic_to_sink`**

Same pattern but verify mic passthrough links (both FL + FR from physical mic → honkhonk-mix). Only assert if a physical source exists.

### Green — Fix `registry.rs`

**Change:** Replace boolean `mic_links_created` / `monitor_links_created` flags with `HashSet<(u32, u32)>` tracking which port pairs are already linked.

In `try_create_monitor_links` and `try_create_mic_links`:
1. Remove early return on `_links_created` flag
2. For each `(output_port, input_port)` pair from the zip:
   - Skip if pair already in the linked set
   - Create link
   - On success, add pair to linked set
3. Remove the boolean flag entirely

```rust
struct RegistryState {
    // ... existing fields ...
    linked_pairs: HashSet<(u32, u32)>,  // replaces mic_links_created + monitor_links_created
}
```

### Refactor

- Clean up any dead code from removed boolean flags
- Ensure `linked_pairs` is checked in both link functions

### Verify
```bash
cargo test --features pipewire-test both_stereo_channels_linked
cargo clippy -- -D warnings
```

---

## Phase 2: Fix sink stream targeting

### Files
- `src/audio/playback.rs`
- `src/audio/engine.rs`
- `src/audio/registry.rs`
- `tests/pipewire_integration.rs`

### Red — Write failing test

**Test 3: `sink_stream_reaches_virtual_sink`**

Integration test. Spawn engine, play a sound, then verify via `pw-link --links` that a `honkhonk-to-sink` stream is connected to `honkhonk-mix` (not the default audio output).

```rust
#[test]
fn sink_stream_reaches_virtual_sink() {
    pipewire::init();
    let handle = honkhonk::audio::spawn().expect("spawn failed");
    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    std::thread::sleep(Duration::from_secs(2));

    // Play a long sound so stream stays alive during check
    let samples = std::sync::Arc::new(vec![0.5f32; 48000 * 5 * 2]);
    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "routing-test".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
    });

    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::PlaybackStarted { .. }));

    std::thread::sleep(Duration::from_millis(500));

    let output = Command::new("pw-link")
        .arg("--links")
        .output()
        .expect("pw-link not found");
    let links = String::from_utf8_lossy(&output.stdout);

    assert!(
        links.contains("honkhonk-to-sink") && links.contains("honkhonk-mix"),
        "sink stream should be connected to honkhonk-mix.\npw-link:\n{links}"
    );

    handle.send(honkhonk::audio::AudioCommand::Stop);
    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}
```

### Green — Fix targeting

**Option A (preferred):** Track virtual sink's `object.serial` in registry, pass serial to `create_sink_stream` for `target.object`.

Changes to `registry.rs`:
- Store `sink_serial: Option<u32>` alongside `sink_node_id`
- Parse `object.serial` from node global props
- Expose via `shared_sink_serial` (same pattern as `shared_sink_id`)

Changes to `engine.rs`:
- Pass both `sink_node_id` and `sink_serial` to `handle_play`
- Forward serial to `create_sink_stream`

Changes to `playback.rs`:
- `create_sink_stream` takes `sink_serial: u32` parameter
- Set `"target.object" => sink_serial.to_string()`
- Add `"stream.dont-move" => "true"` to prevent WirePlumber re-routing
- Keep `Some(sink_node_id)` in `stream.connect()` as backward-compat fallback

**Option B (simpler, if Option A fails):** Remove `target.object` property entirely, rely only on `Some(sink_node_id)` in `stream.connect()`. Add `"node.target" => sink_node_id.to_string()` as fallback.

### Verify
```bash
cargo test --features pipewire-test sink_stream_reaches_virtual_sink
cargo test --features pipewire-test  # all integration tests
cargo clippy -- -D warnings
```

---

## Phase 3: End-to-end verification

### Red — Write failing test

**Test 4: `audio_reaches_virtual_source_during_playback`**

The ultimate integration test. Spawn engine, play a sound, verify that `honkhonk-mic` capture ports show as connected to `honkhonk-mix` AND that the playback stream is connected to the sink.

```rust
#[test]
fn audio_reaches_virtual_source_during_playback() {
    pipewire::init();
    let handle = honkhonk::audio::spawn().expect("spawn failed");
    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    std::thread::sleep(Duration::from_secs(2));

    let samples = std::sync::Arc::new(vec![0.5f32; 48000 * 3 * 2]);
    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "e2e-test".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
    });

    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::PlaybackStarted { .. }));

    std::thread::sleep(Duration::from_millis(500));

    let output = Command::new("pw-link")
        .arg("--links")
        .output()
        .expect("pw-link not found");
    let links = String::from_utf8_lossy(&output.stdout);

    // Full pipeline check:
    // 1. Playback stream → virtual sink
    assert!(links.contains("honkhonk-to-sink"), "playback stream missing");

    // 2. Virtual sink → virtual source (both channels)
    let fl_linked = links.contains("honkhonk-mix:capture_FL")
        && links.contains("honkhonk-mic:input_FL");
    let fr_linked = links.contains("honkhonk-mix:capture_FR")
        && links.contains("honkhonk-mic:input_FR");
    assert!(fl_linked, "FL sink→source link missing");
    assert!(fr_linked, "FR sink→source link missing");

    handle.send(honkhonk::audio::AudioCommand::Stop);
    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}
```

### Green

Should pass after Phase 1 + Phase 2 fixes. If not, debug.

### Final Verification
```bash
cargo test                            # all unit tests
cargo test --features pipewire-test   # all integration tests
cargo clippy -- -D warnings
cargo fmt -- --check
```

---

## Implementation Order

| # | Phase | Est. LOC | Files Modified |
|---|-------|----------|----------------|
| 1 | Registry link race fix | ~40 | registry.rs, pipewire_integration.rs |
| 2 | Stream targeting fix | ~30 | playback.rs, engine.rs, registry.rs |
| 3 | E2E verification test | ~30 | pipewire_integration.rs |
| **Total** | | **~100** | |

## Commit Plan

1. `test(audio): add stereo channel link verification tests` (RED)
2. `fix(audio): link all stereo channels in registry — replace boolean flags with port-pair tracking` (GREEN)
3. `test(audio): add sink stream routing verification test` (RED)
4. `fix(audio): use object serial for target.object + add stream.dont-move` (GREEN)
5. `test(audio): add end-to-end pipeline routing test` (GREEN — should pass)
