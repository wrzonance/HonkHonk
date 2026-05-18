# Design — Issue #26 sub-MVP: external audio stream watcher

**Date:** 2026-05-17
**Branch:** `feat/issue-26-stream-watcher`
**Closes:** #26 (PipeWire graph watcher + node enumeration — observation-only pass)

## Goal

Expose a live, hot-plug list of other applications producing audio (e.g. Spotify, Firefox, paplay) via a new `StreamWatcher` running on the existing PipeWire thread. Capture stable identity properties (`application.name`, `application.process.binary`, `application.process.id`) so future #27 routing and #29 per-stream controls can implement persistent route intent across stream destroy/recreate cycles. This PR ships the watcher only; UI consumption is debug-print to stderr.

## Scope

### In

| File | Purpose | LOC est. |
|---|---|---|
| `src/audio/streams.rs` (new) | `StreamWatcher` + `StreamEvent` + registry listener + prop extraction + self-PID filter | ~320 |
| `src/audio/error.rs` | Add `StreamWatcherInit(String)` variant to `AudioError` | ~3 |
| `src/audio/mod.rs` | `mod streams;` + `pub use streams::{StreamEvent, StreamWatcher, Direction};` | ~2 |
| `src/audio/engine.rs` | Init `StreamWatcher` in `start_engine`, spawn debug-print drain thread, hold guard | ~20 |

**Total: ~345 LOC.** Under CLAUDE.md 500 LOC ceiling.

No new external crates (`pipewire-rs` already a direct dep).

### Out (explicit — separate future PRs)

- UI rendering of stream list (#28 mixer panel)
- Stream-to-virtual-sink routing (#27 link routing)
- Per-stream volume / mute controls (#29)
- Cross-restart auto-reconnect using stable identity (#27 routing work consumes the stable props this PR exposes)
- Source-input / mic-style stream tracking — only `Stream/Output/Audio` for this PR
- ALSA-corked-vs-destroyed lifecycle distinction handling
- Property-predicate matching beyond simple class filter (venmic-style match expressions)
- Multi-app stress / fuzz testing
- `AudioEvent` enum extension (separate channel keeps observation decoupled)

## Architecture

### Module placement

`src/audio/streams.rs` is a sibling of the existing `src/audio/registry.rs`. The existing `registry.rs` (374 LOC) manages HonkHonk's *own* infrastructure (virtual sink, virtual source, mic passthrough links). The new `streams.rs` is a separate concern — observing *other applications'* output streams. Per CLAUDE.md "self-contained unit, one clear purpose", they stay separate.

Both modules attach independent registry listeners to the same PipeWire `Core` instance — supported by `pipewire-rs` and standard pattern (Helvum uses similar separation).

### Public API

```rust
// src/audio/streams.rs

#[derive(Debug, Clone)]
pub enum StreamEvent {
    SourceAdded {
        id: u32,
        name: String,                  // node.description → node.nick → node.name fallback
        app_name: Option<String>,      // application.name (stable across reconnects)
        app_binary: Option<String>,    // application.process.binary (stable)
        app_pid: Option<u32>,          // application.process.id (stable while process lives)
        icon: Option<String>,          // application.icon_name
        media_name: Option<String>,    // media.name (current track title etc.)
    },
    SourceRemoved { id: u32 },
    SourceUpdated { id: u32, media_name: Option<String> },
    PortAdded { id: u32, node_id: u32, channel: String, direction: Direction },
    PortRemoved { id: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction { Input, Output }

pub struct StreamWatcher {
    _registry: pipewire::registry::Registry,
    _listener: pipewire::registry::Listener,
    // node proxies stashed in Rc<RefCell<HashMap<u32, NodeProxyEntry>>> so
    // listener closures can mutate; dropping watcher drops all proxies.
}

pub fn start(
    core: &pipewire::core::Core,
    self_pid: u32,
) -> Result<(StreamWatcher, std::sync::mpsc::Receiver<StreamEvent>), AudioError>;
```

### Filter chain (inside `global` listener)

1. `global.type_ == ObjectType::Node`
2. `props.get("media.class") == Some("Stream/Output/Audio")`
3. `!is_own_node(props, self_pid)` — exclude HonkHonk's own audio nodes by PID match against `application.process.id`
4. Bind `pipewire::node::Node` proxy, attach info listener (extracts full props after registry-event time), stash in `nodes_map`
5. On `node.info` arrival: emit `StreamEvent::SourceAdded { ... }` once with full extracted props

### Helper functions

```rust
fn is_own_node(props: &DictRef, self_pid: u32) -> bool {
    props.get("application.process.id")
        .and_then(|s| s.parse::<u32>().ok())
        .map(|pid| pid == self_pid)
        .unwrap_or(false)   // fail-open: PID missing → don't filter
}

fn extract_name(props: &DictRef) -> String {
    props.get("node.description")
        .or_else(|| props.get("node.nick"))
        .or_else(|| props.get("node.name"))
        .unwrap_or("unknown")
        .to_owned()
}

fn extract_pid(props: &DictRef) -> Option<u32> {
    props.get("application.process.id").and_then(|s| s.parse().ok())
}

// + extract_binary, extract_app_name, extract_icon (all Option<String>)
```

### Port tracking

When `global.type_ == ObjectType::Port` arrives:
- Look up `node.id` prop against `nodes_map`
- If parent tracked, extract `audio.channel` (e.g. `FL`, `FR`, `MONO`) and `port.direction` (`in` / `out`)
- Emit `StreamEvent::PortAdded { id, node_id, channel, direction }`
- If parent NOT tracked: drop port event with single `eprintln!` warning (out-of-order rare; refined in #27)

### `global_remove` handling

- Always forward as `StreamEvent::SourceRemoved { id }` (cheap; consumers know to ignore IDs they don't care about)
- Remove entry from `nodes_map` (drops proxy, frees PW resources)

### Channel pattern

- `streams.rs` owns a fresh `std::sync::mpsc::channel::<StreamEvent>()`
- `start()` returns `(StreamWatcher, Receiver<StreamEvent>)`
- `engine.rs` keeps a reference to `StreamWatcher` (for RAII shutdown) and spawns a sidecar thread draining the receiver:
  ```rust
  std::thread::spawn(move || {
      while let Ok(event) = stream_rx.recv() {
          eprintln!("honkhonk stream: {event:?}");
      }
  });
  ```
- Future #28 work replaces drain task with `Sender<Message::StreamEvent>` forwarding to `app.rs` Iced state. No `AudioEvent` enum churn this PR.

### `src/audio/error.rs` extension

```rust
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    // ... existing variants ...
    #[error("stream watcher initialization failed: {0}")]
    StreamWatcherInit(String),
}
```

### `src/audio/engine.rs` integration

Inside `start_engine` (after `core` is connected, near existing `setup_registry_listener` call):

```rust
let self_pid = std::process::id();
let (stream_watcher, stream_rx) = streams::start(&core, self_pid)
    .context("starting external stream watcher")?;

std::thread::spawn(move || {
    while let Ok(event) = stream_rx.recv() {
        eprintln!("honkhonk stream: {event:?}");
    }
});

// stream_watcher held on engine guard struct so its Drop runs at shutdown
```

`stream_watcher` is added as a field on whatever RAII struct currently holds `RegistryGuard`. When `engine.rs` shuts down, Drop runs on `StreamWatcher` → listener detaches → all node proxies drop → registry quiet.

## Testing

### Unit (regular `cargo test`)

| Test | Asserts |
|---|---|
| `is_own_node_matches_self_pid` | Returns `true` when `application.process.id` equals self_pid |
| `is_own_node_skips_missing` | Returns `false` when prop absent (fail-open) |
| `is_own_node_skips_non_numeric` | Returns `false` on garbage PID string |
| `extract_name_uses_description` | Returns `node.description` when present |
| `extract_name_falls_back_to_nick` | Returns `node.nick` when description missing |
| `extract_name_falls_back_to_name` | Returns `node.name` when description + nick missing |
| `extract_name_default_unknown` | Returns `"unknown"` when all three missing |
| `extract_pid_parses_numeric` | `Some(1234)` from `"1234"` |
| `extract_pid_handles_missing` | `None` from absent prop |

### Integration (`#[cfg(feature = "pipewire-test")]`, gated)

```rust
#[cfg(feature = "pipewire-test")]
#[test]
fn paplay_subprocess_generates_source_added_then_removed() {
    let core = test_pipewire_connect();
    let (watcher, rx) = streams::start(&core, std::process::id()).unwrap();
    let drained: Arc<Mutex<Vec<StreamEvent>>> = Arc::default();
    let drain_clone = drained.clone();
    std::thread::spawn(move || {
        while let Ok(e) = rx.recv() {
            drain_clone.lock().unwrap().push(e);
        }
    });

    let mut child = std::process::Command::new("paplay")
        .arg("/usr/share/sounds/freedesktop/stereo/bell.oga")
        .spawn().unwrap();

    let pid = child.id();
    wait_for_event(&drained, |e| matches!(e, StreamEvent::SourceAdded { app_pid: Some(p), .. } if *p == pid), Duration::from_secs(2));

    child.kill().unwrap();
    wait_for_event(&drained, |e| matches!(e, StreamEvent::SourceRemoved { .. }), Duration::from_secs(2));

    drop(watcher);
}
```

### Manual smoke (post-merge)

1. Launch HonkHonk on real Wayland + PipeWire session
2. `spotify` plays a track → stderr shows `honkhonk stream: SourceAdded { id: <X>, app_name: Some("Spotify"), app_binary: Some("spotify"), ... }`
3. Pause Spotify → `SourceUpdated` (media_name may change to empty)
4. Stop Spotify → `SourceRemoved { id: <X> }`
5. Restart Spotify → new `SourceAdded` with new `id` but same `app_binary` + same `app_pid` (until process exit)
6. HonkHonk's own engine streams do NOT appear (self-PID filter works)

### Out of test scope

- ALSA-corked vs destroyed lifecycle differences (covered in #27)
- Multi-app stress (>10 concurrent streams)
- CI integration test execution — no PipeWire in stock GitHub Actions runners; `pipewire-test` feature stays opt-in

## Error handling + edge cases

- **Registry init failure**: `AudioError::StreamWatcherInit(detail)` propagated to `start_engine`; surfaces via existing AudioEvent error path.
- **Missing optional props**: `app_name`, `app_binary`, `icon`, `media_name`, `app_pid` all `Option<String>` / `Option<u32>` — never error.
- **Non-numeric `application.process.id`**: treated as PID unknown (`None`); self-filter fail-opens (node reported, not filtered).
- **Node disappears between `global` and `info` listener**: stashed proxy drops in `global_remove`; no `SourceAdded` emitted. No leak.
- **Port arrives before parent node**: warning `eprintln!`, event dropped. Out-of-order PW events rare; refined in #27 if it bites.
- **Listener thread panic**: caught at PW thread boundary; existing AudioEvent error path reports. PW thread itself continues.
- **`pipewire-test` feature off**: integration test skipped by `#[cfg]` — `cargo test` clean in CI without PipeWire.

## TDD ordering (writing-plans will expand)

1. RED: unit test `is_own_node_matches_self_pid` — fails (fn doesn't exist).
2. GREEN: implement `is_own_node`.
3. RED: `extract_name_uses_description` — fails.
4. GREEN: implement `extract_name`.
5. RED: remaining unit tests for prop extractors.
6. GREEN: implement extractors.
7. RED: skeleton `StreamWatcher::start` returning empty + integration test gated by feature — fails when paplay spawned (no events).
8. GREEN: wire registry listener + global handler + Node proxy binding.
9. RED: `SourceRemoved` integration test — fails until `global_remove` forwarded.
10. GREEN: handle `global_remove`.
11. RED: port-tracking integration test — fails until Port type handled.
12. GREEN: handle `ObjectType::Port`.
13. REFACTOR: extract shared `DictRef → String` helpers if patterns repeat.
14. Wire into `engine.rs` and ship.

## References

- Issue #26: https://github.com/wrzonance/HonkHonk/issues/26
- Existing registry: `src/audio/registry.rs:setup_registry_listener` (sibling pattern reference)
- Helvum registry watcher: https://gitlab.freedesktop.org/pipewire/helvum
- pipewire-soundpad: https://github.com/arabianq/pipewire-soundpad
- venmic property-predicate matching: https://github.com/Vencord/venmic
- pipewire-screenaudio: https://github.com/IceDBorn/pipewire-screenaudio
- Stable identity rationale: issue body "Stream Lifecycle Context" + "Stable Identity Properties" tables
- CLAUDE.md error-handling rules: typed module error + `.context()` propagation
- CLAUDE.md 500 LOC / sub-MVP rule
