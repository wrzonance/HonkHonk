# PipeWire Virtual Sink + Mic Passthrough — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create persistent PipeWire virtual sink ("HonkHonk Mix") with physical mic passthrough. Sink visible in `wpctl status`, mic audio routed through, nodes destroyed cleanly on exit.

**Architecture:** Dedicated PipeWire thread owns `MainLoop` + `Core`. Virtual sink created via `core.create_object("adapter", ...)` with `support.null-audio-sink` factory. Physical mic passthrough via PipeWire links from default source output ports → sink input ports (PipeWire sums inputs natively — no app-side mixer). Registry watcher discovers node/port IDs asynchronously. Communication with Iced app via `pipewire::channel` (commands in) and `std::sync::mpsc` (events out).

**Tech Stack:** pipewire-rs 0.8, thiserror 2, std::sync::mpsc

**Issue:** #3

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src/audio/engine.rs` | `AudioEngine` lifecycle: PipeWire init, virtual sink, registry watcher, mic links, shutdown (~250 lines) |
| Modify | `src/audio/error.rs` | Add PipeWire error variants (`PipeWireInit`, `VirtualSinkCreation`, `LinkCreation`, `ThreadSpawn`) |
| Modify | `src/audio/mod.rs` | Re-export engine types (`AudioCommand`, `AudioEvent`, `AudioHandle`, `spawn`) |
| Modify | `src/app.rs` | Add `AudioHandle` field, `AudioEvent` message variant, poll subscription |
| Modify | `src/main.rs` | Call `pipewire::init()`, spawn engine, pass handle to Iced app |
| Modify | `Cargo.toml` | Add `pipewire` dep, `pipewire-test` feature |
| Create | `tests/pipewire_integration.rs` | Feature-gated integration tests: sink creation, mic passthrough, clean shutdown (~80 lines) |

---

### Task 1: Dependencies + Error Variants + Type Stubs

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/audio/error.rs`
- Create: `src/audio/engine.rs`
- Modify: `src/audio/mod.rs`

- [ ] **Step 1: Add pipewire dependency and feature to `Cargo.toml`**

Add to `[dependencies]`:

```toml
pipewire = "0.8"
```

Add to `[features]`:

```toml
pipewire-test = []
```

- [ ] **Step 2: Add PipeWire error variants to `src/audio/error.rs`**

Replace the full file contents:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("failed to open audio file")]
    FileOpen(#[source] std::io::Error),

    #[error("unsupported audio format")]
    UnsupportedFormat(#[source] symphonia::core::errors::Error),

    #[error("no audio track found in file")]
    NoTrack,

    #[error("missing codec parameters (sample rate or channels)")]
    MissingCodecParams,

    #[error("failed to create audio decoder")]
    DecoderInit(#[source] symphonia::core::errors::Error),

    #[error("decode error")]
    Decode(#[source] symphonia::core::errors::Error),

    #[error("failed to initialize PipeWire: {0}")]
    PipeWireInit(String),

    #[error("failed to create virtual sink: {0}")]
    VirtualSinkCreation(String),

    #[error("failed to create audio link: {0}")]
    LinkCreation(String),

    #[error("failed to spawn audio thread")]
    ThreadSpawn(#[source] std::io::Error),
}
```

- [ ] **Step 3: Create `src/audio/engine.rs` with type definitions and `spawn()` stub**

```rust
use std::sync::mpsc;

use super::error::AudioError;

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SINK_DESCRIPTION: &str = "HonkHonk Mix";

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum AudioEvent {
    Ready,
    Error(String),
}

pub struct AudioHandle {
    cmd_tx: pipewire::channel::Sender<AudioCommand>,
    evt_rx: mpsc::Receiver<AudioEvent>,
}

impl AudioHandle {
    pub fn try_recv(&self) -> Option<AudioEvent> {
        self.evt_rx.try_recv().ok()
    }

    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<AudioEvent> {
        self.evt_rx.recv_timeout(timeout).ok()
    }

    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(AudioCommand::Shutdown);
    }
}

pub fn spawn() -> Result<AudioHandle, AudioError> {
    let (cmd_tx, cmd_rx) = pipewire::channel::channel::<AudioCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>();

    std::thread::Builder::new()
        .name("honkhonk-pw".into())
        .spawn(move || {
            if let Err(e) = run_engine(cmd_rx, evt_tx.clone()) {
                let _ = evt_tx.send(AudioEvent::Error(e.to_string()));
            }
        })
        .map_err(AudioError::ThreadSpawn)?;

    Ok(AudioHandle { cmd_tx, evt_rx })
}

fn run_engine(
    _cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
) -> Result<(), AudioError> {
    let _ = evt_tx.send(AudioEvent::Error("not implemented".into()));
    Ok(())
}
```

> **Note on `pipewire::channel`:** If `pipewire::channel` does not exist in pipewire 0.8, upgrade to `pipewire = "0.9"` (latest, has the channel module). Alternatively, use `std::sync::mpsc` + a `nix::sys::eventfd` registered on the PipeWire main loop as an IO source — but try the channel module first.

- [ ] **Step 4: Update `src/audio/mod.rs` to re-export engine types**

```rust
mod decoder;
mod engine;
mod error;

pub use decoder::{decode, DecodedAudio};
pub use engine::{spawn, AudioCommand, AudioEvent, AudioHandle};
pub use error::AudioError;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check`

Expected: Compiles with no errors. May have unused warnings for `SINK_NODE_NAME`, `SINK_DESCRIPTION`, `_cmd_rx` — that's fine.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/audio/error.rs src/audio/engine.rs src/audio/mod.rs
git commit -m "feat(audio): add PipeWire engine types and spawn stub (#3)"
```

---

### Task 2: PipeWire Init + Virtual Sink Creation

**Files:**
- Modify: `src/audio/engine.rs`

- [ ] **Step 1: Write integration test for virtual sink appearing in `wpctl status`**

Create `tests/pipewire_integration.rs`:

```rust
#![cfg(feature = "pipewire-test")]

use std::process::Command;
use std::time::Duration;

#[test]
fn virtual_sink_appears_in_wpctl() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("failed to spawn audio engine");

    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no event received from audio engine within 5s");

    assert!(
        matches!(event, honkhonk::audio::AudioEvent::Ready),
        "expected Ready event, got: {event:?}"
    );

    // Give WirePlumber a moment to register the node
    std::thread::sleep(Duration::from_millis(500));

    let output = Command::new("wpctl")
        .arg("status")
        .output()
        .expect("wpctl not found — is WirePlumber running?");

    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        status.contains("HonkHonk Mix"),
        "Virtual sink 'HonkHonk Mix' not found in wpctl status.\n\
         wpctl output:\n{status}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));

    let output = Command::new("wpctl")
        .arg("status")
        .output()
        .expect("wpctl failed");

    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        !status.contains("HonkHonk Mix"),
        "Virtual sink 'HonkHonk Mix' was NOT cleaned up after shutdown.\n\
         wpctl output:\n{status}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --features pipewire-test virtual_sink_appears -- --nocapture`

Expected: FAIL — engine returns `AudioEvent::Error("not implemented")` since `run_engine` is a stub.

- [ ] **Step 3: Implement `run_engine` — PipeWire connection + virtual sink creation**

Replace `run_engine` in `src/audio/engine.rs`:

```rust
fn run_engine(
    cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
) -> Result<(), AudioError> {
    let mainloop = std::rc::Rc::new(
        pipewire::main_loop::MainLoop::new(None)
            .map_err(|e| AudioError::PipeWireInit(format!("main loop: {e}")))?,
    );

    let context = pipewire::context::Context::new(&*mainloop)
        .map_err(|e| AudioError::PipeWireInit(format!("context: {e}")))?;

    let core_props = pipewire::properties! {
        "media.category" => "Manager",
    };
    let core = std::rc::Rc::new(
        context
            .connect(Some(core_props))
            .map_err(|e| AudioError::PipeWireInit(format!("core connect: {e}")))?,
    );

    // Create persistent virtual sink — appears as "HonkHonk Mix" in audio settings.
    // Its monitor output is what Discord/apps select as mic input.
    let sink_props = pipewire::properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => SINK_NODE_NAME,
        "node.description" => SINK_DESCRIPTION,
        "media.class" => "Audio/Sink/Virtual",
        "audio.position" => "[FL,FR]",
        "object.linger" => "false",
    };
    let _sink: pipewire::node::Node = core
        .create_object("adapter", &sink_props)
        .map_err(|e| AudioError::VirtualSinkCreation(e.to_string()))?;

    // Command channel: quit when app sends Shutdown
    let mainloop_quit = mainloop.clone();
    let _cmd_listener = cmd_rx.attach(mainloop.loop_(), move |cmd| {
        match cmd {
            AudioCommand::Shutdown => mainloop_quit.quit(),
        }
    });

    let _ = evt_tx.send(AudioEvent::Ready);
    mainloop.run();

    // Cleanup: dropping _sink proxy + disconnecting core destroys the virtual sink
    // (object.linger = false → destroyed when client disconnects)
    Ok(())
}
```

Add these imports at the top of `engine.rs`:

```rust
use std::rc::Rc;
use std::sync::mpsc;

use super::error::AudioError;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --features pipewire-test virtual_sink_appears -- --nocapture`

Expected: PASS — virtual sink appears in `wpctl status`, disappears after shutdown.

> **Troubleshooting:** If `create_object` fails with "no factory 'support.null-audio-sink'", check that PipeWire's null-audio-sink module is installed (`pacman -Ql pipewire | grep null`). If the factory name is wrong, try `"support.null-audio-sink"` without the adapter wrapper — i.e., `core.create_object("support.null-audio-sink", &sink_props)` with `factory.name` removed from props.

> **Troubleshooting:** If `MainLoop::new()` or `Context::new()` signatures differ (Option instead of Result, no error parameter), adjust the error mapping. The exact return types vary between pipewire-rs 0.8 and 0.9.

- [ ] **Step 5: Commit**

```bash
git add src/audio/engine.rs tests/pipewire_integration.rs
git commit -m "feat(audio): PipeWire init + virtual sink creation (#3)"
```

---

### Task 3: Registry Watcher + Mic Passthrough Links

**Files:**
- Modify: `src/audio/engine.rs`

- [ ] **Step 1: Add integration test for mic passthrough**

Add to `tests/pipewire_integration.rs`:

```rust
#[test]
fn mic_linked_to_virtual_sink() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("failed to spawn audio engine");

    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no event received within 5s");

    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    // Wait for registry discovery + link creation
    std::thread::sleep(Duration::from_secs(2));

    // Check pw-link for active links to our sink
    let output = Command::new("pw-link")
        .arg("--links")
        .output()
        .expect("pw-link not found");

    let links = String::from_utf8_lossy(&output.stdout);

    // At minimum, the sink should exist. If a mic is connected,
    // there should be links to honkhonk-mix input ports.
    let has_sink = links.contains("honkhonk-mix");

    // Check if any audio source exists in the system
    let wpctl = Command::new("wpctl")
        .arg("status")
        .output()
        .expect("wpctl failed");
    let status = String::from_utf8_lossy(&wpctl.stdout);
    let has_source = status.contains("Sources:");

    if has_source {
        assert!(
            has_sink,
            "Expected links to honkhonk-mix when audio sources exist.\n\
             pw-link output:\n{links}"
        );
    }

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --features pipewire-test mic_linked -- --nocapture`

Expected: FAIL — no link creation code exists yet, so `honkhonk-mix` won't appear in `pw-link --links` output as a link target.

- [ ] **Step 3: Add `RegistryState` struct and `try_create_links` helper to `engine.rs`**

Add above `run_engine`:

```rust
#[derive(Default)]
struct RegistryState {
    sink_node_id: Option<u32>,
    sink_input_ports: Vec<u32>,
    source_node_id: Option<u32>,
    source_output_ports: Vec<u32>,
    links_created: bool,
}

fn try_create_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
) {
    if state.links_created {
        return;
    }
    if state.sink_input_ports.len() < 2 || state.source_output_ports.len() < 2 {
        return;
    }

    state.links_created = true;

    for (src_port, sink_port) in state
        .source_output_ports
        .iter()
        .zip(state.sink_input_ports.iter())
    {
        let link_props = pipewire::properties! {
            "link.output.port" => src_port.to_string(),
            "link.input.port" => sink_port.to_string(),
            "object.linger" => "false",
        };
        match core.create_object::<pipewire::link::Link>("link-factory", &link_props) {
            Ok(link) => links.push(link),
            Err(e) => eprintln!("honkhonk: failed to create mic passthrough link: {e}"),
        }
    }
}
```

- [ ] **Step 4: Add registry listener to `run_engine`**

Insert after `_sink` creation, before the command channel setup:

```rust
    let state = Rc::new(std::cell::RefCell::new(RegistryState::default()));
    let mic_links: Rc<std::cell::RefCell<Vec<pipewire::link::Link>>> =
        Rc::new(std::cell::RefCell::new(Vec::new()));

    let registry = core
        .get_registry()
        .map_err(|e| AudioError::PipeWireInit(format!("registry: {e}")))?;

    let state_ref = state.clone();
    let links_ref = mic_links.clone();
    let core_ref = core.clone();
    let _reg_listener = registry
        .add_listener_local()
        .global(move |global| {
            let props = match global.props {
                Some(p) => p,
                None => return,
            };

            let mut s = state_ref.borrow_mut();

            match global.type_ {
                pipewire::types::ObjectType::Node => {
                    let name = props.get("node.name").unwrap_or("");
                    let class = props.get("media.class").unwrap_or("");

                    if name == SINK_NODE_NAME {
                        s.sink_node_id = Some(global.id);
                    } else if class == "Audio/Source" && s.source_node_id.is_none() {
                        s.source_node_id = Some(global.id);
                    }
                }
                pipewire::types::ObjectType::Port => {
                    let node_id: u32 = props
                        .get("node.id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let direction = props.get("port.direction").unwrap_or("");

                    if Some(node_id) == s.sink_node_id && direction == "in" {
                        s.sink_input_ports.push(global.id);
                    } else if Some(node_id) == s.source_node_id && direction == "out" {
                        s.source_output_ports.push(global.id);
                    }
                }
                _ => {}
            }

            let mut link_store = links_ref.borrow_mut();
            try_create_links(&mut s, &core_ref, &mut link_store);
        })
        .register();
```

Add `std::cell::RefCell` to the imports at the top.

- [ ] **Step 5: Run the test**

Run: `cargo test --features pipewire-test mic_linked -- --nocapture`

Expected: PASS — if a physical mic exists, links are created from its output ports to `honkhonk-mix` input ports.

> **Troubleshooting:** If port direction is not `"in"`/`"out"` but uses enum values, check `global.props` for `"port.direction"` key format. Some pipewire-rs versions use `"input"`/`"output"` instead.

> **Troubleshooting:** If `create_object::<pipewire::link::Link>` doesn't work (wrong trait bound), check if the type is `pipewire::link::Link` or `pipewire::proxy::Link`. Look at `pipewire::link` module exports.

- [ ] **Step 6: Commit**

```bash
git add src/audio/engine.rs tests/pipewire_integration.rs
git commit -m "feat(audio): registry watcher + mic passthrough links (#3)"
```

---

### Task 4: Wire AudioEngine into Iced App

**Files:**
- Modify: `src/app.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add `AudioEvent` message variant and `AudioHandle` field to `src/app.rs`**

Add to the `Message` enum:

```rust
AudioEvent(crate::audio::AudioEvent),
```

Add field to `HonkHonk` struct:

```rust
pub struct HonkHonk {
    visible: bool,
    exit: bool,
    tray_rx: Arc<Mutex<Receiver<TrayEvent>>>,
    _tray: Option<TrayHandle>,
    audio: Option<crate::audio::AudioHandle>,
}
```

- [ ] **Step 2: Update `HonkHonk::new()` to accept `AudioHandle`**

```rust
pub fn new(mut tray: TrayHandle, audio: crate::audio::AudioHandle) -> Self {
    let rx = tray.take_rx();
    Self {
        visible: true,
        exit: false,
        tray_rx: Arc::new(Mutex::new(rx)),
        _tray: Some(tray),
        audio: Some(audio),
    }
}

pub fn new_for_test() -> Self {
    let (_tx, rx) = std::sync::mpsc::channel();
    Self {
        visible: true,
        exit: false,
        tray_rx: Arc::new(Mutex::new(rx)),
        _tray: None,
        audio: None,
    }
}
```

- [ ] **Step 3: Handle audio events in `update()`**

Add match arms:

```rust
Message::AudioEvent(event) => {
    match event {
        crate::audio::AudioEvent::Ready => {
            eprintln!("honkhonk: audio engine ready");
        }
        crate::audio::AudioEvent::Error(e) => {
            eprintln!("honkhonk: audio error: {e}");
        }
    }
    Task::none()
}
Message::AudioPoll => {
    if let Some(ref audio) = self.audio {
        if let Some(event) = audio.try_recv() {
            return self.update(Message::AudioEvent(event));
        }
    }
    Task::none()
}
```

- [ ] **Step 4: Add audio poll to `subscription()`**

Update the subscription to poll both tray and audio:

```rust
pub fn subscription(&self) -> Subscription<Message> {
    iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll)
}
```

Wait — both `TrayPoll` and `AudioPoll` use 100ms polling. Merge them into the same tick:

Change the `TrayPoll` handler in `update()` to also check audio:

```rust
Message::TrayPoll => {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }

    // Poll tray events
    let tray_event = self.tray_rx.lock().ok().and_then(|rx| rx.try_recv().ok());
    if let Some(e) = tray_event {
        let msg = Message::from_tray_event(e);
        return self.update(msg);
    }

    // Poll audio events
    if let Some(ref audio) = self.audio {
        if let Some(event) = audio.try_recv() {
            return self.update(Message::AudioEvent(event));
        }
    }

    Task::none()
}
```

This avoids adding a separate message variant — reuse the existing 100ms tick.

- [ ] **Step 5: Send shutdown to audio engine on Quit**

Update the `Quit` handler:

```rust
Message::Quit => {
    if let Some(ref audio) = self.audio {
        audio.shutdown();
    }
    self.exit = true;
    iced::exit()
}
```

- [ ] **Step 6: Update `src/main.rs` to init PipeWire and spawn engine**

```rust
fn main() -> iced::Result {
    pipewire::init();

    if let Err(e) = gtk::init() {
        eprintln!("fatal: failed to initialize GTK (required for system tray): {e}");
        std::process::exit(1);
    }

    let tray_handle = match honkhonk::tray::build_tray() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("fatal: failed to initialize system tray: {e}");
            std::process::exit(1);
        }
    };

    let audio_handle = match honkhonk::audio::spawn() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("fatal: failed to start audio engine: {e}");
            std::process::exit(1);
        }
    };

    let tray_handle = std::sync::Mutex::new(Some(tray_handle));
    let audio_handle = std::sync::Mutex::new(Some(audio_handle));

    iced::application(
        move || {
            let tray = tray_handle
                .lock()
                .expect("tray mutex poisoned")
                .take()
                .expect("boot called more than once");
            let audio = audio_handle
                .lock()
                .expect("audio mutex poisoned")
                .take()
                .expect("boot called more than once");
            honkhonk::app::HonkHonk::new(tray, audio)
        },
        honkhonk::app::HonkHonk::update,
        honkhonk::app::HonkHonk::view,
    )
    .title("HonkHonk")
    .subscription(honkhonk::app::HonkHonk::subscription)
    .theme(honkhonk::app::HonkHonk::theme)
    .run()
}
```

- [ ] **Step 7: Verify it compiles and runs**

Run: `cargo build`

Expected: Compiles. No errors.

Run: `cargo run`

Expected: Window opens with tray icon. Terminal shows `honkhonk: audio engine ready`. `wpctl status` shows "HonkHonk Mix" under Sinks. Closing the window destroys the virtual sink.

- [ ] **Step 8: Commit**

```bash
git add src/app.rs src/main.rs
git commit -m "feat(audio): wire PipeWire engine into Iced app (#3)"
```

---

### Task 5: Integration Test — Clean Shutdown Verification

**Files:**
- Modify: `tests/pipewire_integration.rs`

- [ ] **Step 1: Add shutdown cleanup test**

Add to `tests/pipewire_integration.rs`:

```rust
#[test]
fn engine_cleans_up_on_shutdown() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("spawn failed");
    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    // Verify sink exists
    std::thread::sleep(Duration::from_millis(500));
    let output = Command::new("wpctl").arg("status").output().unwrap();
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(status.contains("HonkHonk Mix"), "Sink should exist before shutdown");

    // Shutdown
    handle.shutdown();

    // Wait for PipeWire thread to exit and cleanup
    std::thread::sleep(Duration::from_secs(1));

    // Verify sink is gone
    let output = Command::new("wpctl").arg("status").output().unwrap();
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        !status.contains("HonkHonk Mix"),
        "Sink should be destroyed after shutdown"
    );
}
```

- [ ] **Step 2: Run all integration tests**

Run: `cargo test --features pipewire-test -- --nocapture --test-threads=1`

The `--test-threads=1` flag is required — multiple tests creating PipeWire engines simultaneously can conflict (same node name, shared PipeWire graph).

Expected: All tests PASS.

- [ ] **Step 3: Run standard tests to verify no regressions**

Run: `cargo test`

Expected: All existing decoder tests pass. Integration tests are skipped (no `pipewire-test` feature).

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: No warnings. Fix any issues before committing.

- [ ] **Step 5: Commit**

```bash
git add tests/pipewire_integration.rs
git commit -m "test(audio): integration tests for PipeWire engine lifecycle (#3)"
```

---

### Task 6: End-to-End Verification + Final Commit

**Files:** None (verification only)

- [ ] **Step 1: Manual verification — virtual sink visible**

Run: `cargo run &`

Then in another terminal:

```bash
wpctl status
```

Expected output includes under "Sinks":
```
 ├── Sinks:
 │   ...
 │      XX. HonkHonk Mix [vol: 1.00]
```

- [ ] **Step 2: Manual verification — mic passthrough**

Check that links exist from your physical mic to the virtual sink:

```bash
pw-link --links
```

Expected: Output shows links from your mic's output ports (e.g., `alsa_input.*.capture_FL`) to `honkhonk-mix:input_FL` and `honkhonk-mix:input_FR`.

- [ ] **Step 3: Manual verification — Discord/app can see virtual mic**

Open Discord (or `pavucontrol`) and check the input device list. Expected: "Monitor of HonkHonk Mix" appears as an available microphone input.

- [ ] **Step 4: Manual verification — clean shutdown**

Close the HonkHonk window (or Ctrl+C the process). Then:

```bash
wpctl status
```

Expected: "HonkHonk Mix" is no longer listed.

- [ ] **Step 5: Verify LOC delta within budget**

```bash
git diff --stat main...HEAD
```

Expected: ~450 lines added/modified. Should be under 500 LOC limit.

- [ ] **Step 6: Final commit (if any fixups needed)**

If any manual verification revealed issues that were fixed, commit the fixes:

```bash
git add -p
git commit -m "fix(audio): [description of fix] (#3)"
```

---

## API Uncertainty Notes

The following pipewire-rs API details may differ between versions 0.8 and 0.9. If compilation fails, check these first:

| Assumption | Alternative |
|-----------|-------------|
| `MainLoop::new(None)` returns `Result<MainLoop, Error>` | May return `Option<MainLoop>` — use `.ok_or_else(\|\| AudioError::PipeWireInit(...))?` |
| `pipewire::channel::channel()` exists | If missing, upgrade to `pipewire = "0.9"`. Fallback: use `std::sync::mpsc` + `nix::sys::eventfd` + `loop_.add_io()` |
| `core.create_object::<Node>("adapter", &props)` | Factory name might be `"support.null-audio-sink"` directly (remove `factory.name` from props) |
| `pipewire::types::ObjectType::Node` / `::Port` | May be `pw::spa::utils::ObjectType` or similar path |
| `global.props.get("key")` returns `Option<&str>` | May need `.to_string()` or different accessor |
| `registry.add_listener_local().global(closure).register()` | Listener builder API may differ — check `RegistryListener` docs |
| `pipewire::link::Link` as `ProxyT` for `create_object` | May need `pipewire::proxy::Link` or different path |
| `Rc<MainLoop>` — wrapping in Rc for callback access | If `MainLoop` has interior mutability issues, use raw `pw_main_loop_quit()` via `pipewire_sys` as last resort |

## Out of Scope

- Sound playback to virtual sink (Issue #5)
- Mic selection UI (user picks which mic — Phase 2/3)
- Multiple mic support (link ALL sources, not just first)
- Hot-plug handling (mic connected after engine start)
- Per-source volume control (Phase 4, per ADR-007)
