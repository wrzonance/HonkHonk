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
| System Tray | tray-icon (StatusNotifierItem) |
| Audio Decode | symphonia |

## Status

**Phase 1 complete.** The core soundboard loop is working:

| Feature | Status |
|---------|--------|
| Iced GUI — sound grid, search, categories, volume | ✅ Shipped |
| PipeWire virtual mic (persistent sink + mic passthrough) | ✅ Shipped |
| Play sound → virtual mic + local headset | ✅ Shipped |
| Stop / volume / now-playing bar | ✅ Shipped |
| System tray with quit | ✅ Shipped |
| Settings panel (Audio, Library, Hotkeys, Appearance, About) | ✅ Shipped |
| Theme persistence (Light / Dark / System) | ✅ Shipped |
| Grid density (Compact / Regular / Comfy) | ✅ Shipped |
| Mic passthrough toggle + level slider | ✅ Shipped |
| GPU renderer (wgpu default) / software fallback (tiny-skia) | ✅ Shipped |
| XDG global shortcuts (20 fixed slots) | ✅ Shipped |
| Monitor output device selection | ✅ Shipped |
| In-app shortcut assignment with conflict feedback | ✅ Shipped |
| Persistent shortcut assignments across restarts | ✅ Shipped |
| System-persistent virtual mic (survives app restart/reboot) | 🔜 Planned (#49) |

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design and roadmap.

## Installing

### Arch Linux (AUR)

```bash
yay -S honkhonk-bin    # or: paru -S honkhonk-bin
```

Pre-built binary from GitHub Releases. Source build (`honkhonk`) and VCS (`honkhonk-git`) variants are planned. See [`packaging/aur/README.md`](packaging/aur/README.md) for maintainer notes.

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

## License

[MIT](LICENSE)
