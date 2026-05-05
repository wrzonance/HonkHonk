# Playback to Virtual Sink + Monitor Output — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Play decoded PCM audio to the PipeWire virtual sink (Discord hears it) AND default output (user monitors in headset). Support play/stop/volume commands from the app.

**Architecture:** Two PipeWire output streams run on the engine thread — one targets the virtual sink by node ID, one targets default output (None). Both share decoded PCM data via an `Arc<Vec<f32>>`. A `PlaybackState` struct tracks cursor position, volume, and active flag. Commands arrive via the existing `pipewire::channel`, events flow back via `mpsc`.

**Tech Stack:** pipewire-rs 0.9 (StreamBox, process callback), existing symphonia decoder output (`DecodedAudio`), f32 interleaved PCM at source sample rate.

---

## File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Create | `src/audio/playback.rs` | PlaybackState, stream creation, process callbacks |
| Modify | `src/audio/engine.rs` | Wire commands → playback, expose sink_node_id |
| Modify | `src/audio/error.rs` | Add StreamCreation variant |
| Modify | `src/audio/mod.rs` | Re-export new public types |
| Modify | `src/app.rs` | Add Play/Stop messages, wire to AudioCommand |
| Create | `tests/playback_test.rs` | Unit tests for PlaybackState logic |
| Modify | `tests/pipewire_integration.rs` | Integration test: play sound, verify stream active |

---

## Task 1: Extend AudioCommand and AudioEvent enums

**Files:**
- Modify: `src/audio/engine.rs:10-18`
- Modify: `src/audio/error.rs`
- Modify: `src/audio/mod.rs`

- [ ] **Step 1: Add new error variant**

In `src/audio/error.rs`, add:

```rust
#[error("failed to create playback stream: {0}")]
StreamCreation(String),
```

- [ ] **Step 2: Extend AudioCommand enum**

In `src/audio/engine.rs`, replace the `AudioCommand` enum:

```rust
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play {
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
    },
    Stop,
    SetVolume(f32),
    Shutdown,
}
```

- [ ] **Step 3: Extend AudioEvent enum**

In `src/audio/engine.rs`, replace the `AudioEvent` enum:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Ready,
    PlaybackStarted { sound_id: String },
    PlaybackFinished { sound_id: String },
    Error(String),
}
```

- [ ] **Step 4: Update mod.rs re-exports**

In `src/audio/mod.rs`:

```rust
mod decoder;
mod engine;
mod error;
mod playback;

pub use decoder::{decode, DecodedAudio};
pub use engine::{spawn, AudioCommand, AudioEvent, AudioHandle};
pub use error::AudioError;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check`
Expected: Compile succeeds (playback module empty/stubbed next task)

- [ ] **Step 6: Commit**

```bash
git add src/audio/engine.rs src/audio/error.rs src/audio/mod.rs
git commit -m "feat(audio): extend AudioCommand/AudioEvent for playback"
```

---

## Task 2: Create PlaybackState (pure logic, no PipeWire)

**Files:**
- Create: `src/audio/playback.rs`
- Create: `tests/playback_test.rs`

- [ ] **Step 1: Write failing tests for PlaybackState**

Create `tests/playback_test.rs`:

```rust
use std::sync::Arc;

use honkhonk::audio::playback::PlaybackState;

#[test]
fn new_playback_state_is_inactive() {
    let state = PlaybackState::new();
    assert!(!state.is_active());
}

#[test]
fn start_activates_playback() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![0.0f32; 4800]);
    state.start("test-sound".into(), samples, 48000, 2);
    assert!(state.is_active());
    assert_eq!(state.sound_id(), Some("test-sound"));
}

#[test]
fn fill_buffer_writes_samples_and_advances_cursor() {
    let mut state = PlaybackState::new();
    // 10 frames of stereo (20 samples)
    let samples = Arc::new(vec![0.5f32; 20]);
    state.start("s1".into(), samples, 48000, 2);

    let mut buf = vec![0.0f32; 10]; // 5 frames * 2 channels
    let wrote = state.fill_buffer(&mut buf);
    assert_eq!(wrote, 10);
    assert!(buf.iter().all(|&s| s == 0.5));
}

#[test]
fn fill_buffer_applies_volume() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![1.0f32; 20]);
    state.start("s1".into(), samples, 48000, 2);
    state.set_volume(0.5);

    let mut buf = vec![0.0f32; 10];
    state.fill_buffer(&mut buf);
    assert!(buf.iter().all(|&s| (s - 0.5).abs() < 0.001));
}

#[test]
fn fill_buffer_returns_zero_when_exhausted() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![1.0f32; 4]); // 2 frames stereo
    state.start("s1".into(), samples, 48000, 2);

    let mut buf = vec![0.0f32; 4];
    let wrote = state.fill_buffer(&mut buf); // consumes all
    assert_eq!(wrote, 4);

    let mut buf2 = vec![0.0f32; 4];
    let wrote2 = state.fill_buffer(&mut buf2);
    assert_eq!(wrote2, 0);
    assert!(!state.is_active());
}

#[test]
fn stop_deactivates_and_resets() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![0.5f32; 100]);
    state.start("s1".into(), samples, 48000, 2);
    state.stop();
    assert!(!state.is_active());
    assert_eq!(state.sound_id(), None);
}

#[test]
fn fill_buffer_on_inactive_state_returns_zero() {
    let mut state = PlaybackState::new();
    let mut buf = vec![0.0f32; 10];
    let wrote = state.fill_buffer(&mut buf);
    assert_eq!(wrote, 0);
}

#[test]
fn set_volume_clamps_to_zero_one() {
    let mut state = PlaybackState::new();
    state.set_volume(1.5);
    assert_eq!(state.volume(), 1.0);
    state.set_volume(-0.3);
    assert_eq!(state.volume(), 0.0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test playback_test`
Expected: FAIL — `honkhonk::audio::playback` module not found or `PlaybackState` not defined

- [ ] **Step 3: Implement PlaybackState**

Create `src/audio/playback.rs`:

```rust
use std::sync::Arc;

pub struct PlaybackState {
    sound_id: Option<String>,
    samples: Option<Arc<Vec<f32>>>,
    cursor: usize,
    volume: f32,
    sample_rate: u32,
    channels: u16,
    active: bool,
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            sound_id: None,
            samples: None,
            cursor: 0,
            volume: 1.0,
            sample_rate: 48000,
            channels: 2,
            active: false,
        }
    }

    pub fn start(
        &mut self,
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
    ) {
        self.sound_id = Some(sound_id);
        self.samples = Some(samples);
        self.cursor = 0;
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.sound_id = None;
        self.samples = None;
        self.cursor = 0;
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn sound_id(&self) -> Option<&str> {
        self.sound_id.as_deref()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn fill_buffer(&mut self, buf: &mut [f32]) -> usize {
        let samples = match &self.samples {
            Some(s) if self.active => s,
            _ => return 0,
        };

        let remaining = samples.len().saturating_sub(self.cursor);
        let to_write = buf.len().min(remaining);

        if to_write == 0 {
            self.active = false;
            return 0;
        }

        let src = &samples[self.cursor..self.cursor + to_write];
        for (dst, &sample) in buf[..to_write].iter_mut().zip(src.iter()) {
            *dst = sample * self.volume;
        }

        self.cursor += to_write;

        if self.cursor >= samples.len() {
            self.active = false;
        }

        to_write
    }
}
```

- [ ] **Step 4: Make PlaybackState public from module**

In `src/audio/mod.rs`, add:

```rust
pub mod playback;
```

(Replace the `mod playback;` line from Task 1 Step 4 with `pub mod playback;`)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test playback_test`
Expected: All 8 tests PASS

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 7: Commit**

```bash
git add src/audio/playback.rs tests/playback_test.rs src/audio/mod.rs
git commit -m "feat(audio): add PlaybackState with volume and buffer fill logic"
```

---

## Task 3: Create PipeWire output streams in playback.rs

**Files:**
- Modify: `src/audio/playback.rs`
- Modify: `src/audio/engine.rs`

This task adds `create_sink_stream` and `create_monitor_stream` — two functions that create PipeWire `StreamBox` instances targeting (a) the virtual sink by node ID and (b) the default output device. Both share a `PlaybackState` via `Rc<RefCell<_>>`.

- [ ] **Step 1: Add stream creation functions to playback.rs**

Append to `src/audio/playback.rs`:

```rust
use std::cell::RefCell;
use std::rc::Rc;

use pipewire as pw;
use pw::spa;
use pw::spa::pod::Pod;

use super::error::AudioError;

const FRAME_SIZE: usize = std::mem::size_of::<f32>();

fn build_audio_params(rate: u32, channels: u32) -> Vec<u8> {
    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::F32LE);
    audio_info.set_rate(rate);
    audio_info.set_channels(channels);

    let mut position = [0u32; spa::param::audio::MAX_CHANNELS];
    if channels >= 1 {
        position[0] = spa_sys::SPA_AUDIO_CHANNEL_FL;
    }
    if channels >= 2 {
        position[1] = spa_sys::SPA_AUDIO_CHANNEL_FR;
    }
    audio_info.set_position(position);

    pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(pw::spa::pod::Object {
            type_: spa_sys::SPA_TYPE_OBJECT_Format,
            id: spa_sys::SPA_PARAM_EnumFormat,
            properties: audio_info.into(),
        }),
    )
    .expect("pod serialization cannot fail for valid AudioInfoRaw")
    .0
    .into_inner()
}

pub fn create_sink_stream(
    core: &pw::core::CoreRc,
    state: Rc<RefCell<PlaybackState>>,
    sink_node_id: u32,
    sample_rate: u32,
    channels: u16,
) -> Result<pw::stream::StreamBox<()>, AudioError> {
    let stream = pw::stream::StreamBox::new(
        core,
        "honkhonk-to-sink",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::MEDIA_CATEGORY => "Playback",
            *pw::keys::TARGET_OBJECT => sink_node_id.to_string(),
            *pw::keys::AUDIO_CHANNELS => channels.to_string(),
        },
    )
    .map_err(|e| AudioError::StreamCreation(format!("sink stream: {e}")))?;

    let _listener = stream
        .add_local_listener_with_user_data(())
        .process(move |stream, _| {
            if let Some(mut buffer) = stream.dequeue_buffer() {
                let datas = buffer.datas_mut();
                if let Some(data) = datas.first_mut() {
                    if let Some(slice) = data.data() {
                        let float_slice = bytemuck_cast_f32_mut(slice);
                        let mut ps = state.borrow_mut();
                        let wrote = ps.fill_buffer(float_slice);
                        // Zero remainder
                        for s in float_slice[wrote..].iter_mut() {
                            *s = 0.0;
                        }
                        let chunk = data.chunk_mut();
                        *chunk.offset_mut() = 0;
                        *chunk.stride_mut() = (FRAME_SIZE * channels as usize) as i32;
                        *chunk.size_mut() = slice.len() as u32;
                    }
                }
            }
        })
        .register()
        .map_err(|e| AudioError::StreamCreation(format!("sink listener: {e}")))?;

    let params_bytes = build_audio_params(sample_rate, channels as u32);
    let mut params = [Pod::from_bytes(&params_bytes).unwrap()];

    stream
        .connect(
            spa::utils::Direction::Output,
            Some(sink_node_id),
            pw::stream::StreamFlags::AUTOCONNECT
                | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(|e| AudioError::StreamCreation(format!("sink connect: {e}")))?;

    Ok(stream)
}

pub fn create_monitor_stream(
    core: &pw::core::CoreRc,
    state: Rc<RefCell<PlaybackState>>,
    sample_rate: u32,
    channels: u16,
) -> Result<pw::stream::StreamBox<()>, AudioError> {
    let stream = pw::stream::StreamBox::new(
        core,
        "honkhonk-monitor",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::MEDIA_CATEGORY => "Playback",
            *pw::keys::AUDIO_CHANNELS => channels.to_string(),
        },
    )
    .map_err(|e| AudioError::StreamCreation(format!("monitor stream: {e}")))?;

    let _listener = stream
        .add_local_listener_with_user_data(())
        .process(move |stream, _| {
            if let Some(mut buffer) = stream.dequeue_buffer() {
                let datas = buffer.datas_mut();
                if let Some(data) = datas.first_mut() {
                    if let Some(slice) = data.data() {
                        let float_slice = bytemuck_cast_f32_mut(slice);
                        let mut ps = state.borrow_mut();
                        let wrote = ps.fill_buffer(float_slice);
                        for s in float_slice[wrote..].iter_mut() {
                            *s = 0.0;
                        }
                        let chunk = data.chunk_mut();
                        *chunk.offset_mut() = 0;
                        *chunk.stride_mut() = (FRAME_SIZE * channels as usize) as i32;
                        *chunk.size_mut() = slice.len() as u32;
                    }
                }
            }
        })
        .register()
        .map_err(|e| AudioError::StreamCreation(format!("monitor listener: {e}")))?;

    let params_bytes = build_audio_params(sample_rate, channels as u32);
    let mut params = [Pod::from_bytes(&params_bytes).unwrap()];

    stream
        .connect(
            spa::utils::Direction::Output,
            None,
            pw::stream::StreamFlags::AUTOCONNECT
                | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(|e| AudioError::StreamCreation(format!("monitor connect: {e}")))?;

    Ok(stream)
}

fn bytemuck_cast_f32_mut(bytes: &mut [u8]) -> &mut [f32] {
    let len = bytes.len() / FRAME_SIZE;
    let ptr = bytes.as_mut_ptr() as *mut f32;
    unsafe { std::slice::from_raw_parts_mut(ptr, len) }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles (streams not yet wired into engine)

- [ ] **Step 3: Commit**

```bash
git add src/audio/playback.rs
git commit -m "feat(audio): add PipeWire stream creation for sink and monitor output"
```

---

## Task 4: Wire playback into the engine main loop

**Files:**
- Modify: `src/audio/engine.rs`

This task makes `run_engine` respond to `Play`, `Stop`, and `SetVolume` commands by managing `PlaybackState` and creating/destroying streams.

- [ ] **Step 1: Refactor engine to support playback commands**

Replace the command handler section in `src/audio/engine.rs`. The full updated `run_engine` function:

```rust
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;

use super::error::AudioError;
use super::playback::{self, PlaybackState};

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SINK_DESCRIPTION: &str = "HonkHonk Mix";

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play {
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
    },
    Stop,
    SetVolume(f32),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Ready,
    PlaybackStarted { sound_id: String },
    PlaybackFinished { sound_id: String },
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

    pub fn send(&self, cmd: AudioCommand) {
        let _ = self.cmd_tx.send(cmd);
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
    if state.sink_input_ports.is_empty() || state.source_output_ports.is_empty() {
        return;
    }

    let mut all_ok = true;
    for (src_port, sink_port) in state
        .source_output_ports
        .iter()
        .zip(state.sink_input_ports.iter())
    {
        let link_props = pipewire::properties::properties! {
            "link.output.port" => src_port.to_string(),
            "link.input.port" => sink_port.to_string(),
            "object.linger" => "false",
        };
        match core.create_object::<pipewire::link::Link>("link-factory", &link_props) {
            Ok(link) => links.push(link),
            Err(e) => {
                eprintln!("honkhonk: failed to create mic passthrough link: {e}");
                all_ok = false;
            }
        }
    }

    state.links_created = all_ok;
}

fn create_virtual_sink(
    core: &pipewire::core::CoreRc,
) -> Result<pipewire::node::Node, AudioError> {
    let sink_props = pipewire::properties::properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => SINK_NODE_NAME,
        "node.description" => SINK_DESCRIPTION,
        "media.class" => "Audio/Sink/Virtual",
        "audio.position" => "[FL,FR]",
        "object.linger" => "false",
    };
    core.create_object("adapter", &sink_props)
        .map_err(|e| AudioError::VirtualSinkCreation(e.to_string()))
}

fn handle_registry_global(
    global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
    state: &mut RegistryState,
) {
    let props = match global.props {
        Some(p) => p,
        None => return,
    };

    match global.type_ {
        pipewire::types::ObjectType::Node => {
            let name = props.get("node.name").unwrap_or("");
            let class = props.get("media.class").unwrap_or("");

            if name == SINK_NODE_NAME {
                state.sink_node_id = Some(global.id);
            } else if class == "Audio/Source" && state.source_node_id.is_none() {
                state.source_node_id = Some(global.id);
            }
        }
        pipewire::types::ObjectType::Port => {
            let node_id: u32 = props
                .get("node.id")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let direction = props.get("port.direction").unwrap_or("");

            if Some(node_id) == state.sink_node_id && direction == "in" {
                state.sink_input_ports.push(global.id);
            } else if Some(node_id) == state.source_node_id && direction == "out" {
                state.source_output_ports.push(global.id);
            }
        }
        _ => {}
    }
}

struct RegistryGuard<'a> {
    _registry: pipewire::registry::RegistryBox<'a>,
    _listener: pipewire::registry::Listener,
    _links: Rc<RefCell<Vec<pipewire::link::Link>>>,
}

fn setup_registry_listener(
    core: &pipewire::core::CoreRc,
) -> Result<RegistryGuard<'_>, AudioError> {
    let state = Rc::new(RefCell::new(RegistryState::default()));
    let mic_links: Rc<RefCell<Vec<pipewire::link::Link>>> =
        Rc::new(RefCell::new(Vec::new()));

    let registry = core
        .get_registry()
        .map_err(|e| AudioError::PipeWireInit(format!("registry: {e}")))?;

    let state_ref = state.clone();
    let links_ref = mic_links.clone();
    let core_ref = core.clone();
    let listener = registry
        .add_listener_local()
        .global(move |global| {
            let mut s = state_ref.borrow_mut();
            handle_registry_global(global, &mut s);
            let mut link_store = links_ref.borrow_mut();
            try_create_links(&mut s, &core_ref, &mut link_store);
        })
        .register();

    Ok(RegistryGuard {
        _registry: registry,
        _listener: listener,
        _links: mic_links,
    })
}

struct PlaybackStreams {
    _sink_stream: pipewire::stream::StreamBox<()>,
    _monitor_stream: pipewire::stream::StreamBox<()>,
}

fn run_engine(
    cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
) -> Result<(), AudioError> {
    let mainloop = pipewire::main_loop::MainLoopRc::new(None)
        .map_err(|e| AudioError::PipeWireInit(format!("main loop: {e}")))?;

    let context = pipewire::context::ContextRc::new(&mainloop, None)
        .map_err(|e| AudioError::PipeWireInit(format!("context: {e}")))?;

    let core = context
        .connect_rc(None)
        .map_err(|e| AudioError::PipeWireInit(format!("core connect: {e}")))?;

    let _sink = create_virtual_sink(&core)?;
    let _registry_guard = setup_registry_listener(&core)?;

    let playback_state = Rc::new(RefCell::new(PlaybackState::new()));
    let mut streams: Option<PlaybackStreams> = None;

    let mainloop_quit = mainloop.clone();
    let ps_cmd = playback_state.clone();
    let core_cmd = core.clone();
    let _cmd_listener = cmd_rx.attach(mainloop.loop_(), move |cmd| match cmd {
        AudioCommand::Play { sound_id, samples, sample_rate, channels } => {
            // Get sink node ID from registry
            let sink_node_id = {
                // We need the registry state's sink_node_id.
                // For now, scan nodes to find ours. The registry listener
                // already populated it — but we stored it in a closure.
                // Workaround: use a shared cell for sink_node_id.
                // This is handled via the registry_sink_id shared below.
                0u32 // placeholder — replaced in Step 2
            };
            let _ = sink_node_id; // suppress warning, replaced below

            ps_cmd.borrow_mut().start(
                sound_id.clone(),
                samples,
                sample_rate,
                channels,
            );

            let _ = evt_tx.send(AudioEvent::PlaybackStarted { sound_id });
        }
        AudioCommand::Stop => {
            let finished_id = ps_cmd.borrow().sound_id().map(String::from);
            ps_cmd.borrow_mut().stop();
            if let Some(id) = finished_id {
                let _ = evt_tx.send(AudioEvent::PlaybackFinished { sound_id: id });
            }
        }
        AudioCommand::SetVolume(v) => {
            ps_cmd.borrow_mut().set_volume(v);
        }
        AudioCommand::Shutdown => mainloop_quit.quit(),
    });

    let _ = evt_tx.send(AudioEvent::Ready);
    mainloop.run();

    Ok(())
}
```

**Note:** The sink_node_id placeholder is resolved in Step 2 below.

- [ ] **Step 2: Share sink_node_id between registry and command handler**

Add a shared `Rc<Cell<Option<u32>>>` for the sink node ID that both the registry listener and command handler can access. Modify `run_engine` to thread this through:

Replace the `run_engine` function with proper shared state. Add before the registry setup:

```rust
let registry_sink_id: Rc<std::cell::Cell<Option<u32>>> =
    Rc::new(std::cell::Cell::new(None));
```

Modify `setup_registry_listener` to accept and update this cell (or alternatively, move the registry state to be accessible from outside its closure). The cleanest approach: pass `registry_sink_id` clone into `handle_registry_global` closure and set it when sink node appears.

Update the `Play` command handler:

```rust
AudioCommand::Play { sound_id, samples, sample_rate, channels } => {
    let sink_id = match registry_sink_id_cmd.get() {
        Some(id) => id,
        None => {
            let _ = evt_tx.send(AudioEvent::Error(
                "virtual sink not yet registered".into()
            ));
            return;
        }
    };

    ps_cmd.borrow_mut().start(
        sound_id.clone(),
        samples,
        sample_rate,
        channels,
    );

    // Create streams (or reuse if format matches)
    let sink_state = ps_cmd.clone();
    let mon_state = ps_cmd.clone();
    match (
        playback::create_sink_stream(&core_cmd, sink_state, sink_id, sample_rate, channels),
        playback::create_monitor_stream(&core_cmd, mon_state, sample_rate, channels),
    ) {
        (Ok(sink_s), Ok(mon_s)) => {
            *streams_cmd.borrow_mut() = Some(PlaybackStreams {
                _sink_stream: sink_s,
                _monitor_stream: mon_s,
            });
            let _ = evt_tx.send(AudioEvent::PlaybackStarted { sound_id });
        }
        (Err(e), _) | (_, Err(e)) => {
            ps_cmd.borrow_mut().stop();
            let _ = evt_tx.send(AudioEvent::Error(e.to_string()));
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles

- [ ] **Step 4: Run existing tests still pass**

Run: `cargo test --test playback_test --test app_test --test decoder_test`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/audio/engine.rs
git commit -m "feat(audio): wire Play/Stop/SetVolume commands into PipeWire engine loop"
```

---

## Task 5: Detect playback finished and emit event

**Files:**
- Modify: `src/audio/engine.rs`

The process callbacks fill buffers until `PlaybackState::is_active()` returns false. We need a mechanism to detect this and send `PlaybackFinished`. PipeWire process callbacks run on the main loop thread, so we can check after each iteration.

- [ ] **Step 1: Add a timer to poll playback completion**

In `run_engine`, after streams are created, add a timer that checks if playback finished:

```rust
let ps_timer = playback_state.clone();
let evt_tx_timer = evt_tx.clone();
let timer_source = mainloop.loop_().add_timer(move |_| {
    let ps = ps_timer.borrow();
    if !ps.is_active() {
        if let Some(id) = ps.sound_id() {
            let _ = evt_tx_timer.send(AudioEvent::PlaybackFinished {
                sound_id: id.to_string(),
            });
        }
    }
});
```

Alternative (simpler): Check in the process callback itself. When `fill_buffer` returns 0 and state becomes inactive, set a flag. The command loop's next iteration picks it up. Since PipeWire callbacks are on the same thread as the main loop, a simple `Rc<Cell<bool>>` works:

```rust
let finished_flag: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
```

Pass a clone into both stream process callbacks. When `fill_buffer` returns `wrote == 0` and state is inactive, set `finished_flag.set(true)`.

Add an idle callback or timer (100ms interval) that checks `finished_flag`, sends `PlaybackFinished`, drops streams, and resets the flag.

- [ ] **Step 2: Verify it compiles and existing tests pass**

Run: `cargo check && cargo test --test playback_test`
Expected: Both pass

- [ ] **Step 3: Commit**

```bash
git add src/audio/engine.rs
git commit -m "feat(audio): emit PlaybackFinished event when sound completes"
```

---

## Task 6: Wire app.rs to send Play commands

**Files:**
- Modify: `src/app.rs`
- Modify: `tests/app_test.rs`

- [ ] **Step 1: Write failing test for play message**

Add to `tests/app_test.rs`:

```rust
#[test]
fn audio_playback_started_event_is_handled() {
    let mut app = HonkHonk::new_for_test();
    let event = honkhonk::audio::AudioEvent::PlaybackStarted {
        sound_id: "test".into(),
    };
    let _task = app.update(Message::AudioEvent(event));
    // Should not crash, should not exit
    assert!(!app.should_exit());
}

#[test]
fn audio_playback_finished_event_is_handled() {
    let mut app = HonkHonk::new_for_test();
    let event = honkhonk::audio::AudioEvent::PlaybackFinished {
        sound_id: "test".into(),
    };
    let _task = app.update(Message::AudioEvent(event));
    assert!(!app.should_exit());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test app_test`
Expected: FAIL — pattern match not exhaustive for new AudioEvent variants

- [ ] **Step 3: Update app.rs AudioEvent handler**

In `src/app.rs`, update the `Message::AudioEvent` match:

```rust
Message::AudioEvent(event) => {
    match event {
        crate::audio::AudioEvent::Ready => {
            eprintln!("honkhonk: audio engine ready");
        }
        crate::audio::AudioEvent::PlaybackStarted { sound_id } => {
            eprintln!("honkhonk: playback started: {sound_id}");
        }
        crate::audio::AudioEvent::PlaybackFinished { sound_id } => {
            eprintln!("honkhonk: playback finished: {sound_id}");
        }
        crate::audio::AudioEvent::Error(e) => {
            eprintln!("honkhonk: audio error: {e}");
        }
    }
    Task::none()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test app_test`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/app.rs tests/app_test.rs
git commit -m "feat(app): handle PlaybackStarted/Finished audio events"
```

---

## Task 7: Integration test — play decoded audio through engine

**Files:**
- Modify: `tests/pipewire_integration.rs`

- [ ] **Step 1: Write integration test**

Add to `tests/pipewire_integration.rs`:

```rust
#[test]
fn play_sound_emits_started_and_finished_events() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("failed to spawn audio engine");

    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no Ready event");
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    // Wait for registry to discover sink node ID
    std::thread::sleep(Duration::from_secs(1));

    // Decode a short test fixture
    let decoded = honkhonk::audio::decode(
        std::path::Path::new("tests/fixtures/sine_mono.wav")
    ).expect("decode failed");

    let samples = std::sync::Arc::new(decoded.samples);

    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "test-sine".into(),
        samples,
        sample_rate: decoded.sample_rate,
        channels: decoded.channels,
    });

    // Should get PlaybackStarted
    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no PlaybackStarted event");
    assert!(
        matches!(event, honkhonk::audio::AudioEvent::PlaybackStarted { ref sound_id } if sound_id == "test-sine"),
        "expected PlaybackStarted, got: {event:?}"
    );

    // Should get PlaybackFinished within a few seconds (1s audio)
    let event = handle
        .recv_timeout(Duration::from_secs(10))
        .expect("no PlaybackFinished event");
    assert!(
        matches!(event, honkhonk::audio::AudioEvent::PlaybackFinished { ref sound_id } if sound_id == "test-sine"),
        "expected PlaybackFinished, got: {event:?}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn stop_command_halts_playback() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("spawn failed");
    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    std::thread::sleep(Duration::from_secs(1));

    // Use a long sound (repeat samples to ~5 seconds)
    let samples = std::sync::Arc::new(vec![0.3f32; 48000 * 5 * 2]); // 5s stereo

    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "long-sound".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
    });

    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::PlaybackStarted { .. }));

    // Stop after 500ms
    std::thread::sleep(Duration::from_millis(500));
    handle.send(honkhonk::audio::AudioCommand::Stop);

    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        matches!(event, honkhonk::audio::AudioEvent::PlaybackFinished { ref sound_id } if sound_id == "long-sound"),
        "expected PlaybackFinished after stop, got: {event:?}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --features pipewire-test --test pipewire_integration`
Expected: All pass (requires running PipeWire session)

- [ ] **Step 3: Commit**

```bash
git add tests/pipewire_integration.rs
git commit -m "test(audio): integration tests for play/stop through PipeWire engine"
```

---

## Task 8: Final verification and cleanup

**Files:**
- All modified files

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All unit tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Run integration tests**

Run: `cargo test --features pipewire-test`
Expected: All pass

- [ ] **Step 4: Verify LOC delta**

Run: `git diff --stat main...HEAD | tail -1`
Expected: Under 500 lines changed

- [ ] **Step 5: Manual smoke test**

If running with PipeWire desktop session:
1. Run `cargo run`
2. From another terminal, verify `wpctl status` shows "HonkHonk Mix"
3. (After UI wiring in Issue #7, click-to-play will be testable)

- [ ] **Step 6: Final commit if any cleanup needed**

```bash
git add -A
git commit -m "chore(audio): cleanup after playback implementation"
```

---

## Design Decisions

1. **Two separate PlaybackState instances vs one shared:** Using one shared `PlaybackState` means both streams read from the same cursor — they'd fight. Instead, each stream gets its own clone of the `Rc<RefCell<PlaybackState>>`. **Wait — that means double reads.** Fix: use two separate `PlaybackState` instances, one for sink, one for monitor, both initialized with the same `Arc<Vec<f32>>` samples. Each advances independently. This is correct — both outputs play the same audio without cursor contention.

2. **Stream lifecycle:** Create streams on `Play`, drop on `Stop` or `PlaybackFinished`. No persistent streams when idle — avoids unnecessary PipeWire graph nodes when nothing is playing.

3. **No resampling in v1:** If the decoded sample rate differs from PipeWire's configured rate, PipeWire's built-in SPA resampler handles it (streams negotiate format). Explicit resampling is Phase 3 polish.

4. **`unsafe` in `bytemuck_cast_f32_mut`:** The PipeWire buffer gives `&mut [u8]`. We need `&mut [f32]`. This is safe because: (a) we request F32LE format, (b) PipeWire aligns buffers to frame size, (c) `f32` has no invalid bit patterns. Could use `bytemuck` crate but adding a dep for one cast is not justified.
