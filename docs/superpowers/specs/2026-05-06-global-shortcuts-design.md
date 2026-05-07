# Global Shortcuts Integration — Design Spec
**Issue:** #10 feat(shortcuts): ashpd GlobalShortcuts integration
**Phase:** 2
**Date:** 2026-05-06

## Decisions

| Question | Decision |
|----------|----------|
| Shortcut fires during playback | Stop current + play new |
| Slot assignment UI scope | Minimal: right-click → assign submenu + hotkey badge. Full slot manager deferred to #12 |
| Portal unavailable | Visible amber banner in-app. App continues without shortcuts. |
| Slot names in KDE System Settings | Generic: "Slot 1" … "Slot 20" (static, registered once) |
| Iced integration pattern | `Subscription::run` (stream-based, not mpsc poll) |

## Out of Scope

- Full slot manager grid (4×5 stream-deck UI) — #12
- Settings panel — #11
- Canvas tile stickers / rotation — Phase 3
- Per-slot sound preview
- Shortcut conflict detection

## Module Structure

```
src/shortcuts/
├── mod.rs        # pub use; ShortcutsStatus enum; ShortcutEvent enum
├── error.rs      # PortalError (thiserror)
└── portal.rs     # async shortcut_stream() → Stream<Item = ShortcutEvent>

src/state/
└── slots.rs      # SlotMap: [Option<PathBuf>; 20], load/save slots.json
```

Modified:
```
src/state/mod.rs     # pub use slots::SlotMap
src/app.rs           # new Message variants, state fields, subscription, update arms
src/ui/sound_card.rs # hotkey badge on tile when slot assigned + status Active
src/ui/sound_grid.rs # mouse_area right-click → context menu overlay
src/main.rs          # SlotMap::load() at startup, pass to HonkHonk::new()
Cargo.toml           # add ashpd = { version = "0.13", features = ["global_shortcuts", "tokio"] }
```

## Data Types

```rust
// src/shortcuts/mod.rs
pub enum ShortcutsStatus {
    Initializing,
    Active,
    Unavailable(String), // reason string shown in banner
}

pub enum ShortcutEvent {
    Ready,
    Activated(u8),      // slot index 0–19
    Failed(String),     // reason; stream ends after this
}

// src/shortcuts/error.rs
#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("portal connection failed: {0}")]
    Connection(#[from] ashpd::Error),
    #[error("session creation failed: {0}")]
    Session(String),
    #[error("shortcut registration failed: {0}")]
    Registration(String),
}

// src/state/slots.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotMap(pub [Option<PathBuf>; 20]);

impl SlotMap {
    pub fn load() -> Self;                             // missing/corrupt → default (all None)
    pub fn save(&self) -> Result<(), ConfigError>;    // XDG_CONFIG_HOME/honkhonk/slots.json
    pub fn get(&self, idx: u8) -> Option<&PathBuf>;
    pub fn set(&mut self, idx: u8, path: PathBuf);
    pub fn clear(&mut self, idx: u8);
    pub fn slot_for(&self, path: &Path) -> Option<u8>; // reverse lookup for badge
}

// src/app.rs — new Message variants
Message::ShortcutsReady
Message::ShortcutsUnavailable(String)
Message::ShortcutActivated(u8)           // slot index 0–19
Message::AssignSlot(u8, PathBuf)
Message::ClearSlot(u8)
Message::OpenContextMenu(String, Point)  // (sound_path, cursor_pos)
Message::CloseContextMenu
Message::DismissShortcutsWarning

// src/app.rs — new state fields
shortcuts_status: ShortcutsStatus,
slots: SlotMap,
context_menu: Option<(String, Point)>,
shortcuts_warning_dismissed: bool,
```

## Portal Integration

`portal.rs` exposes:
```rust
pub fn shortcut_stream() -> impl Stream<Item = ShortcutEvent>
```

Internal sequence:
1. Connect to `GlobalShortcuts` portal via ashpd
2. Create session
3. Register 20 shortcuts: id `"slot-{n}"`, description `"Slot {n}"` for n in 1..=20
4. Await portal confirmation (KDE shows native dialog)
5. Emit `ShortcutEvent::Ready`
6. For each `activated` signal: parse slot index from id, emit `ShortcutEvent::Activated(idx)`
7. On any `PortalError`: emit `ShortcutEvent::Failed(reason)`, stream ends

Subscription wiring in `app.rs`:
```rust
fn subscription(&self) -> Subscription<Message> {
    let shortcuts = Subscription::run(shortcut_stream).map(|ev| match ev {
        ShortcutEvent::Ready => Message::ShortcutsReady,
        ShortcutEvent::Activated(idx) => Message::ShortcutActivated(idx),
        ShortcutEvent::Failed(reason) => Message::ShortcutsUnavailable(reason),
    });
    Subscription::batch([shortcuts, /* existing tray subscription */])
}
```

`ShortcutActivated(idx)` handler:
```rust
if let Some(path) = self.slots.get(idx) {
    if let Some(audio) = &self.audio {
        audio.send(AudioCommand::StopAll);
        audio.send(AudioCommand::Play(path.clone()));
    }
}
```

## UI Changes

### Hotkey badge (`sound_card.rs`)
- Compute `slot_idx = app.slots.slot_for(sound_path)`
- If `Some(idx)` and `shortcuts_status == Active`: render monospace `"F{idx+1}"` badge bottom-right
- Plain `text` widget, pill-shaped container, `theme::radius::pill`
- No badge when `Initializing` or `Unavailable`

### Right-click context menu (`sound_grid.rs`)
- Wrap each tile in `mouse_area()`, `on_right_press` → `Message::OpenContextMenu(path, cursor_pos)`
- Context menu rendered as overlay: column of buttons
  - "Assign to Slot ▶" → nested column of 20 slot buttons (each emits `AssignSlot(idx, path)`)
  - "Clear Slot" (only if sound has slot assigned) → `ClearSlot(idx)`
- Click outside → `Message::CloseContextMenu`
- `AssignSlot`: updates `SlotMap`, saves `slots.json`
- `ClearSlot`: clears slot in `SlotMap`, saves `slots.json`

### Warning banner (`app.rs` view)
- Shown when `shortcuts_status == Unavailable(_)` and `!shortcuts_warning_dismissed`
- Amber banner at top of main content area
- Text: `"Global shortcuts unavailable: {reason}. Check xdg-desktop-portal is running."`
- Dismiss button (×) → `Message::DismissShortcutsWarning`

## Persistence

`SlotMap` path: `$XDG_CONFIG_HOME/honkhonk/slots.json`

Format:
```json
{"slots": [null, "/home/user/Music/HonkHonk/goose.mp3", null, ...]}
```

Loaded at startup in `main.rs` alongside `AppConfig`. Passed into `HonkHonk::new()`.
Saved on every `AssignSlot` and `ClearSlot`.
Missing or corrupt file → silent default (all slots empty), no error shown.

## Testing

### Unit tests (written before implementation)

**`src/state/slots.rs`**
- Round-trip: serialize → deserialize → equal
- `get(idx)` returns correct path after `set`
- `clear(idx)` sets slot to None
- `slot_for(path)` returns correct index; returns None when unassigned
- `load()` on missing file returns all-None default (no panic)
- `save()` + `load()` round-trip via tempfile

**`src/app.rs`**
- `ShortcutActivated(idx)` with unassigned slot: state unchanged (no panic, no play)
- `ShortcutActivated(idx)` with assigned slot: `playing` field updates to assigned path
- `AssignSlot(idx, path)`: `slots.get(idx)` returns path after update
- `ClearSlot(idx)`: `slots.get(idx)` returns None after clear
- `ShortcutsUnavailable(reason)`: `shortcuts_status` becomes `Unavailable`
- `ShortcutsReady`: `shortcuts_status` becomes `Active`
- `DismissShortcutsWarning`: `shortcuts_warning_dismissed` becomes true

### Not unit tested
- `portal.rs` — requires live xdg-desktop-portal. Covered by manual integration testing only.
- Right-click overlay rendering — Iced framework responsibility.
- `PortalError` display strings — wording; not user-facing behavior. Behavior covered by `ShortcutsUnavailable` state test.

### CI
- All new tests run under `cargo test` (no feature gate needed — no PipeWire/portal dependency)
- `portal.rs` integration excluded from CI (same pattern as `pipewire-test` feature gate)
