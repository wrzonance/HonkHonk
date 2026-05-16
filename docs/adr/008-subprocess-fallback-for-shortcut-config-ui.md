# ADR-008: Subprocess Fallback for Shortcut Config UI on Portal v1

## Status: Accepted

## Context

CLAUDE.md states: "All desktop integration MUST go through xdg-desktop-portal D-Bus APIs. Never use KDE-specific, GNOME-specific, or compositor-specific APIs directly."

The XDG GlobalShortcuts portal v2 added `ConfigureShortcuts` — a method that opens the DE's native shortcut configuration dialog for an active session. When available, it is the correct cross-DE path and is what `ShortcutConfigService` uses first.

However, xdg-desktop-portal 1.20.4 (as installed on KDE Plasma 6 / Arch at time of writing) exposes GlobalShortcuts interface **version 1**, which does not include `ConfigureShortcuts`. The portal frontend does not forward the call even though the KDE backend binary has the implementation. This is not a bug — it reflects the interface version the installed daemon supports.

When portal v2 is unavailable:
- The "Configure Shortcuts" button in the slot manager would show non-functional text only
- KDE and GNOME users would have no path to configure their hotkeys from within the app

## Decision

`ShortcutConfigService::open()` falls back to DE-specific subprocesses **only when portal v2 is confirmed unavailable** (`portal_v2_available = false`):

- **KDE**: `kcmshell6 kcm_keys` — opens the KDE Keyboard Shortcuts KCM
- **GNOME**: `gnome-control-center keyboard` — opens GNOME keyboard settings
- **Hyprland, Sway, Unknown**: log-only; portal is the only supported path on these DEs

This is a **targeted exception** to the portal-first rule, not a general pattern. The exception is bounded:

1. Portal v2 is always preferred and tried first.
2. Subprocess fallback only fires when the portal explicitly cannot provide the action.
3. The subprocess targets are widely available standard tools on their respective DEs, not obscure KDE/GNOME internal APIs.
4. The fallback is isolated to a single function in a dedicated module (`config_ui.rs`), not spread through the codebase.

As xdg-desktop-portal gains wider v2 adoption, the subprocess branches will become dead code on most user systems without any code change.

## Consequences

- KDE and GNOME users on portal v1 get a functional "Configure Shortcuts" button (opens the native panel).
- Hyprland/Sway users on portal v1 see informational text directing them to their compositor config.
- The fallback violates the letter of the portal-first architecture rule but not its spirit — the portal is still the primary path, and the subprocess is a best-effort bridge for a transitional period.
- If the subprocess binaries are missing (minimal install), the user gets an `eprintln!` error only. No in-app feedback. Acceptable given the rarity of the scenario.

## Related

- ADR-002 (PipeWire-only): same philosophy — pick the right modern API, don't maintain legacy fallbacks indefinitely.
- Issue #88: session token persistence — the same portal version gap that motivated this ADR also affects shortcut persistence across restarts.
