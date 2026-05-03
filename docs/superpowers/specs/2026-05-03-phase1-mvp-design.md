# Phase 1 MVP Design — HonkHonk

## Summary

Phase 1 delivers a click-to-play soundboard that routes audio to a PipeWire virtual mic (for Discord/voice chat) and local headset (for self-monitoring). Pure Rust application using Iced for GUI. No hotkeys — click-to-play only.

## Tech Stack

| Component | Technology | Why |
|-----------|-----------|-----|
| GUI | Iced 0.13 (wgpu default, tiny-skia fallback) | Pure Rust, Elm architecture, MIT license, Wayland-native |
| Audio | pipewire-rs 0.8 | Official PipeWire Rust bindings, persistent virtual sink |
| Decode | symphonia 0.5 | Pure Rust, MP3/OGG/FLAC/WAV/AAC |
| Tray | tray-icon 0.19 + muda 0.15 | Active maintenance (Tauri team), standalone SNI |
| Errors | thiserror 2 + anyhow 1 | Typed enums at boundaries, context chains in glue |
| Config | serde + serde_json | XDG-compliant JSON config |
| Async | tokio | Iced integration, async commands |
| XDG paths | directories 6 | Cross-distro path resolution |

### Renderer Selection

Default: wgpu (GPU). Override via environment variable:

```
HONKHONK_RENDERER=software honkhonk
```

Compiles with both `wgpu` and `tiny-skia` features. No auto-fallback — explicit user choice.

### Runtime Dependencies

- pipewire >= 1.0
- Vulkan/Mesa drivers (wgpu) OR nothing extra (tiny-skia)
- wayland-client libs

### Build Dependencies

- rust >= 1.75
- pkg-config
- pipewire-devel / libpipewire-0.3-dev
- wayland-devel / libwayland-dev

## Project Structure

```
honkhonk/
├── src/
│   ├── main.rs              # Entry, renderer selection, app launch
│   ├── app.rs               # Iced Application impl (state, update, view)
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── sound_grid.rs    # Grid of sound cards
│   │   ├── sound_card.rs    # Individual sound button/card
│   │   ├── search_bar.rs    # Search input
│   │   ├── volume.rs        # Volume slider
│   │   └── theme.rs         # Custom theme (colors, spacing)
│   ├── audio/
│   │   ├── mod.rs
│   │   ├── error.rs         # AudioError enum (thiserror)
│   │   ├── engine.rs        # PipeWire lifecycle (virtual sink, mic passthrough)
│   │   ├── decoder.rs       # symphonia → PCM samples
│   │   ├── mixer.rs         # Mix mic + playback into virtual sink
│   │   └── playback.rs      # Play sound to sink + monitor output
│   ├── tray/
│   │   ├── mod.rs
│   │   └── icon.rs          # tray-icon setup, menu, quit handler
│   └── state/
│       ├── mod.rs
│       ├── error.rs         # ConfigError enum
│       ├── config.rs        # App settings (serde JSON)
│       └── library.rs       # Sound file index + metadata
├── assets/
│   └── icons/               # App icon, tray icon
├── packaging/
│   └── flatpak/
│       └── io.github.thewrz.HonkHonk.yml
├── tests/
│   └── fixtures/            # Short audio files for decode tests
├── docs/
│   └── adr/
├── Cargo.toml
├── clippy.toml
└── deny.toml
```

## Application Architecture

### Iced Application (Elm/MVU)

```rust
struct HonkHonk {
    sounds: Vec<SoundEntry>,
    query: String,
    volume: f32,
    playing: Option<SoundId>,
    audio: AudioHandle,
    library: Library,
    config: AppConfig,
}

enum Message {
    // UI events
    SearchChanged(String),
    PlaySound(SoundId),
    StopAll,
    VolumeChanged(f32),

    // Backend events (from subscriptions)
    AudioEvent(AudioEvent),
    TrayEvent(TrayEvent),

    // Async results
    LibraryScanned(Vec<SoundEntry>),
    AudioEngineReady(Result<AudioHandle, AudioError>),
}
```

### Audio ↔ UI Communication

No IPC. Direct channel communication:

1. **Commands (UI → Audio):** `Message::PlaySound` triggers Iced `Command::perform` which sends `AudioCommand` through channel to PipeWire thread.

2. **Subscriptions (Audio → UI):** Iced `Subscription` wraps channel receiver. Audio engine sends `AudioEvent` back. Iced polls each frame.

### Tray Integration

`tray-icon` initializes before Iced event loop on main thread. Communicates via channel → Iced Subscription. Menu: "Show/Hide", separator, "Quit".

### Error Flow

```
audio::engine → AudioError (thiserror)
    → channel → Message::AudioEvent(AudioEvent::Error(..))
        → UI displays error banner
```

No panics. No unwraps in non-test code. Errors surface as messages in Elm loop.

## PipeWire Audio Engine

### Architecture

```
Physical Mic ──────────────┐
                           ▼
HonkHonk Playback ──→ HonkHonk Mix (virtual sink) ──→ "HonkHonk Mic" (source)
                                                              │
                                                              ▼
                                                         Discord / App

HonkHonk Playback ──→ Default Output (monitor — user hears sound)
```

### Lifecycle

1. App start → connect to PipeWire server
2. Create virtual sink node ("HonkHonk Mix")
3. Create virtual source node ("HonkHonk Mic")
4. Link physical mic → virtual sink (passthrough)
5. Ready for playback
6. App quit → destroy nodes → disconnect

### Key Types

```rust
pub struct AudioEngine {
    core: pipewire::core::Core,
    main_loop: pipewire::main_loop::MainLoop,
    sink_node: Node,
    source_node: Node,
    mic_link: Link,
    event_tx: Sender<AudioEvent>,
}

pub struct AudioHandle {
    cmd_tx: Sender<AudioCommand>,
    event_rx: Receiver<AudioEvent>,
}

pub enum AudioCommand {
    Play { sound_id: SoundId, pcm: Arc<DecodedAudio> },
    Stop,
    SetVolume(f32),
    Shutdown,
}

pub enum AudioEvent {
    Ready,
    PlaybackStarted(SoundId),
    PlaybackFinished(SoundId),
    Error(AudioError),
}
```

### Threading Model

PipeWire runs its own event loop on a dedicated thread. Communication via bounded channels:
- `AudioHandle` lives in Iced app state (main thread)
- `AudioEngine` lives on PipeWire thread
- Commands: main → PipeWire thread
- Events: PipeWire thread → main

### Playback Flow

1. User clicks sound → `Message::PlaySound(id)`
2. Update: decode file (symphonia → `DecodedAudio`), send `AudioCommand::Play`
3. PipeWire thread: write PCM samples to sink stream + monitor stream
4. Done → send `AudioEvent::PlaybackFinished`

### Persistent Sink (no per-sound nodes)

One sink exists for app lifetime. Playback writes samples into existing stream. No PipeWire graph reconfiguration during playback. No audio dropouts.

## Sound Library

### Scanning

- Default directory: `$XDG_MUSIC_DIR/HonkHonk/`
- Recursive walk, filter by extension (mp3, ogg, flac, wav, aac)
- Async on startup → `Message::LibraryScanned`
- No file watcher in Phase 1 — manual rescan button

### Types

```rust
pub struct Library {
    directories: Vec<PathBuf>,
    sounds: Vec<SoundEntry>,
}

pub struct SoundEntry {
    pub id: SoundId,
    pub name: String,
    pub path: PathBuf,
    pub format: AudioFormat,
    pub duration: Option<Duration>,
}
```

### Decoding Strategy

Lazy: decode on play, not on scan. Keeps startup fast.

### Config Persistence

```rust
pub struct AppConfig {
    pub sound_directories: Vec<PathBuf>,
    pub volume: f32,
    pub window_size: (u32, u32),
}
```

Stored at `$XDG_CONFIG_HOME/honkhonk/config.json`.

## Sub-MVP PR Sequence (Risk-First)

| PR | Title | Delivers | ~LOC |
|----|-------|----------|------|
| 1 | `feat: iced window + tray with quit` | Empty window renders, tray icon shows, quit works | ~300 |
| 2 | `feat(audio): pipewire virtual sink + mic passthrough` | Virtual sink in `wpctl status`, mic passes through | ~450 |
| 3 | `feat(audio): symphonia decode to PCM` | Decode MP3/OGG/FLAC/WAV → raw samples, unit tested | ~350 |
| 4 | `feat(audio): playback to sink + monitor` | Play decoded audio → virtual mic + headset | ~400 |
| 5 | `feat(state): sound library scanning` | Scan directory, produce SoundEntry list, config persistence | ~300 |
| 6 | `feat(ui): sound grid + click to play` | Grid renders sounds, click plays, stop button works | ~400 |
| 7 | `feat(ui): search + volume controls` | Filter grid by name, volume slider adjusts playback | ~300 |
| 8 | `chore: flatpak packaging` | Flatpak manifest, builds, runs | ~200 |

Each PR: passes CI independently, has test plan, TDD (failing test first), mergeable to main.

## Testing Strategy

### Unit Tests

| Module | Tests | Method |
|--------|-------|--------|
| `audio::decoder` | Correct sample count, sample rate per format | Test fixtures in `tests/fixtures/` |
| `audio::engine` | Virtual sink creation, cleanup on drop | Integration test (requires PipeWire) |
| `state::library` | Scanning finds correct files, ignores non-audio | `tempfile` with fake directory trees |
| `state::config` | Serialize/deserialize round-trip, missing file defaults | Unit test with temp paths |
| `app` (update fn) | State transitions for each Message variant | Unit test with mock messages |

### Integration Tests

| Test | Requires | Guard |
|------|----------|-------|
| PipeWire sink creation | Running PipeWire | `#[cfg(feature = "pipewire-test")]` |
| Full playback pipeline | PipeWire + audio file | Same feature gate |

### CI

```yaml
jobs:
  lint:
    - cargo clippy -- -D warnings
    - cargo fmt -- --check
  test:
    - cargo test
    - cargo test --features pipewire-test  # when PipeWire available
  build:
    - cargo build --release
```

### Coverage

80% target on non-UI code. View functions not unit tested — validated manually.

### Not Tested

- Iced view rendering (framework responsibility)
- PipeWire internals
- tray-icon library behavior

## Explicitly Out of Scope

- Global hotkeys (Phase 2)
- Favorites / recently played (Phase 3)
- Per-sound volume (Phase 3)
- Overlap / interrupt mode (Phase 3)
- File watcher / auto-rescan
- Sound previews / waveform display
- Drag-and-drop import
- Themes / dark mode (Phase 3)
- Any X11 code
- Any PulseAudio direct calls
