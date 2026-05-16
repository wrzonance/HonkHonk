# `ShortcutConfigService` — DE-Aware Shortcut Config UI Design Spec

**Date:** 2026-05-16
**Context:** Follows PR #85 fix; closes the gap where portal v1 backends (like xdg-desktop-portal 1.20.4 on this machine) can't call `configure_shortcuts()` and the app showed a useless text note.

## Problem

HonkHonk currently scatters shortcut configuration logic across `app.rs`:
- `portal_cmd_tx` — portal command channel
- `shortcuts_configure_available` — portal v2 flag
- Branching logic in `OpenShortcutConfig` handler
- `configure_available` derived in view()

Adding a DE-specific fallback (e.g., `kcmshell6 kcm_keys` for KDE) would mean more flags and more branching directly in `app.rs`. This doesn't scale as we add more DEs or platform targets.

**Research findings:**
- **XDG GlobalShortcuts portal v1** (current installed): no `ConfigureShortcuts` method exposed
- **Portal v2** `configure_shortcuts()`: not yet in xdg-desktop-portal 1.20.4 on this system
- **KDE fallback**: `kcmshell6 kcm_keys` — fire-and-forget process spawn, opens KDE "Keyboard Shortcuts" panel
- **GNOME fallback**: `gnome-control-center keyboard` — fire-and-forget, opens GNOME keyboard settings
- **Hyprland / Sway**: portal-only DEs; no native GUI shortcut config tool; if portal v1 only → text instruction
- **OBS on Wayland**: uses XDG GlobalShortcuts portal (same path we use)
- **Detection standard**: `$XDG_CURRENT_DESKTOP` env var (`"KDE"`, `"GNOME"`, `"Hyprland"`, `"sway"`)

## Solution

`src/shortcuts/config_ui.rs` — a self-contained service that:
1. Detects the desktop environment once at startup
2. Stores portal state as it arrives (sender + v2 availability)
3. Exposes a single `open()` call — decides portal vs. DE fallback internally
4. App.rs holds ONE field and delegates fully — no branching logic remains in app.rs

The main process just says "open keyboard shortcuts config." The service figures out how.

## Types

### `DesktopEnv`

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DesktopEnv {
    Kde,
    Gnome,
    Hyprland,
    Sway,
    Windows,          // stub — unimplemented, reserved for future port
    MacOs,            // stub — unimplemented, reserved for future port
    Unknown(String),  // raw $XDG_CURRENT_DESKTOP value preserved for logging
}
```

### `ShortcutConfigService`

```rust
pub(crate) struct ShortcutConfigService {
    desktop_env: DesktopEnv,
    portal_v2_available: bool,
    portal_cmd_tx: Option<tokio::sync::mpsc::Sender<PortalCommand>>,
}
```

## Detection

Called once in `ShortcutConfigService::new()`. Detection order:

1. `#[cfg(target_os = "windows")]` → `DesktopEnv::Windows`
2. `#[cfg(target_os = "macos")]` → `DesktopEnv::MacOs`
3. `std::env::var("XDG_CURRENT_DESKTOP")` — case-insensitive match:
   - `"kde"` → `Kde`
   - `"gnome"` → `Gnome`
   - `"hyprland"` → `Hyprland`
   - `"sway"` → `Sway`
   - other/missing → `Unknown(raw_value_or_empty)`

`$XDG_CURRENT_DESKTOP` can contain colon-separated values (e.g., `"GNOME:GNOME"`). Match against the first token (split on `:`).

## Public API

```rust
impl ShortcutConfigService {
    /// Detect DE at startup.
    pub(crate) fn new() -> Self;

    /// Called when portal Handle event arrives.
    pub(crate) fn set_portal_sender(&mut self, tx: tokio::sync::mpsc::Sender<PortalCommand>);

    /// Called when ConfigureAvailable event arrives (portal v2 flag).
    pub(crate) fn set_portal_v2_available(&mut self, available: bool);

    /// Whether any path to open shortcut config exists (portal OR DE tool).
    /// Used by slot_manager to decide button vs. text.
    pub(crate) fn can_open(&self) -> bool;

    /// Open the shortcut configuration UI via the best available path.
    /// Fire-and-forget — errors are logged, not propagated.
    pub(crate) fn open(&self);
}
```

## `open()` Priority Logic

```
1. If portal_v2_available AND portal_cmd_tx is Some:
   → try_send(PortalCommand::ConfigureShortcuts)
   → on send error: eprintln!("honkhonk: portal configure_shortcuts dropped: {e}")

2. Else — DE fallback:
   Kde      → spawn "kcmshell6" "kcm_keys"
   Gnome    → spawn "gnome-control-center" "keyboard"
   Hyprland → eprintln info (no GUI tool; portal is the only path)
   Sway     → eprintln info (same as Hyprland)
   Windows  → stub, no-op (future port)
   MacOs    → stub, no-op (future port)
   Unknown  → eprintln info with raw DE name
```

All process spawns are `std::process::Command::spawn()` — non-blocking, child runs independently. Spawn errors → `eprintln!`, not propagated.

## `can_open()` Logic

```rust
fn can_open(&self) -> bool {
    self.portal_v2_available
        || matches!(self.desktop_env, DesktopEnv::Kde | DesktopEnv::Gnome)
}
```

- **KDE portal-v1**: `can_open()` → true (kcmshell6 available) → button shown
- **GNOME portal-v1**: `can_open()` → true (gnome-control-center available) → button shown
- **Hyprland portal-v1**: `can_open()` → false → text shown: "Assign keys in System Settings → Shortcuts"
- **Any DE portal-v2**: `can_open()` → true → button shown

## App.rs Changes

### Remove fields:
```rust
// DELETE:
portal_cmd_tx: Option<tokio::sync::mpsc::Sender<crate::shortcuts::PortalCommand>>,
shortcuts_configure_available: bool,
```

### Add field:
```rust
shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService,
```

### Init in constructors:
```rust
shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService::new(),
```

### Message handlers — three become one-liners:
```rust
Message::ShortcutHandle(PortalCmdSender(sender)) => {
    self.shortcut_config.set_portal_sender(sender);
    Task::none()
}

Message::ShortcutsConfigureAvailable(v) => {
    self.shortcut_config.set_portal_v2_available(v);
    Task::none()
}

Message::OpenShortcutConfig => {
    self.shortcut_config.open();
    Task::none()
}
```

### View — `configure_available` sourced from service:
```rust
slot_manager::SlotManagerCtx {
    // ...
    configure_available: self.shortcut_config.can_open(),
}
```

## slot_manager.rs Changes

No structural changes. `SlotManagerCtx.configure_available: bool` stays — just populated differently (from `service.can_open()` instead of `self.shortcuts_configure_available`).

UI behavior unchanged: button when `configure_available`, text when not.

## File Changes

| File | Action |
|------|--------|
| `src/shortcuts/config_ui.rs` | **Create** — full service implementation |
| `src/shortcuts/mod.rs` | Add `pub mod config_ui;` |
| `src/app.rs` | Replace 2 fields with 1; simplify 3 handlers; update view call |

## Tests

In `config_ui.rs`:

Detection tests avoid mutating `std::env` (which is process-global and unsafe under parallel tests). Instead, extract a pure `fn parse_desktop_env(raw: &str) -> DesktopEnv` and test that:
- `parse_desktop_env("KDE")` → `Kde`
- `parse_desktop_env("GNOME:GNOME")` → `Gnome` (first token before `:`)
- `parse_desktop_env("Hyprland")` → `Hyprland`
- `parse_desktop_env("coolde")` → `Unknown("coolde")`
- `parse_desktop_env("")` → `Unknown("")`
- `can_open_true_on_kde_no_portal` — `Kde` env, portal v2 false → `can_open()` true
- `can_open_false_on_hyprland_no_portal` — `Hyprland` env, portal v2 false → `can_open()` false
- `can_open_true_on_hyprland_with_portal_v2` — any env, portal v2 true → `can_open()` true

In `app.rs`:
- Update `open_shortcut_config_sends_command_when_handle_present` — now injects via `set_portal_sender` + `set_portal_v2_available`

## Out of Scope

- GNOME deep-link to shortcut page vs. keyboard page (current: `keyboard` arg; may want `shortcuts` — leave for after testing)
- Sway-specific tool (no standard exists)
- Windows / macOS implementation (stubs only)
- Action label localization (`"Configure Shortcuts"` hardcoded for now)
