# In-App Shortcut Assignment with Conflict Feedback — Design Spec

**Issue:** #77
**Phase:** 2 (final item — completes Phase 2)
**Date:** 2026-05-15

## Summary

Let users assign global hotkeys directly inside HonkHonk (instead of configuring them in KDE/GNOME System Settings), with clear feedback when the DE cannot honor the requested key combination.

## Background

The XDG GlobalShortcuts portal has no pre-check API — there is no way to query whether a keybind is already claimed before attempting to bind it. The portal resolves conflicts server-side (KGlobalAccel on KDE, gnome-shell on GNOME) and communicates the outcome silently: `BindShortcuts` returns only the subset of shortcuts that were actually bound. If a slot's requested trigger was rejected, it is simply absent from the response.

The current codebase already establishes a portal session with 20 fixed slots and reads back `trigger_description` strings into `slot_triggers`. This spec adds:
1. Click-to-capture keyboard combo entry
2. Portal re-bind with `preferred_trigger` hint
3. Save/not-save feedback on the result
4. Live sync when DE changes bindings externally via `ShortcutsChanged`

## Architecture

### Bidirectional Portal Stream

The existing `shortcut_stream` is one-way (events out only). It becomes bidirectional using Iced's self-reporting sender pattern:

1. Stream creates `(cmd_tx, cmd_rx)` internally at startup
2. After session + initial bind: emits `ShortcutEvent::Handle(cmd_tx)` to the app
3. App stores `cmd_tx` as `portal_cmd_tx: Option<mpsc::Sender<PortalCommand>>`
4. Main loop uses `tokio::select!` across three sources:
   - `activated` stream (hotkey presses)
   - `shortcuts_changed` stream (DE-initiated changes)
   - `cmd_rx` receiver (app-initiated rebind requests)

When a `RebindSlot` command arrives, the stream rebuilds all 20 `NewShortcut` entries with the updated `preferred_trigger` for that slot, calls `bind_shortcuts` on the existing session (no new session, no repeat dialog), then emits `ShortcutEvent::RebindResult`.

### New Types

**`shortcuts/mod.rs` additions:**

```rust
pub enum ShortcutEvent {
    Ready,
    Handle(tokio::sync::mpsc::Sender<PortalCommand>),  // NEW
    Activated(u8),
    Bindings(Vec<(u8, String)>),
    RebindResult(Vec<(u8, String)>),                   // NEW
    Changed(Vec<(u8, String)>),                        // NEW — ShortcutsChanged signal
    Failed(String),
}

pub enum PortalCommand {
    RebindSlot { idx: u8, trigger: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindFeedback {
    Unset,
    Saved,
    NotSaved,
}
```

### State Additions (`app.rs`)

```rust
// New fields in AppState:
portal_cmd_tx: Option<tokio::sync::mpsc::Sender<PortalCommand>>,
capturing_slot: Option<u8>,
held_modifiers: keyboard::Modifiers,
desired_triggers: [Option<String>; 20],   // user intent — persisted in config
bind_feedback: [BindFeedback; 20],        // Unset | Saved | NotSaved per slot
```

### Config Additions (`state/config.rs`)

```rust
// New field on AppConfig:
#[serde(default)]
desired_triggers: [Option<String>; 20],
```

Persisting `desired_triggers` means `preferred_trigger` hints are passed in the initial `BindShortcuts` on every app launch — the portal grants previously approved combos without user interaction.

## Keyboard Capture

**New file:** `src/shortcuts/capture.rs`

Contains:
- `keyboard_capture_sub()` — Iced subscription, active only while `capturing_slot.is_some()`
- `format_combo(modifiers, key) -> Option<String>` — formats the captured combo

**Combo rules:**
- Bare letters/numbers/symbols without a modifier → rejected; show "Add a modifier key (Ctrl, Alt, Meta)" hint inline
- Function keys (F1–F20) are allowed without a modifier (unambiguous)
- Modifier-only press → updates `held_modifiers`, does not snap
- Escape → `Message::CancelCapture`
- Valid combo → `Message::KeyCaptured(String)`

**Combo format:** `Ctrl+Alt+F1`, `Meta+1`, `Shift+F2`, `Meta+Shift+A`
Order: Ctrl → Alt → Shift → Meta → Key. Key names: Named keys use display strings ("F1", "Return", "Space"), character keys uppercased.

**New Messages:**
```rust
Message::StartCapture(u8),          // user clicks "Set Hotkey"
Message::CancelCapture,             // Esc or slot deselected
Message::KeyCaptured(String),       // valid combo captured
Message::PortalHandle(mpsc::Sender<PortalCommand>),
Message::RebindResult(Vec<(u8, String)>),
Message::ShortcutsChangedExternal(Vec<(u8, String)>),
```

## Portal Re-bind Flow

On `Message::KeyCaptured(combo)` when `capturing_slot == Some(idx)`:
1. Update `desired_triggers[idx] = Some(combo.clone())`
2. Save config
3. Send `PortalCommand::RebindSlot { idx, trigger: combo }` via `portal_cmd_tx`
4. Exit capture mode (`capturing_slot = None`)

On `ShortcutEvent::RebindResult(bound_slots)`:
- For each slot with a `desired_triggers[idx]`: if idx present in `bound_slots` → `Saved`; absent → `NotSaved`

On `ShortcutEvent::Changed(bindings)`:
- Update `slot_triggers` live (same logic as `Bindings` today)
- Re-evaluate `bind_feedback` for affected slots

## UI Changes

### Slot Sidebar — Bound Sound Selected

Current sidebar sections: slot label, sound header, "GLOBAL HOTKEY" display, "PORTAL STATUS" dot, Unbind button.

New layout:
```
SLOT #01
[sound header]

GLOBAL HOTKEY
┌──────────────────────────────┐   ← existing trigger display (unchanged)
│  Meta+1                      │
└──────────────────────────────┘
[Set Hotkey]                       ← NEW button

✓ Meta+1 · Saved                   ← feedback badge (green, shown after bind)
  OR
⚠ Meta+1 · Not saved —             ← feedback badge (amber, shown after bind)
  may be in use by another app

● Registered via xdg-desktop-portal ← existing portal status dot (unchanged)
[Unbind]                            ← existing unbind button (unchanged)
```

Feedback badge is absent when `BindFeedback::Unset`.

### Slot Sidebar — Capture Mode Active

When `capturing_slot == Some(idx)` and this slot is selected, replace normal sidebar content:

```
SLOT #01
[sound header]

CAPTURING HOTKEY
┌──────────────────────────────┐
│  Press a key combo…          │
│  e.g. Meta+1, Ctrl+Alt+F     │
└──────────────────────────────┘

  "Add a modifier key (Ctrl, Alt, Meta)"   ← shown only if bare key pressed

[Cancel]
```

Selecting a different slot automatically cancels capture.

### Empty Slot Sidebar

No change — assign a sound first.

## New File Layout

| File | Change |
|------|--------|
| `src/shortcuts/capture.rs` | NEW — keyboard capture subscription + combo formatting |
| `src/shortcuts/mod.rs` | Add `PortalCommand`, `BindFeedback` enums; extend `ShortcutEvent` |
| `src/shortcuts/portal.rs` | Bidirectional channel + `tokio::select!` loop + `ShortcutsChanged` handler |
| `src/state/config.rs` | Add `desired_triggers` field |
| `src/ui/slot_manager.rs` | Add "Set Hotkey" button + capture mode UI + feedback badge |
| `src/app.rs` | New Messages, state fields, update handlers, keyboard capture subscription |

## LOC Estimate

| File | ±LOC |
|------|------|
| `capture.rs` (new) | +90 |
| `shortcuts/mod.rs` | +25 |
| `shortcuts/portal.rs` | +60 |
| `state/config.rs` | +10 |
| `ui/slot_manager.rs` | +90 |
| `app.rs` | +110 |
| **Total** | **~385** |

Within the 500 LOC per PR limit.

## Out of Scope

- Querying system-wide shortcut registry (not exposed by portal)
- Per-DE conflict APIs (KGlobalAccel D-Bus) — breaks multi-DE support
- Bare letter/number bindings without modifier
- Shortcut assignment on empty slots (assign sound first)

## Test Plan

Unit tests:
- `format_combo` — all modifier combos, F-keys without modifier, bare key rejection
- `parse_rebind_result` — subset matching, absent slot → NotSaved
- Config serialization — `desired_triggers` round-trips correctly

Integration / manual:
- [ ] Click "Set Hotkey" → capture overlay appears
- [ ] Press Meta+1 → combo appears, overlay exits, DE grants it → "✓ Saved"
- [ ] Press Meta+1 again on a different slot → first slot loses binding → "⚠ Not saved" on second
- [ ] Change binding in KDE System Settings → slot manager updates live (ShortcutsChanged)
- [ ] Escape exits capture, no binding change
- [ ] Restart app → desired_triggers loaded from config, passed as preferred_trigger hints
- [ ] Bare 'a' key → shows "Add a modifier key" hint, no snap
- [ ] F1 without modifier → snaps immediately (F-key exception)

## README Update (in this PR)

Update status table:
- Monitor output device selection (#72): `🔜 Planned` → `✅ Shipped`
- Renderer selection (#73): add as `✅ Shipped`
- In-app shortcut assignment (#77): `🔜 Next` → `✅ Shipped` (when PR merges)
- Update ARCHITECTURE.md Phase 2 checklist to mark #72 and #73 done
