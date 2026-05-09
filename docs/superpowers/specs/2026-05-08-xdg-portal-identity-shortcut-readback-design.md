# XDG Portal Identity & Shortcut Readback Design

**Date:** 2026-05-08
**Status:** Approved

## Problem

Three issues with the current XDG GlobalShortcuts portal integration:

1. **Wrong app identity** — HonkHonk appears as "Konsole" in KDE's shortcuts settings UI when run via `cargo run`. Root cause: no `.desktop` file installed in dev, so portal resolves identity via D-Bus PID → parent terminal.
2. **No shortcut readback** — after the user assigns a key (e.g., "Meta+1") in KDE settings, HonkHonk discards the bound trigger info returned by `bind_shortcuts`. UI always shows "no hotkey" / "—".
3. **Misleading slot labels** — shortcuts register as `"Slot 1"…"Slot 20"` in the portal, with no app attribution.

## Scope

In scope:
- Fix app identity via `WindowIdentifier` passed to `bind_shortcuts`
- Parse `ShortcutsInformation` from `bind_shortcuts` response; surface bound keys in UI
- Rename portal shortcut descriptions from `"Slot N"` → `"HonkHonk Slot N"`
- Display trigger strings in slot manager grid tiles and sidebar "Global Hotkey" field

Out of scope:
- Persisting trigger strings across restarts (portal re-registers on each launch; triggers re-read from response each time)
- Polling for user-made shortcut changes at runtime after initial bind
- Changes to `sound_grid.rs` slot badge

## Architecture

### Files changed

| File | Change |
|------|--------|
| `src/shortcuts/mod.rs` | Add `ShortcutEvent::Bindings(Vec<(u8, String)>)` variant |
| `src/shortcuts/portal.rs` | Accept `WindowIdentifier`; fix label; parse + emit `Bindings` |
| `src/app.rs` | Store `slot_triggers`; gate subscription on window handle; handle `ShortcutBindingsUpdated` message; add `Message` variant |
| `src/ui/slot_manager.rs` | Thread `slot_triggers` through to `bound_tile` + `sidebar_bound_hotkey` |
| `Cargo.toml` | Enable `ashpd` feature `raw-window-handle` |

### New state

```rust
// in HonkHonk struct
slot_triggers: Box<[Option<String>; 20]>,  // "Meta+1" etc.; None = no binding assigned
```

Initialized as `Box::new(std::array::from_fn(|_| None))`.

### New message

```rust
Message::ShortcutBindingsUpdated(Vec<(u8, String)>)  // (0-indexed slot, trigger description)
```

### New event variant

```rust
// in shortcuts/mod.rs ShortcutEvent
ShortcutEvent::Bindings(Vec<(u8, String)>)
```

## Data Flow

### Startup sequence

```
app::new()
  → slot_triggers = all None
  → window_id = None

window::Event::Opened (already handled for window_size)
  → attempt WindowIdentifier from iced raw window handle
      (enable ashpd feature "raw-window-handle"; use raw-window-handle 0.6 WaylandWindowHandle)
      (if iced 0.14 does not expose surface: store None, degrade gracefully)
  → store Option<WindowIdentifier> in HonkHonk state

subscriptions()
  → shortcut subscription keyed on window_id presence
  → once window_id set: Subscription::run_with(window_id_hash, shortcut_stream_sub)
  → passes window_id into shortcut_stream(window_id: Option<WindowIdentifier>)

shortcut_stream(window_id):
  → GlobalShortcuts::new()
  → create_session()
  → bind_shortcuts(session, ["HonkHonk Slot 1"…"HonkHonk Slot 20"], window_id.as_ref(), options)
  → parse response.response()?.shortcuts():
      for each Shortcut: parse_slot_index(shortcut_id()) → (u8, trigger_description().to_owned())
  → emit ShortcutEvent::Bindings(Vec<(u8, String)>)
  → emit ShortcutEvent::Ready
  → loop: receive_activated → emit ShortcutEvent::Activated(idx)

app update (ShortcutEvent::Bindings via shortcuts_stream_sub):
  → Message::ShortcutBindingsUpdated(bindings)
  → for (idx, trigger) in bindings: slot_triggers[idx] = Some(trigger)
```

### Runtime (no change)

`ShortcutEvent::Activated(idx)` → `Message::ShortcutActivated(idx)` → play sound. Unchanged.

## Portal Label Fix

`portal.rs` line 40 — change:

```rust
// before
NewShortcut::new(format!("slot-{n}"), format!("Slot {n}"))

// after
NewShortcut::new(format!("slot-{n}"), format!("HonkHonk Slot {n}"))
```

## WindowIdentifier Approach

Enable ashpd's `raw-window-handle` feature in `Cargo.toml`:

```toml
ashpd = { version = "0.13", features = ["global_shortcuts", "raw-window-handle"] }
```

At implementation time, verify whether iced 0.14 exposes a `HasWindowHandle + HasDisplayHandle` type from within the window event handler or a task. If available:

```rust
WindowIdentifier::try_from(&handle).ok()
```

If iced 0.14 cannot expose the surface (no xdg-foreign export), store `None` and proceed. The portal still registers shortcuts correctly; only the app identity label in KDE settings is affected. This is acceptable for dev workflow — packaged installs resolve identity via the installed `.desktop` file regardless.

## UI Rendering

### `slot_manager.rs` — function signature changes

```rust
// view_slot_manager gains slot_triggers param
pub fn view_slot_manager<'a>(
    slots: &'a SlotMap,
    slot_triggers: &'a [Option<String>; 20],
    selected_slot: Option<u8>,
    sounds: &'a [SoundEntry],
    shortcuts_active: bool,
    t: Theme,
) -> Element<'a, Message>

// bound_tile gains trigger param
fn bound_tile<'a>(
    idx: u8,
    sound: &'a SoundEntry,
    trigger: Option<&'a str>,
    selected: bool,
    t: Theme,
) -> Element<'a, Message>

// sidebar_bound_hotkey gains trigger param
fn sidebar_bound_hotkey<'a>(trigger: Option<&'a str>, t: Theme) -> Element<'a, Message>
```

### `bound_tile` render change (line 177)

```rust
// before
text("no hotkey").size(10).color(t.ink_faint()),

// after
text(trigger.unwrap_or("no hotkey")).size(10).color(t.ink_faint()),
```

### `sidebar_bound_hotkey` render change (line 254)

```rust
// before
container(text("—").size(13).color(t.ink()))

// after
container(text(trigger.unwrap_or("—")).size(13).color(t.ink()))
```

### `app.rs` call site

```rust
view_slot_manager(&self.slots, &self.slot_triggers, ...)
```

## Error Handling

| Failure | Behavior |
|---------|----------|
| `bind_shortcuts` response parse fails | Emit `Bindings(vec![])` — UI shows "—" / "no hotkey". `Failed` not emitted; portal continues. |
| `WindowIdentifier` construction fails | Store `None`. Portal still registers shortcuts. App may show as "Konsole" in dev. Non-fatal. |
| `slot_triggers[idx]` is `None` | `unwrap_or` in render — shows `"—"` or `"no hotkey"`. No crash. |

## Testing

### `portal.rs`

```rust
#[test]
fn bindings_parse_trigger_descriptions() {
    // given mock ShortcutsInformation with slot-1 → "Meta+1", slot-3 → "Ctrl+3"
    // verify emitted Bindings contains [(0, "Meta+1"), (2, "Ctrl+3")]
    // slot-0 and slot-21 entries ignored
}
```

### `app.rs` update function

```rust
#[test]
fn shortcut_bindings_updated_stores_triggers() {
    let mut app = HonkHonk::new_for_test();
    app.update(Message::ShortcutBindingsUpdated(vec![(0, "Meta+1".into())]));
    assert_eq!(app.slot_triggers[0], Some("Meta+1".to_owned()));
    assert!(app.slot_triggers[1].is_none());
}
```

Existing `parse_slot_index` tests unchanged.
No portal integration tests — external service, excluded by convention.

## Implementation Notes

- `slot_triggers` is `Box<[Option<String>; 20]>` (not `Vec`) to match the fixed-size pattern of `SlotMap`.
- `ShortcutEvent::Bindings` fires once after `bind_shortcuts` on each launch. No live-update subscription for user key changes in KDE settings — user must relaunch to pick up reassigned keys.
- `parse_slot_index` is already tested and unchanged. The bindings parser reuses it.
