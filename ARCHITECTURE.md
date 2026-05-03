# HonkHonk — Architecture Specification

> A modern, Wayland-native soundboard for Linux. Play meme sounds in Discord, games, and voice chat with style.

## Vision

A polished, VoiceMod-quality soundboard that works natively on Wayland + PipeWire + KDE6 Plasma. Built for the modern Linux desktop from day one.

## Problem Statement

The Linux desktop has matured significantly — Wayland compositors are the default, PipeWire replaced PulseAudio, and xdg-desktop-portal provides standardized APIs for global shortcuts. But soundboard apps haven't caught up yet. Most existing options were built for X11 and PulseAudio, and Wayland support remains an afterthought.

The key technical blocker — global hotkeys on Wayland — is now solved via `xdg-desktop-portal GlobalShortcuts v2`, which KDE6, GNOME 45+, and Hyprland all implement. HonkHonk is built on this foundation.

## Design Principles

1. **Ship early, iterate.** Soundux died from scope creep. Phase 1 MVP has ONE job: play sounds into Discord.
2. **Use existing libraries.** Don't rewrite what's solved. PipeWire bindings, portal APIs, audio decoders — all exist.
3. **Wayland-native from day one.** No X11 fallbacks. No XWayland hacks. If it doesn't work on Wayland, it doesn't ship.
4. **Look good.** The UI is the product. A soundboard with an ugly UI is a soundboard nobody uses.
5. **Single toolchain.** Pure Rust. One language, one build system, minimal external dependencies.

## Tech Stack

| Component | Technology | Why |
|-----------|------------|-----|
| Language | **Rust** | Single language for everything — GUI, audio, system integration. Memory safety. Strong PipeWire/portal ecosystem |
| GUI | **Iced 0.13** | Pure Rust, Elm/MVU architecture, wgpu GPU rendering, Wayland-native via winit. MIT license |
| Renderer | **wgpu** (default) / **tiny-skia** (fallback) | GPU-accelerated by default, software renderer via env var for edge cases |
| PipeWire | **pipewire-rs 0.8** | Official Rust bindings from PipeWire project. Production-proven |
| Global Shortcuts | **ashpd 0.13** (`global_shortcuts` feature) | Full xdg-desktop-portal GlobalShortcuts API. Async/tokio |
| System Tray | **tray-icon 0.19** + **muda 0.15** | Actively maintained (Tauri team), standalone SNI, cross-DE |
| Audio Decode | **symphonia 0.5** | Pure Rust. MP3, WAV, OGG, FLAC, AAC. No C dependencies |
| Audio Playback | **pipewire-rs** streams | Direct PipeWire playback — no rodio/ALSA intermediary |

### Why Iced over alternatives

| Option | Rejected Because |
|--------|-----------------|
| Tauri v2 + Svelte | WebKitGTK dep (~50MB), two languages (Rust+TS), Node.js toolchain, IPC serialization overhead. Overkill for a grid-of-buttons UI |
| Qt6/QML | C++ complexity. QML learning curve. rohrkabel (C++23 PipeWire wrapper) has solo maintainer risk |
| Electron | 150MB binary. Memory hog. Not native |
| GTK4 | GNOME design language looks foreign on KDE6 |
| Slint | GPL or commercial license. Conflicts with MIT project |
| egui | Immediate mode, "dev tools" aesthetic. Not suitable for consumer-facing UI |

Iced gives us: pure Rust (single `cargo build`), Elm architecture (immutable state, message-driven updates), wgpu GPU rendering, Wayland-native via winit, MIT license, no WebKitGTK/Node.js dependencies, custom theming via Rust traits.

### Renderer Selection

```
HONKHONK_RENDERER=software honkhonk   # Force CPU rendering (tiny-skia)
honkhonk                                # Default: GPU rendering (wgpu)
```

Wayland sessions require GPU drivers (compositor needs them), so wgpu works on all target systems. Software fallback exists for VMs, debugging, and edge cases. No auto-fallback — explicit user choice.

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                   Iced Application                     │
│  ┌────────────────────────────────────────────────┐  │
│  │                 View Layer                      │  │
│  │                                                 │  │
│  │  ┌──────────────┐  ┌───────────────────────┐   │  │
│  │  │  Sound Grid  │  │  Controls             │   │  │
│  │  │  - cards     │  │  - search bar         │   │  │
│  │  │  - click play│  │  - volume slider      │   │  │
│  │  │  - stop btn  │  │  - stop all           │   │  │
│  │  └──────────────┘  └───────────────────────┘   │  │
│  │                                                 │  │
│  └─────────────────────┬──────────────────────────┘  │
│                        │ Messages (Elm architecture)  │
│  ┌─────────────────────┴──────────────────────────┐  │
│  │            State + Update Logic                  │  │
│  │                                                 │  │
│  │  ┌──────────────┐  ┌───────────────────────┐   │  │
│  │  │ AudioHandle  │  │  Library / Config     │   │  │
│  │  │ (channel tx) │  │  (sound entries,      │   │  │
│  │  │              │  │   XDG paths)          │   │  │
│  │  └──────┬───────┘  └───────────────────────┘   │  │
│  │         │                                       │  │
│  └─────────┼───────────────────────────────────────┘  │
└────────────┼──────────────────────────────────────────┘
             │ Channels (no IPC serialization)
             ▼
┌─────────────────────────────┐    ┌────────────────────┐
│  AudioEngine (PipeWire      │    │  tray-icon         │
│  thread)                    │    │  (main thread,     │
│  - virtual sink             │    │   channel → sub)   │
│  - mic passthrough          │    └────────────────────┘
│  - playback streams         │
│  - monitor output           │
└──────────────┬──────────────┘
               │
               ▼
         PipeWire Server
         (virtual sink +
          audio graph)
```

### Communication Model

No IPC serialization. Direct Rust channel communication:

1. **UI → Audio (Commands):** Iced `Command::perform` sends `AudioCommand` via `tokio::sync::mpsc` to PipeWire thread.
2. **Audio → UI (Events):** PipeWire thread sends `AudioEvent` via channel. Iced `Subscription` polls receiver each frame.
3. **Tray → App:** `tray-icon` sends events via channel → Iced `Subscription`.

### Threading Model

- **Main thread:** Iced event loop + tray-icon (both need main thread on Linux)
- **PipeWire thread:** Dedicated thread running PipeWire's own event loop. Owns `AudioEngine`.
- **Communication:** Bounded `mpsc` channels. Non-blocking sends from main thread.

## PipeWire Audio Architecture

### Virtual Mic Creation

HonkHonk creates a single persistent virtual audio device on startup:

```
┌─────────────────────┐
│  Physical Mic       │────────────────────┐
│  (e.g. G733)        │                    │
└─────────────────────┘                    │
                                           ▼
┌─────────────────────┐          ┌──────────────────┐
│  HonkHonk Playback  │─────────▶│  HonkHonk Mix    │──▶ "HonkHonk Mic"
│  (sound effects)    │          │  (virtual sink)   │    (Audio/Source)
└─────────────────────┘          └──────────────────┘         │
                                                              ▼
                                                         Discord / App
                                                    (selects as mic input)
```

Key design decisions:
- **One persistent virtual sink** created at startup, destroyed at shutdown. No per-sound node creation/destruction (this is what caused PWSP's audio cutouts).
- **Real mic passthrough** mixed into the virtual sink. User's voice + sound effects come through one device.
- **Playback streams** write directly to the virtual sink. PipeWire handles mixing natively.
- **Local monitoring** via a separate playback stream to the default audio output (headset), so the user hears the sound too.

### Why this avoids PWSP's problems

PWSP created/destroyed a PipeWire stream node for every sound played. Each creation triggered PipeWire graph reconfiguration → driver renegotiation → audio dropouts. HonkHonk keeps a persistent sink and writes audio data into it, similar to how a music player works. No graph changes during playback.

## Global Shortcuts — Fixed Slot Model (Phase 2)

The xdg-desktop-portal GlobalShortcuts API requires a user confirmation dialog when shortcuts are registered. To avoid spamming dialogs every time a sound is added:

**Fixed slot approach:**
1. On first run, register 20 shortcut slots: `honkhonk-slot-1` through `honkhonk-slot-20`
2. KDE shows ONE confirmation dialog for all 20
3. User assigns key combos via KDE System Settings (native UX)
4. In HonkHonk, user assigns sounds to slots: "Slot 1 = Vine Boom, Slot 2 = Bruh, ..."
5. Adding/removing sounds from slots requires NO new portal registration

This mirrors VoiceMod's approach — fixed button grid, user maps sounds to buttons.

## Phased Delivery

### Phase 1: MVP — "It plays sounds in Discord"
- Iced GUI skeleton (window, sound grid, search, volume)
- Sound file browser (folder-based, search, grid view)
- PipeWire virtual mic (persistent sink + mic passthrough)
- Play sound → virtual mic + local headset
- Stop / volume controls
- System tray with quit
- Flatpak packaging

**No hotkeys in Phase 1.** Click-to-play only. Ship it, get feedback.

### Phase 2: Global Shortcuts
- ashpd GlobalShortcuts integration
- 20 fixed slots, user assigns sounds to slots
- Settings panel for slot management
- KDE System Settings integration for key binding

### Phase 3: Polish
- Favorites / recently played
- Sound pack import (drag-and-drop folders, MyInstants URL import)
- Themes (dark/light, accent colors via Iced custom Theme)
- Per-sound volume
- Overlap mode (concurrent vs. interrupt)

### Phase 4: Advanced
- App audio passthrough (route Spotify/YouTube to mic)
- Per-app audio routing (like Soundux's passthrough feature)
- Sound effects (reverb, pitch shift — stretch goal)
- Cross-desktop support (GNOME, Hyprland — portal-based, should work)

## File Structure

```
honkhonk/
├── src/
│   ├── main.rs              # Entry point, renderer selection, app launch
│   ├── app.rs               # Iced Application impl (state, update, view)
│   ├── ui/
│   │   ├── mod.rs           # Re-exports
│   │   ├── sound_grid.rs    # Grid of sound cards
│   │   ├── sound_card.rs    # Individual sound button/card
│   │   ├── search_bar.rs    # Search input
│   │   ├── volume.rs        # Volume slider
│   │   └── theme.rs         # Custom theme (colors, spacing)
│   ├── audio/
│   │   ├── mod.rs           # Re-exports
│   │   ├── error.rs         # AudioError enum (thiserror)
│   │   ├── engine.rs        # PipeWire lifecycle (virtual sink, mic passthrough)
│   │   ├── decoder.rs       # symphonia → PCM samples
│   │   ├── mixer.rs         # Mix mic + playback into virtual sink
│   │   └── playback.rs      # Play sound to sink + monitor output
│   ├── tray/
│   │   ├── mod.rs
│   │   └── icon.rs          # tray-icon setup, menu, quit handler
│   ├── shortcuts/           # Phase 2
│   │   ├── mod.rs
│   │   ├── error.rs         # PortalError enum
│   │   └── portal.rs        # ashpd GlobalShortcuts session
│   └── state/
│       ├── mod.rs
│       ├── error.rs         # ConfigError enum
│       ├── config.rs        # App settings (serde JSON)
│       ├── library.rs       # Sound file index + metadata
│       └── slots.rs         # Hotkey slot ↔ sound mapping (Phase 2)
├── assets/
│   └── icons/               # App icon, tray icon (SVG + PNG sizes)
├── tests/
│   └── fixtures/            # Short audio files for decode tests
├── packaging/
│   ├── flatpak/
│   │   └── io.github.thewrz.HonkHonk.yml
│   ├── aur/
│   │   └── PKGBUILD
│   ├── debian/
│   │   ├── control
│   │   ├── rules
│   │   ├── changelog
│   │   └── copyright
│   ├── rpm/
│   │   └── honkhonk.spec
│   ├── nix/
│   │   └── flake.nix
│   └── appimage/
│       └── HonkHonk.desktop
├── docs/
│   └── adr/                     # Architecture Decision Records
│       ├── 001-iced-over-tauri-svelte.md
│       ├── 002-pipewire-only-no-pulseaudio.md
│       ├── 003-fixed-slot-hotkey-model.md
│       ├── 004-persistent-sink-no-per-sound-nodes.md
│       └── 005-tray-icon-over-ksni.md
├── .github/
│   └── workflows/
│       ├── ci.yml               # Lint, test, build on PR
│       └── release.yml          # Build all package formats on tag
├── Cargo.toml
├── clippy.toml              # Strict complexity thresholds
├── deny.toml                # cargo-deny config (license + advisory audit)
├── ARCHITECTURE.md          # This file
├── CLAUDE.md                # Dev instructions
├── LICENSE                  # MIT
└── README.md
```

## Key Dependencies (Cargo.toml)

```toml
[dependencies]
iced = { version = "0.13", features = ["tokio", "tiny-skia"] }
pipewire = "0.8"           # pipewire-rs — official PipeWire Rust bindings
ashpd = { version = "0.13", features = ["global_shortcuts", "tokio"] }  # Phase 2
symphonia = { version = "0.5", features = ["mp3", "ogg", "flac", "wav", "pcm", "aac"] }
tray-icon = "0.19"         # System tray (StatusNotifierItem)
muda = "0.15"              # Menu for tray-icon
thiserror = "2"            # Typed error enums at module boundaries
anyhow = "1"               # Error context chains in glue/app layer
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
directories = "6"          # XDG path resolution

[dev-dependencies]
tempfile = "3"             # Test fixtures
```

## Lessons from Prior Art

Building on what the community has learned from existing Linux soundboard projects:

| Challenge | HonkHonk Approach |
|-----------|-------------------|
| Large rewrites stalling | Phased delivery. MVP ships without hotkeys. Iterate. |
| Solo maintainer bottleneck | Public from day one. Accept contributions. |
| Building custom libs before the app | Use existing crates (pipewire-rs, ashpd, symphonia). Don't reinvent. |
| X11-first with Wayland bolted on | Wayland-native from line 1. Portal APIs only. |
| Complex web toolchains for desktop apps | Pure Rust. Single `cargo build`. No Node/npm/WebKitGTK. |
| Dual PulseAudio + PipeWire backends | PipeWire only. PulseAudio compat layer handles legacy apps. |
| Two-language complexity | Single language (Rust) for GUI + backend. No IPC serialization. |

## Target Platforms

### Desktop Environments (Tier 1 — fully supported)

| DE | Portal Implementation | Shortcuts | Tray | Notes |
|----|----------------------|-----------|------|-------|
| **KDE Plasma 6** | xdg-desktop-portal-kde | GlobalShortcuts v2 via KGlobalAccelD in KWin | StatusNotifierItem (native) | Primary dev target |
| **GNOME 45+** | xdg-desktop-portal-gnome | GlobalShortcuts v2 | StatusNotifierItem (via extension) | Second priority. Test on Fedora |
| **Hyprland** | xdg-desktop-portal-hyprland | GlobalShortcuts v2 | StatusNotifierItem (via waybar) | Popular tiling WM, portal-compliant |

### Desktop Environments (Tier 2 — should work, best-effort)

| DE | Notes |
|----|-------|
| **Sway** | xdg-desktop-portal-wlr has limited GlobalShortcuts support. May need wlr-specific portal |
| **Cosmic** | System76's DE uses iced (Rust). Portal support TBD as it matures |
| **Cinnamon / MATE / Xfce** | X11-based. Not targeted but may work under XWayland |

### Distro Targets

| Distro Family | Target Versions | PipeWire | Portal | Package Format |
|---------------|----------------|----------|--------|---------------|
| **Arch / Manjaro** | Rolling | 1.4+ | xdg-desktop-portal 1.18+ | AUR (PKGBUILD) |
| **Fedora** | 40+ | 1.0+ | xdg-desktop-portal 1.18+ | RPM (.spec) |
| **Ubuntu / Debian** | Ubuntu 24.04+, Debian 13+ | 1.0+ | xdg-desktop-portal 1.18+ | DEB (debian/) |
| **openSUSE** | Tumbleweed, Leap 16+ | 1.0+ | xdg-desktop-portal 1.18+ | RPM (.spec, shared with Fedora) |
| **NixOS** | Unstable / 24.11+ | 1.0+ | xdg-desktop-portal 1.18+ | Nix derivation (flake.nix) |

### Runtime Dependencies

These must be available on the host system (not bundled):

```
pipewire >= 1.0
xdg-desktop-portal >= 1.18
xdg-desktop-portal-{kde,gnome,hyprland,wlr} (DE-specific)
vulkan-driver or mesa (for wgpu GPU rendering)
wayland-client
```

### Build Dependencies

```
rust >= 1.75 (Cargo, rustc)
pkg-config
pipewire-devel / libpipewire-0.3-dev (headers)
wayland-devel / libwayland-dev (headers)
clang / gcc
```

### Explicitly Not Targeting

X11-only sessions, PulseAudio-only systems (no PipeWire), Windows, macOS.

## Desktop Integration Guidelines

### Portal-First Architecture

All desktop integration uses xdg-desktop-portal D-Bus APIs, never DE-specific APIs directly:

| Feature | Portal API | D-Bus Interface |
|---------|-----------|-----------------|
| Global shortcuts | GlobalShortcuts v2 | `org.freedesktop.portal.GlobalShortcuts` |
| File picker | FileChooser | `org.freedesktop.portal.FileChooser` |
| Notifications | Notification | `org.freedesktop.portal.Notification` |
| Background permission | Background | `org.freedesktop.portal.Background` |
| Autostart | Background (autostart) | `org.freedesktop.portal.Background` |

This ensures cross-DE compatibility. KDE, GNOME, and Hyprland each implement these portals differently under the hood, but the app talks to one API.

### System Tray

StatusNotifierItem (SNI) is the standard. Implementation via `tray-icon` crate:
1. Initialize on main thread before Iced event loop
2. Menu via `muda` crate: "Show/Hide", separator, "Quit"
3. Events communicated to Iced via channel → Subscription
4. No legacy XEmbed tray support

### .desktop File

```ini
[Desktop Entry]
Name=HonkHonk
Comment=Soundboard for Linux
Exec=honkhonk %u
Icon=honkhonk
Terminal=false
Type=Application
Categories=Audio;AudioVideo;
Keywords=soundboard;sound;effects;discord;voice;
StartupWMClass=honkhonk
Actions=quit;

[Desktop Action quit]
Name=Quit
Exec=honkhonk --quit
```

### XDG Directories

| Data | Path | Content |
|------|------|---------|
| Config | `$XDG_CONFIG_HOME/honkhonk/` | `config.json`, `slots.json` |
| Data | `$XDG_DATA_HOME/honkhonk/` | Sound library index, cached waveforms |
| User sounds | `$XDG_MUSIC_DIR/HonkHonk/` or user-configured | Default sound import directory |

## Packaging

### Flatpak (primary distribution)

Portal-based apps work naturally in Flatpak's sandbox.

```yaml
# Flatpak manifest key points
app-id: io.github.thewrz.HonkHonk
runtime: org.freedesktop.Platform
sdk: org.freedesktop.Sdk
finish-args:
  - --socket=wayland
  - --socket=pulseaudio      # PipeWire accessed via pulse socket
  - --device=dri             # GPU access for wgpu
  - --talk-name=org.freedesktop.portal.Desktop
  - --talk-name=org.kde.StatusNotifierWatcher
  - --filesystem=xdg-music:ro
```

### AUR (Arch / Manjaro)

```bash
# PKGBUILD key points
pkgname=honkhonk
makedepends=('rust' 'cargo' 'pkg-config' 'pipewire' 'wayland')
depends=('pipewire' 'wayland' 'vulkan-driver' 'xdg-desktop-portal')
optdepends=(
  'xdg-desktop-portal-kde: KDE Plasma support'
  'xdg-desktop-portal-gnome: GNOME support'
  'xdg-desktop-portal-hyprland: Hyprland support'
)
```

### DEB (Debian / Ubuntu)

```
# debian/control key points
Build-Depends: rustc, cargo, pkg-config,
 libpipewire-0.3-dev, libwayland-dev
Depends: pipewire, libwayland-client0, mesa-vulkan-drivers, xdg-desktop-portal
Recommends: xdg-desktop-portal-kde | xdg-desktop-portal-gnome | xdg-desktop-portal-hyprland
```

### RPM (Fedora / openSUSE)

```spec
# .spec key points
BuildRequires: rust cargo pkg-config pipewire-devel wayland-devel
Requires: pipewire wayland mesa-vulkan-drivers xdg-desktop-portal
Recommends: (xdg-desktop-portal-kde if plasma-workspace)
Recommends: (xdg-desktop-portal-gnome if gnome-shell)
```

### NixOS

```nix
# flake.nix — provide a package and NixOS module
# Module enables PipeWire + portal integration automatically
# Package uses buildRustPackage
```

### AppImage (portable fallback)

Self-contained binary with bundled libs. Least preferred — portal access from AppImage requires proper desktop integration.

### CI/CD — Build Matrix

Every tagged release builds all formats:

| Format | Build Environment | Test |
|--------|-------------------|------|
| Flatpak | Flathub builder or `flatpak-builder` in CI | `flatpak run` smoke test |
| AUR PKGBUILD | Arch container (`archlinux:latest`) | `makepkg -si` in clean chroot |
| .deb | Ubuntu 24.04 container | `dpkg -i` + `apt install -f` |
| .rpm | Fedora 40 container | `rpmbuild` + `dnf install` |
| AppImage | Ubuntu 22.04 (oldest glibc target) | Run on multiple distros |
| Nix flake | `nix build` | `nix run` smoke test |

GitHub Actions workflow runs the full matrix on every release tag.

## License

MIT — permissive, no friction for contributors or downstream use.

## Prior Art and References

| Project | What we learn from it |
|---------|----------------------|
| [PWSP](https://github.com/arabianq/pipewire-soundpad) | Rust soundboard architecture, virtual mic pattern. Avoid: per-sound node creation |
| [venmic](https://github.com/Vencord/venmic) | PipeWire PatchBay pattern, node filtering, feedback prevention |
| [Pipeweaver](https://github.com/pipeweaver/pipeweaver) | Rust daemon + web UI architecture |
| [obs-wayland-hotkeys](https://github.com/leia-uwu/obs-wayland-hotkeys) | GlobalShortcuts portal proof-of-concept on KDE6 |
| [Helvum](https://github.com/relulz/helvum) | Rust + PipeWire desktop app integration |
| [Soundux](https://github.com/Soundux/Soundux) | Feature set reference. Avoid: scope creep, private repos, library perfectionism |
| [ashpd docs](https://docs.rs/ashpd/latest/ashpd/desktop/global_shortcuts/) | GlobalShortcuts API reference |
| [pipewire-rs](https://gitlab.freedesktop.org/pipewire/pipewire-rs) | PipeWire Rust bindings |
| [Iced](https://github.com/iced-rs/iced) | GUI framework — examples, widget catalog, custom styling |
| [Cosmic DE](https://github.com/pop-os/cosmic-epoch) | Large Iced application reference (System76's desktop) |
