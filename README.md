# HonkHonk

A modern, Wayland-native soundboard for Linux.

Play meme sounds, sound effects, and audio clips in Discord, games, and voice chat — with a polished UI and global hotkeys that work on Wayland.

## Why HonkHonk?

Linux has lacked a soundboard that feels as polished and easy to use as VoiceMod on Windows. Existing options either don't support Wayland, have dated UIs, or require complex audio routing setup.

HonkHonk is built from the ground up for the modern Linux desktop:

- **Pure Rust** — single binary, single `cargo build`, no Node.js/WebKitGTK
- **Wayland-native** — no X11 fallbacks or XWayland hacks
- **PipeWire audio** — persistent virtual mic, zero-glitch playback
- **Global hotkeys** — via xdg-desktop-portal GlobalShortcuts (works on KDE, GNOME, Hyprland)
- **Polished UI** — Iced GUI with custom theming, GPU-rendered
- **Multi-DE support** — KDE Plasma 6, GNOME 45+, Hyprland, and more
- **Distro-friendly** — packages for Flatpak, AUR, DEB, RPM, Nix, AppImage

## Tech Stack

| Component | Technology |
|-----------|-----------|
| GUI | Iced (Rust, Elm architecture, wgpu) |
| Audio | pipewire-rs |
| Shortcuts | ashpd (xdg-desktop-portal) |
| System Tray | ksni (StatusNotifierItem over zbus) |
| Audio Decode | symphonia |

## Status

**0.1.0 — Phases 1 & 2 complete.** The core soundboard loop, global hotkeys,
and the audio-effects engine are all working:

| Feature | Status |
|---------|--------|
| Iced GUI — sound grid, search, categories, volume | ✅ Shipped |
| PipeWire virtual mic (persistent sink + mic passthrough) | ✅ Shipped |
| System-persistent virtual mic (survives app restart/reboot) | ✅ Shipped |
| Play sound → virtual mic + local headset | ✅ Shipped |
| Stop / volume / now-playing bar | ✅ Shipped |
| System tray with quit | ✅ Shipped |
| Settings panel (Audio, Library, Hotkeys, Appearance, About) | ✅ Shipped |
| Theme persistence (Light / Dark / System) | ✅ Shipped |
| Grid density (Compact / Regular / Comfy) | ✅ Shipped |
| Mic passthrough toggle + level slider | ✅ Shipped |
| Microphone input device selection | ✅ Shipped |
| Monitor output device selection | ✅ Shipped |
| GPU renderer (wgpu default) / software fallback (tiny-skia) | ✅ Shipped |
| XDG global shortcuts (20 fixed slots) | ✅ Shipped |
| In-app shortcut assignment with conflict feedback | ✅ Shipped |
| Persistent shortcut assignments across restarts | ✅ Shipped |
| Favorites tab, per-sound volume, rename (sound editor) | ✅ Shipped |
| Effects engine — reverb, chorus, flanger, pitch shift, ring mod, bandpass | ✅ Engine shipped |
| Effects panel UI + presets | 🔜 Planned (#33) |
| External app routing (PipeWire router + stream watcher) | ✅ Engine shipped |
| Audio mixer panel UI | 🔜 Planned (#28) |

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design and roadmap.

## Installing

### Arch Linux (AUR)

```bash
yay -S honkhonk        # source build (recommended) — or: paru -S honkhonk
```

`honkhonk` is the recommended package: an Arch-native build compiled from the
tagged source release, with no foreign-soname workarounds.

Alternatives:

```bash
yay -S honkhonk-bin    # pre-built binary re-extracted from the GitHub .deb (Debian base)
yay -S honkhonk-git    # bleeding-edge, tracks main
```

See [`packaging/aur/README.md`](packaging/aur/README.md) for maintainer notes and
the per-dependency justification.

## Building

```bash
# Install dependencies (Arch / Manjaro)
sudo pacman -S rust pkg-config pipewire wayland base-devel

# Build and run
cargo run

# Release build
cargo build --release
```

See [CLAUDE.md](CLAUDE.md) for build instructions for other distros.

## Prior Art

HonkHonk builds on ideas and lessons learned from the Linux audio community. We're grateful to these projects for paving the way:

- [Soundux](https://github.com/Soundux/Soundux) — pioneered the Linux soundboard space with PipeWire support and a web-based UI
- [PWSP](https://github.com/arabianq/pipewire-soundpad) — demonstrated the Rust + PipeWire soundboard architecture
- [venmic](https://github.com/Vencord/venmic) — excellent PipeWire virtual device patterns
- [Pipeweaver](https://github.com/pipeweaver/pipeweaver) — modern Rust + web UI for PipeWire routing
- [obs-wayland-hotkeys](https://github.com/leia-uwu/obs-wayland-hotkeys) — proved GlobalShortcuts portal works on KDE6
- [Cosmic DE](https://github.com/pop-os/cosmic-epoch) — large-scale Iced application reference

## Icons

HonkHonk's icons are generated from two SVG sources via a small
`make`-driven pipeline. The current art is a placeholder geometric
"HH" mark — real Krita-designed artwork lands in a follow-up PR.

See [`assets/icons/README.md`](assets/icons/README.md) for:

- The swap-real-art runbook
- `resvg` + ImageMagick install hints (Arch / Fedora / Ubuntu)
- Why the symbolic SVG must use `fill="currentColor"`

CI enforces icon freshness via `.github/workflows/icons.yml`: every
push that touches `assets/icons/` regenerates outputs and fails if
the committed PNGs/ICO/SVGs drift from the sources.

## License

[MIT](LICENSE)
