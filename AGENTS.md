# HonkHonk

Wayland-native Linux soundboard. Pure Rust — Iced 0.13 GUI (Elm/MVU) + PipeWire audio.

> Shared agent memory: `AGENTS.md` is canonical; `CLAUDE.md` symlinks here for Claude
> compatibility. Generic workflow, security, and agent rules belong in global agent configuration
> (for Claude, `~/.claude/rules`). This file holds only HonkHonk-specific facts and overrides.
> Full design spec: `ARCHITECTURE.md`. Decisions: `docs/adr/`.

## Project Overrides

- **File size: 400 lines max** (stricter than the global default) — split before adding.
  Functions <=50 lines.
- **Coverage: 80% target** via `cargo tarpaulin` every CI run (diagnostic globally; a target here).
- **Known violation:** `src/app.rs` is 2,491 lines. Do NOT add to it — split first.

## Architecture

- Iced 0.13 GUI (Elm/MVU); no other frontend framework — UI is `Element<Message>` functions.
- PipeWire via pipewire-rs: one persistent virtual sink, no per-sound node churn.
- Global shortcuts via ashpd (xdg-desktop-portal GlobalShortcuts v2) — Phase 2.
- Tray via tray-icon (StatusNotifierItem; cross-DE). Audio decode via symphonia (pure Rust).
- Renderer: wgpu default, tiny-skia software fallback (`HONKHONK_RENDERER=software`).
- **Wayland-native only — no X11. PipeWire only — no direct PulseAudio.**

`src/` layout and module responsibilities live in `ARCHITECTURE.md`. `app.rs` is the Iced
Application (state/update/view) — no business logic; it delegates to module APIs.

## Build

```bash
cargo build --release                 # production binary
cargo run                             # development
cargo test                            # unit tests
cargo test --features pipewire-test   # integration (needs a running PipeWire)
```

Build deps — **Arch:** `rust pkg-config pipewire wayland base-devel` · **Fedora:** `rust cargo
pkg-config pipewire-devel wayland-devel gcc` · **Debian/Ubuntu:** `rustc cargo pkg-config
libpipewire-0.3-dev libwayland-dev build-essential`.

## Rust Conventions

**Error context chains.** Every error carries the full "why" from origin to surface.
- `thiserror` — typed error enum per module boundary (`AudioError`, `PortalError`, `ConfigError`).
- `anyhow` — `.context("what was happening")` at every `?` in `app.rs` / top-level glue.
- No `String` errors across module boundaries. No `.unwrap()` / `panic!()` in non-test code.
- App-level catches errors and surfaces them as `Message::AudioEvent(AudioEvent::Error(..))`.

**Complexity lints (`clippy.toml`):** cognitive-complexity 10 · too-many-arguments 5 ·
too-many-lines 50 · type-complexity 200. `cargo clippy -- -D warnings` must pass clean.

**Cargo tooling:** `cargo deny check` (licenses/advisories/dupes) + `cargo tarpaulin` every CI run;
`cargo machete` / `cargo udeps` / `cargo bloat` pre-release. A new crate needs a PR comment
justifying why stdlib/existing deps cannot do it; a new system `-dev` dep must update
`.github/workflows/*.yml` in the same PR so CI does not break.

## Multi-DE Rules

All desktop integration goes through xdg-desktop-portal D-Bus APIs — never KDE/GNOME/compositor-
specific APIs. One binary must work on KDE, GNOME, Hyprland, Sway:
- Shortcuts `org.freedesktop.portal.GlobalShortcuts` (ashpd, Phase 2) · File dialogs
  `org.freedesktop.portal.FileChooser` · Notifications `org.freedesktop.portal.Notification` ·
  Autostart `org.freedesktop.portal.Background`.

Tray uses StatusNotifierItem (SNI) via tray-icon — no XEmbed. The `libayatana-appindicator`
deprecation warning on stderr is upstream and harmless — ignore it.

## Packaging / XDG

- Runtime deps are NOT bundled (except AppImage/Flatpak) — packages declare deps. `packaging/` holds
  Flatpak/AUR/.deb/.rpm/AppImage/Nix configs, all building from the same source tree. Smoke-test each
  format in a clean container before release. A tagged `v*` triggers the packaging matrix.
- Config `$XDG_CONFIG_HOME/honkhonk/`, data `$XDG_DATA_HOME/honkhonk/`. Never write to `~/` or
  `~/.honkhonk`.
- Desktop files install to `$XDG_DATA_HOME/applications/`; icons install to
  `$XDG_DATA_HOME/icons/hicolor/`.

## Testing

`cargo test` covers audio engine, config, library, and app-state; `--features pipewire-test` runs the
play-sound-to-virtual-mic integration path. Do not test Iced view rendering or third-party internals.
TDD is mandatory (global rule) — failing test first, pin every bugfix with a regression test.
