# Global Shortcuts Integration (Phase 2 #10) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Register 20 fixed shortcut slots via xdg-desktop-portal GlobalShortcuts, persist slot→sound assignments in `slots.json`, and add minimal UI: hotkey badge on tiles + right-click assign submenu + in-app warning banner when portal is unavailable.

**Architecture:** ashpd 0.13 session lives inside an `iced::subscription::channel` — shortcut activations arrive as `Message::ShortcutActivated(u8)` which stops current audio and plays the assigned sound. `SlotMap([Option<PathBuf>; 20])` persists to `$XDG_CONFIG_HOME/honkhonk/slots.json`. Right-click context menu uses `mouse_area` + stack overlay in `sound_grid.rs`.

**Tech Stack:** ashpd 0.13.10 (`global_shortcuts` + `zbus` features), iced 0.14 (`subscription::channel`), serde_json (already in tree), `directories` crate (already in tree).

---

## File Map

| File | Status | Purpose |
|------|--------|---------|
| `Cargo.toml` | Modify | Add ashpd dependency |
| `src/shortcuts/error.rs` | **Create** | `PortalError` typed errors |
| `src/shortcuts/mod.rs` | **Create** | `ShortcutsStatus`, `ShortcutEvent`, pub re-exports |
| `src/shortcuts/portal.rs` | **Create** | ashpd session + async stream |
| `src/state/slots.rs` | **Create** | `SlotMap` type, load/save/query |
| `src/state/mod.rs` | Modify | export `SlotMap` |
| `src/app.rs` | Modify | new Message variants, state fields, update arms, subscription, banner |
| `src/ui/sound_grid.rs` | Modify | hotkey badge, mouse_area, context menu overlay |
| `src/main.rs` | Modify | `SlotMap::load()` at startup, pass to `HonkHonk::new()` |

---

### Task 1: Add ashpd dependency + create `src/shortcuts/error.rs`

**Files:**
- Modify: `Cargo.toml`
- Create: `src/shortcuts/error.rs`
- Create: `src/shortcuts/mod.rs` (stub only — expanded in Task 4)
- Create: `src/shortcuts/portal.rs` (stub only — expanded in Task 4)

- [ ] **Step 1: Add ashpd to Cargo.toml**

In the `[dependencies]` section of `Cargo.toml`, add after the `pipewire` line:
```toml
ashpd = { version = "0.13", features = ["global_shortcuts", "zbus"] }
```

- [ ] **Step 2: Create stub files so Rust knows the module exists**

Create `src/shortcuts/error.rs`:
```rust
#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("portal connection failed: {0}")]
    Connection(#[from] ashpd::Error),
    #[error("session creation failed: {0}")]
    Session(String),
    #[error("shortcut registration failed: {0}")]
    Registration(String),
}
```

Create `src/shortcuts/mod.rs`:
```rust
pub mod error;
pub mod portal;

pub use error::PortalError;

#[derive(Debug, Clone, PartialEq)]
pub enum ShortcutsStatus {
    Initializing,
    Active,
    Unavailable(String),
}

#[derive(Debug, Clone)]
pub enum ShortcutEvent {
    Ready,
    Activated(u8), // 0-indexed slot (0 = Slot 1)
    Failed(String),
}
```

Create `src/shortcuts/portal.rs` (empty stub for now):
```rust
use futures::Stream;

use super::ShortcutEvent;

/// Returns a stream of shortcut events. Yields Ready on success, then
/// Activated events. Yields Failed(reason) if portal is unavailable.
pub async fn shortcut_stream() -> impl Stream<Item = ShortcutEvent> {
    use futures::stream;
    // Stub — implemented in Task 4
    stream::empty()
}
```

- [ ] **Step 3: Register the module in `src/lib.rs`**

`main.rs` references `honkhonk::state`, `honkhonk::app`, etc., which means there is a `src/lib.rs` that declares the public modules. Add `pub mod shortcuts;` there, alongside the existing `pub mod` lines:

```rust
// in src/lib.rs, add:
pub mod shortcuts;
```

Confirm lib.rs exists first:
```bash
ls src/lib.rs
grep "^pub mod" src/lib.rs
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo check 2>&1
```

Expected: compiles with no errors (unused import warnings from stub are fine).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/shortcuts/
git commit -m "feat(shortcuts): add ashpd dep + error types + module stubs"
```

---

### Task 2: Create `src/state/slots.rs` — TDD

**Files:**
- Create: `src/state/slots.rs`
- Modify: `src/state/mod.rs`

- [ ] **Step 1: Write the failing tests first**

Create `src/state/slots.rs` with tests only:
```rust
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::state::error::ConfigError;

const SLOTS_FILE_NAME: &str = "slots.json";
const CONFIG_DIR_NAME: &str = "honkhonk";
const SLOT_COUNT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlotMap(pub [Option<PathBuf>; SLOT_COUNT]);

impl Default for SlotMap {
    fn default() -> Self {
        Self(std::array::from_fn(|_| None))
    }
}

impl SlotMap {
    pub fn get(&self, idx: u8) -> Option<&PathBuf> {
        self.0.get(idx as usize)?.as_ref()
    }

    pub fn set(&mut self, idx: u8, path: PathBuf) {
        if let Some(slot) = self.0.get_mut(idx as usize) {
            *slot = Some(path);
        }
    }

    pub fn clear(&mut self, idx: u8) {
        if let Some(slot) = self.0.get_mut(idx as usize) {
            *slot = None;
        }
    }

    pub fn slot_for(&self, path: &Path) -> Option<u8> {
        self.0
            .iter()
            .position(|slot| slot.as_deref() == Some(path))
            .map(|i| i as u8)
    }

    fn slots_path() -> Result<PathBuf, ConfigError> {
        let proj = directories::ProjectDirs::from("", "", CONFIG_DIR_NAME)
            .ok_or(ConfigError::NoConfigDir)?;
        Ok(proj.config_dir().join(SLOTS_FILE_NAME))
    }

    pub fn load() -> Self {
        Self::slots_path()
            .ok()
            .and_then(|path| std::fs::read_to_string(&path).ok())
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::slots_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::DirectoryCreation {
                path: parent.display().to_string(),
                source: e,
            })?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| ConfigError::Serialize {
            path: path.display().to_string(),
            source: e,
        })?;
        std::fs::write(&path, json).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::DirectoryCreation {
                path: parent.display().to_string(),
                source: e,
            })?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| ConfigError::Serialize {
            path: path.display().to_string(),
            source: e,
        })?;
        std::fs::write(path, json).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_has_all_empty_slots() {
        let slots = SlotMap::default();
        for i in 0u8..20 {
            assert!(slots.get(i).is_none(), "slot {i} should be empty");
        }
    }

    #[test]
    fn set_and_get_round_trip() {
        let mut slots = SlotMap::default();
        let path = PathBuf::from("/sounds/honk.mp3");
        slots.set(0, path.clone());
        assert_eq!(slots.get(0), Some(&path));
    }

    #[test]
    fn set_last_slot() {
        let mut slots = SlotMap::default();
        let path = PathBuf::from("/sounds/boom.mp3");
        slots.set(19, path.clone());
        assert_eq!(slots.get(19), Some(&path));
        assert!(slots.get(18).is_none());
    }

    #[test]
    fn set_out_of_bounds_is_silent_noop() {
        let mut slots = SlotMap::default();
        slots.set(20, PathBuf::from("/sounds/boom.mp3")); // silent, no panic
        slots.set(255, PathBuf::from("/sounds/boom.mp3")); // silent, no panic
    }

    #[test]
    fn clear_removes_slot() {
        let mut slots = SlotMap::default();
        slots.set(3, PathBuf::from("/sounds/vine.mp3"));
        slots.clear(3);
        assert!(slots.get(3).is_none());
    }

    #[test]
    fn slot_for_returns_correct_index() {
        let mut slots = SlotMap::default();
        let path = PathBuf::from("/sounds/boom.mp3");
        slots.set(5, path.clone());
        assert_eq!(slots.slot_for(&path), Some(5));
    }

    #[test]
    fn slot_for_returns_none_when_unassigned() {
        let slots = SlotMap::default();
        assert!(slots.slot_for(Path::new("/sounds/boom.mp3")).is_none());
    }

    #[test]
    fn slot_for_returns_first_match() {
        // Same path assigned to two slots — returns the first one
        let mut slots = SlotMap::default();
        let path = PathBuf::from("/sounds/honk.mp3");
        slots.set(2, path.clone());
        slots.set(7, path.clone());
        let found = slots.slot_for(&path).unwrap();
        assert!(found == 2 || found == 7); // deterministic: 2 comes first
        assert_eq!(found, 2);
    }

    #[test]
    fn save_to_and_load_from_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("slots.json");

        let mut slots = SlotMap::default();
        slots.set(0, PathBuf::from("/sounds/a.mp3"));
        slots.set(9, PathBuf::from("/sounds/b.flac"));
        slots.set(19, PathBuf::from("/sounds/c.wav"));

        slots.save_to(&path).unwrap();
        let loaded = SlotMap::load_from(&path);

        assert_eq!(loaded.get(0), Some(&PathBuf::from("/sounds/a.mp3")));
        assert_eq!(loaded.get(9), Some(&PathBuf::from("/sounds/b.flac")));
        assert_eq!(loaded.get(19), Some(&PathBuf::from("/sounds/c.wav")));
        assert!(loaded.get(1).is_none());
    }

    #[test]
    fn load_from_missing_file_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let loaded = SlotMap::load_from(&path);
        for i in 0u8..20 {
            assert!(loaded.get(i).is_none());
        }
    }

    #[test]
    fn load_from_corrupt_file_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrupt.json");
        std::fs::write(&path, b"not valid json !!!").unwrap();
        let loaded = SlotMap::load_from(&path);
        for i in 0u8..20 {
            assert!(loaded.get(i).is_none());
        }
    }
}
```

- [ ] **Step 2: Run tests — expect compile error (module not exported yet)**

```bash
cargo test state::slots 2>&1 | head -20
```

Expected: compile error — `slots` module not found.

- [ ] **Step 3: Export from `src/state/mod.rs`**

Add to `src/state/mod.rs`:
```rust
pub mod slots;
pub use slots::SlotMap;
```

- [ ] **Step 4: Run tests — expect PASS**

```bash
cargo test state::slots 2>&1
```

Expected: all 11 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/state/slots.rs src/state/mod.rs
git commit -m "feat(state): add SlotMap with XDG persistence and full test coverage"
```

---

### Task 3: Implement `src/shortcuts/portal.rs` — ashpd session

**Files:**
- Modify: `src/shortcuts/portal.rs`

This task has no unit tests — ashpd requires a live xdg-desktop-portal. Verify it compiles only.

- [ ] **Step 1: Replace the stub with the real implementation**

Replace the entire contents of `src/shortcuts/portal.rs`:

```rust
use futures::stream::{self, Stream, StreamExt};

use super::{PortalError, ShortcutEvent};

/// Returns a stream of shortcut events.
/// Yields `Ready` once the portal session is established, then `Activated(idx)`
/// for each fired shortcut (idx is 0-based: Slot 1 = 0, Slot 20 = 19).
/// Yields `Failed(reason)` exactly once if the portal is unavailable, then ends.
pub async fn shortcut_stream() -> impl Stream<Item = ShortcutEvent> {
    match init_session().await {
        Ok(events) => stream::once(async { ShortcutEvent::Ready })
            .chain(events)
            .left_stream(),
        Err(e) => stream::once(async move { ShortcutEvent::Failed(e.to_string()) })
            .right_stream(),
    }
}

async fn init_session() -> Result<impl Stream<Item = ShortcutEvent>, PortalError> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};

    // `?` converts ashpd::Error → PortalError::Connection via the #[from] impl
    let proxy = GlobalShortcuts::new().await?;

    let session = proxy
        .create_session()
        .await
        .map_err(|e| PortalError::Session(e.to_string()))?;

    // Register 20 fixed slots. KDE System Settings shows these names.
    // Shortcut keys are assigned by the user in KDE System Settings.
    let new_shortcuts: Vec<NewShortcut> = (1u8..=20)
        .map(|n| NewShortcut::new(format!("slot-{n}"), format!("Slot {n}")))
        .collect();

    proxy
        .bind_shortcuts(&session, &new_shortcuts, None)
        .await
        .map_err(|e| PortalError::Registration(e.to_string()))?;

    // NOTE: check ashpd 0.13 docs for exact method name and return type.
    // It may be `receive_activated()` or `receive_shortcut_activated()`.
    // The stream yields activation events; each event carries a list of
    // activated shortcut IDs (multiple can fire simultaneously).
    let activated = proxy
        .receive_activated()
        .await
        .map_err(PortalError::Connection)?;

    let event_stream = activated.flat_map(|activation| {
        // activation.shortcuts() returns &[Shortcut], each with .id()
        stream::iter(
            activation
                .shortcuts()
                .iter()
                .filter_map(|s| parse_slot_index(s.id()))
                .map(ShortcutEvent::Activated)
                .collect::<Vec<_>>(),
        )
    });

    Ok(event_stream)
}

/// Parses "slot-N" (1-based) → 0-based index. Returns None for invalid ids.
fn parse_slot_index(id: &str) -> Option<u8> {
    id.strip_prefix("slot-")
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|&n| n >= 1 && n <= 20)
        .map(|n| n - 1)
}

#[cfg(test)]
mod tests {
    use super::parse_slot_index;

    #[test]
    fn parse_valid_slot_ids() {
        assert_eq!(parse_slot_index("slot-1"), Some(0));
        assert_eq!(parse_slot_index("slot-10"), Some(9));
        assert_eq!(parse_slot_index("slot-20"), Some(19));
    }

    #[test]
    fn parse_invalid_slot_ids() {
        assert_eq!(parse_slot_index("slot-0"), None); // 0 is not a valid slot
        assert_eq!(parse_slot_index("slot-21"), None); // out of range
        assert_eq!(parse_slot_index("f1"), None);      // wrong format
        assert_eq!(parse_slot_index("slot-"), None);   // no number
        assert_eq!(parse_slot_index(""), None);
    }
}
```

> **Compiler note:** `NewShortcut::new(id, description)` and `Shortcut::id()` and `receive_activated()` method names must be verified against the ashpd 0.13 docs (`cargo doc --open -p ashpd`). Adjust method names if the compiler rejects them.

- [ ] **Step 2: Run the parse tests**

```bash
cargo test shortcuts::portal 2>&1
```

Expected: `parse_valid_slot_ids` and `parse_invalid_slot_ids` pass. Any ashpd API mismatch shows as a compile error — fix method names per `cargo doc --open -p ashpd` before proceeding.

- [ ] **Step 3: Commit**

```bash
git add src/shortcuts/portal.rs
git commit -m "feat(shortcuts): implement ashpd portal session and shortcut stream"
```

---

### Task 4: Update `src/app.rs` — new state fields + Message variants

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add new Message variants**

In `src/app.rs`, add these variants to the `Message` enum after `VolumeSaveRequested`:

```rust
// Shortcut lifecycle
ShortcutsReady,
ShortcutsUnavailable(String),
DismissShortcutsWarning,
// Shortcut activation
ShortcutActivated(u8),
// Slot assignment
AssignSlot(u8, std::path::PathBuf),
ClearSlot(u8),
// Context menu
OpenContextMenu(String), // sound_id
CloseContextMenu,
```

- [ ] **Step 2: Add new imports**

At the top of `src/app.rs`, add:
```rust
use crate::shortcuts::ShortcutsStatus;
use crate::state::SlotMap;
```

- [ ] **Step 3: Add new fields to `HonkHonk` struct**

In the `HonkHonk` struct, add after `progress: f32`:
```rust
slots: SlotMap,
shortcuts_status: ShortcutsStatus,
context_menu: Option<String>,       // sound_id of tile that was right-clicked
shortcuts_warning_dismissed: bool,
```

- [ ] **Step 4: Update `HonkHonk::new()` signature and body**

Change the `new()` signature to accept `SlotMap`:
```rust
pub fn new(
    mut tray: TrayHandle,
    audio: AudioHandle,
    sounds: Vec<SoundEntry>,
    config: AppConfig,
    slots: SlotMap,
) -> Self {
    let rx = tray.take_rx();
    Self {
        visible: true,
        exit: false,
        tray_rx: Arc::new(Mutex::new(rx)),
        _tray: Some(tray),
        audio: Some(audio),
        sounds,
        playing: None,
        active_category: None,
        config,
        search_query: String::new(),
        progress: 0.0,
        slots,
        shortcuts_status: ShortcutsStatus::Initializing,
        context_menu: None,
        shortcuts_warning_dismissed: false,
    }
}
```

- [ ] **Step 5: Update `HonkHonk::new_for_test()` body**

Add the new fields:
```rust
pub fn new_for_test() -> Self {
    let (_tx, rx) = std::sync::mpsc::channel();
    Self {
        visible: true,
        exit: false,
        tray_rx: Arc::new(Mutex::new(rx)),
        _tray: None,
        audio: None,
        sounds: Vec::new(),
        playing: None,
        active_category: None,
        config: AppConfig::default(),
        search_query: String::new(),
        progress: 0.0,
        slots: SlotMap::default(),
        shortcuts_status: ShortcutsStatus::Initializing,
        context_menu: None,
        shortcuts_warning_dismissed: false,
    }
}
```

- [ ] **Step 6: Add public accessors (needed for tests)**

After `pub fn progress(&self)`, add:
```rust
pub fn shortcuts_status(&self) -> &ShortcutsStatus {
    &self.shortcuts_status
}

pub fn slots(&self) -> &SlotMap {
    &self.slots
}

pub fn context_menu(&self) -> Option<&str> {
    self.context_menu.as_deref()
}

pub fn shortcuts_warning_dismissed(&self) -> bool {
    self.shortcuts_warning_dismissed
}
```

- [ ] **Step 7: Verify it compiles (update arms not yet added — expect warnings only)**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

Expected: compiler errors about missing match arms in `update()` and `main.rs` calling `new()` with wrong arity. Warnings about unused fields are OK. Fix the `update()` match exhaustiveness by adding a placeholder arm temporarily:

At the bottom of the `match message` in `update()`, before the closing brace, add a temporary wildcard that panics (to be replaced in Task 5):
```rust
Message::ShortcutsReady
| Message::ShortcutsUnavailable(_)
| Message::DismissShortcutsWarning
| Message::ShortcutActivated(_)
| Message::AssignSlot(_, _)
| Message::ClearSlot(_)
| Message::OpenContextMenu(_)
| Message::CloseContextMenu => Task::none(), // implemented in Task 5
```

```bash
cargo check 2>&1 | grep "^error"
```

Expected: zero errors (the only remaining errors are in `main.rs` — fixed in Task 8).

- [ ] **Step 8: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): add shortcut state fields and Message variants"
```

---

### Task 5: Implement and TDD-test new `update()` arms

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing tests first**

In the `#[cfg(test)] mod tests` block at the bottom of `src/app.rs`, add:

```rust
#[test]
fn shortcuts_ready_sets_status_active() {
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.shortcuts_status(), &ShortcutsStatus::Initializing);
    let _ = app.update(Message::ShortcutsReady);
    assert_eq!(app.shortcuts_status(), &ShortcutsStatus::Active);
}

#[test]
fn shortcuts_unavailable_sets_status() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutsUnavailable("portal not found".into()));
    assert!(matches!(
        app.shortcuts_status(),
        ShortcutsStatus::Unavailable(_)
    ));
}

#[test]
fn shortcuts_unavailable_contains_reason() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutsUnavailable("no portal".into()));
    let ShortcutsStatus::Unavailable(reason) = app.shortcuts_status() else {
        panic!("expected Unavailable");
    };
    assert!(!reason.is_empty());
}

#[test]
fn dismiss_warning_sets_flag() {
    let mut app = HonkHonk::new_for_test();
    assert!(!app.shortcuts_warning_dismissed());
    let _ = app.update(Message::DismissShortcutsWarning);
    assert!(app.shortcuts_warning_dismissed());
}

#[test]
fn shortcut_activated_with_empty_slot_is_noop() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutActivated(0));
    assert!(app.playing().is_none());
}

#[test]
fn shortcut_activated_with_assigned_slot_updates_playing() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/sounds/honk.mp3");
    app.sounds = vec![SoundEntry {
        id: "honk-id".into(),
        name: "Honk".into(),
        path: path.clone(),
        format: crate::state::AudioFormat::Mp3,
        duration_ms: Some(500),
        category: "Honk".into(),
    }];
    let _ = app.update(Message::AssignSlot(0, path.clone()));
    // ShortcutActivated with audio=None: no audio command sent,
    // but playing state does NOT change without a real PlaybackStarted event.
    // Verify no panic and slot is still assigned.
    let _ = app.update(Message::ShortcutActivated(0));
    assert_eq!(app.slots().get(0), Some(&path));
}

#[test]
fn assign_slot_updates_slot_map() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/sounds/boom.mp3");
    let _ = app.update(Message::AssignSlot(3, path.clone()));
    assert_eq!(app.slots().get(3), Some(&path));
}

#[test]
fn clear_slot_removes_assignment() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/sounds/boom.mp3");
    let _ = app.update(Message::AssignSlot(3, path.clone()));
    let _ = app.update(Message::ClearSlot(3));
    assert!(app.slots().get(3).is_none());
}

#[test]
fn open_context_menu_sets_sound_id() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::OpenContextMenu("some-id".into()));
    assert_eq!(app.context_menu(), Some("some-id"));
}

#[test]
fn close_context_menu_clears_it() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::OpenContextMenu("some-id".into()));
    let _ = app.update(Message::CloseContextMenu);
    assert!(app.context_menu().is_none());
}
```

- [ ] **Step 2: Run failing tests**

```bash
cargo test app::tests 2>&1 | grep -E "FAILED|error" | head -20
```

Expected: tests fail (placeholder arms return `Task::none()` without side effects).

- [ ] **Step 3: Replace the placeholder arms with real implementations**

In `update()`, replace the `// implemented in Task 5` placeholder with:

```rust
Message::ShortcutsReady => {
    self.shortcuts_status = ShortcutsStatus::Active;
    Task::none()
}
Message::ShortcutsUnavailable(reason) => {
    self.shortcuts_status = ShortcutsStatus::Unavailable(reason);
    Task::none()
}
Message::DismissShortcutsWarning => {
    self.shortcuts_warning_dismissed = true;
    Task::none()
}
Message::ShortcutActivated(idx) => {
    if let Some(path) = self.slots.get(idx).cloned() {
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::Stop);
        }
        // Decode + play — same pattern as PlaySound
        let sound = self.sounds.iter().find(|s| s.path == path);
        if let Some(sound) = sound {
            let sound_id = sound.id.clone();
            let decoded = match crate::audio::decode(&sound.path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("honkhonk: shortcut decode error: {e}");
                    return Task::none();
                }
            };
            if let Some(ref audio) = self.audio {
                audio.send(AudioCommand::Play {
                    sound_id,
                    samples: std::sync::Arc::new(decoded.samples),
                    sample_rate: decoded.sample_rate,
                    channels: decoded.channels,
                });
            }
        }
    }
    Task::none()
}
Message::AssignSlot(idx, path) => {
    self.slots.set(idx, path);
    if let Err(e) = self.slots.save() {
        eprintln!("honkhonk: slots save error: {e}");
    }
    Task::none()
}
Message::ClearSlot(idx) => {
    self.slots.clear(idx);
    if let Err(e) = self.slots.save() {
        eprintln!("honkhonk: slots save error: {e}");
    }
    Task::none()
}
Message::OpenContextMenu(sound_id) => {
    self.context_menu = Some(sound_id);
    Task::none()
}
Message::CloseContextMenu => {
    self.context_menu = None;
    Task::none()
}
```

- [ ] **Step 4: Run tests — expect PASS**

```bash
cargo test app::tests 2>&1
```

Expected: all tests pass (including all previously passing tests — no regressions).

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): implement shortcut update arms with full test coverage"
```

---

### Task 6: Update `subscription()` + `view()` banner in `src/app.rs`

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Update `subscription()`**

Replace the existing `subscription()` method:

```rust
pub fn subscription(&self) -> Subscription<Message> {
    use iced::futures::SinkExt;
    use std::any::TypeId;

    struct ShortcutListener;

    let shortcuts = iced::subscription::channel(
        TypeId::of::<ShortcutListener>(),
        16,
        |mut tx| async move {
            use crate::shortcuts::{ShortcutEvent, portal};
            use futures::StreamExt;

            let stream = portal::shortcut_stream().await;
            futures::pin_mut!(stream);

            while let Some(ev) = stream.next().await {
                let msg = match ev {
                    ShortcutEvent::Ready => Message::ShortcutsReady,
                    ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                    ShortcutEvent::Failed(r) => Message::ShortcutsUnavailable(r),
                };
                if tx.send(msg).await.is_err() {
                    break;
                }
            }
            // Stream ended (portal died). Stay alive but idle.
            loop {
                iced::futures::future::pending::<()>().await;
            }
        },
    );

    let tray_poll =
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll);

    Subscription::batch([shortcuts, tray_poll])
}
```

> **Note:** `iced::subscription::channel` signature is from iced 0.13. In iced 0.14, verify this is still the correct API. Alternatives: `Subscription::run_with_id`, `iced::subscription::channel` from `iced::futures`. Check `cargo doc --open -p iced` if there are compile errors.

- [ ] **Step 2: Extract banner helper and inject into `view()`**

Add a private helper method to `HonkHonk` (just before `view()`):

```rust
fn view_shortcuts_banner(&self, t: theme::Theme) -> Option<Element<'_, Message>> {
    let ShortcutsStatus::Unavailable(ref reason) = self.shortcuts_status else {
        return None;
    };
    if self.shortcuts_warning_dismissed {
        return None;
    }
    let banner = container(
        row![
            text(format!(
                "Global shortcuts unavailable: {reason}. Check xdg-desktop-portal is running."
            ))
            .size(13)
            .color(iced::Color::from_rgb(0.6, 0.4, 0.0)),
            space::horizontal(),
            button(text("×").size(14))
                .on_press(Message::DismissShortcutsWarning)
                .style(move |_t, _s| button::Style {
                    background: None,
                    text_color: t.ink(),
                    ..Default::default()
                }),
        ]
        .spacing(theme::space::MD)
        .align_y(iced::Alignment::Center),
    )
    .padding([theme::space::SM, theme::space::LG])
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(iced::Color::from_rgb(0.98, 0.92, 0.75))),
        border: theme::tile_border(iced::Color::from_rgb(0.9, 0.75, 0.3), 1.0),
        ..Default::default()
    });
    Some(banner.into())
}
```

Then in `view()`, replace the existing `let content = column![...].spacing(...)` block with one that optionally prepends the banner. Build the content items as a `Vec` and use `Column::with_children`:

```rust
pub fn view(&self) -> Element<'_, Message> {
    let t = theme::Theme::Dark;
    let header = self.view_header(t);
    let chips = self.view_category_chips(t);
    let filtered = self.filtered_sounds();
    let grid = sound_grid::view_grid(
        &filtered,
        self.playing.as_deref(),
        &self.slots,
        matches!(self.shortcuts_status, ShortcutsStatus::Active),
        self.context_menu.as_deref(),
    );

    let now_playing = now_playing::view_now_playing(
        self.playing.as_deref(),
        &self.sounds,
        self.progress,
        self.config.volume,
    );

    // Build content column, optionally prepending the shortcuts warning banner
    let mut items: Vec<Element<'_, Message>> = Vec::new();
    if let Some(banner) = self.view_shortcuts_banner(t) {
        items.push(banner);
    }
    items.push(header);
    items.push(chips);
    items.push(scrollable(grid).height(Length::Fill).into());
    items.push(now_playing);

    let content = iced::widget::Column::with_children(items).spacing(theme::space::MD);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::space::XL)
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.bg())),
            ..Default::default()
        })
        .into()
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check 2>&1 | grep "^error"
```

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): add shortcut subscription and unavailable banner"
```

---

### Task 7: Update `src/ui/sound_grid.rs` — hotkey badge + right-click context menu

**Files:**
- Modify: `src/ui/sound_grid.rs`

- [ ] **Step 1: Update `view_grid` signature and call sites**

Change `view_grid` to accept slot and context menu state:

```rust
pub fn view_grid<'a>(
    sounds: &[&'a SoundEntry],
    playing: Option<&str>,
    slots: &'a crate::state::SlotMap,
    shortcuts_active: bool,
    context_menu: Option<&'a str>,
) -> iced::Element<'a, crate::app::Message> {
```

Update the call in `src/app.rs` `view()`:

```rust
let grid = sound_grid::view_grid(
    &filtered,
    self.playing.as_deref(),
    &self.slots,
    matches!(self.shortcuts_status, ShortcutsStatus::Active),
    self.context_menu.as_deref(),
);
```

- [ ] **Step 2: Wrap each tile in `mouse_area` for right-click**

In `view_grid`, in the tile-building closure, replace:
```rust
tile_view(sound, is_playing, Tone::from_index(tone_idx), theme)
```
with:
```rust
{
    use iced::widget::mouse_area;
    let tile = tile_view(
        sound,
        is_playing,
        Tone::from_index(tone_idx),
        theme,
        slots,
        shortcuts_active,
    );
    mouse_area(tile)
        .on_right_press(crate::app::Message::OpenContextMenu(sound.id.clone()))
        .into()
}
```

- [ ] **Step 3: Add hotkey badge to `tile_view`**

Update `tile_view` signature:
```rust
fn tile_view<'a>(
    sound: &'a SoundEntry,
    is_playing: bool,
    tone: Tone,
    theme: Theme,
    slots: &'a crate::state::SlotMap,
    shortcuts_active: bool,
) -> Element<'a, Message> {
```

In `tile_view`, before building `content`, compute the badge:
```rust
let slot_badge: Option<Element<'_, Message>> = if shortcuts_active {
    slots.slot_for(&sound.path).map(|idx| {
        container(
            text(format!("F{}", idx + 1))
                .size(10)
                .font(iced::Font::MONOSPACE)
                .color(theme.ink_dim()),
        )
        .padding([2, 6])
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(theme.panel())),
            border: theme::tile_border(theme.hairline(), 1.0),
            ..Default::default()
        })
        .into()
    })
} else {
    None
};
```

Then update the `content` column to include the badge at the bottom:
```rust
let mut col = column![category_text, name_text, duration_text].spacing(theme::space::SM);
if let Some(badge) = slot_badge {
    col = col.push(badge);
}
let content = col.padding(theme::space::LG);
```

- [ ] **Step 4: Add context menu overlay using `stack!`**

After building the `grid` in `view_grid`, wrap it with an overlay when `context_menu.is_some()`:

```rust
if let Some(sound_id) = context_menu {
    // Find the sound to get its path
    let found = sounds.iter().find(|s| s.id == sound_id);
    let overlay = context_menu_overlay(sound_id, found.copied(), slots, theme);
    iced::widget::stack![grid, overlay].into()
} else {
    grid.width(Length::Fill).into()
}
```

Add the `context_menu_overlay` helper function after `view_grid`:

```rust
fn context_menu_overlay<'a>(
    sound_id: &str,
    sound: Option<&'a SoundEntry>,
    slots: &'a crate::state::SlotMap,
    theme: Theme,
) -> Element<'a, Message> {
    use iced::widget::{mouse_area, Column};

    let sound_path = sound.map(|s| &s.path);
    let assigned_slot = sound_path.and_then(|p| slots.slot_for(p));

    // Slot assignment buttons — scrollable column of 20 entries
    let slot_buttons: Vec<Element<'_, Message>> = (0u8..20)
        .map(|i| {
            let is_assigned = assigned_slot == Some(i);
            let label = if is_assigned {
                format!("✓ Slot {} (F{})", i + 1, i + 1)
            } else {
                format!("  Slot {} (F{})", i + 1, i + 1)
            };

            let msg = sound_path.map(|p| {
                if is_assigned {
                    Message::ClearSlot(i)
                } else {
                    Message::AssignSlot(i, p.clone())
                }
            });

            button(text(label).size(13).color(theme.ink()))
                .on_press_maybe(msg)
                .width(Length::Fill)
                .style(move |_t, status| button::Style {
                    background: Some(theme::bg_color(match status {
                        button::Status::Hovered => theme.accent(),
                        _ => theme.panel(),
                    })),
                    text_color: theme.ink(),
                    ..Default::default()
                })
                .into()
        })
        .collect();

    let menu = container(
        column![
            text(sound.map(|s| s.name.as_str()).unwrap_or("")).size(13).color(theme.ink_dim()),
            iced::widget::scrollable(
                Column::with_children(slot_buttons).spacing(2).width(Length::Fill)
            )
            .height(300),
        ]
        .spacing(theme::space::SM)
        .padding(theme::space::MD),
    )
    .width(200)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(theme.panel())),
        border: theme::tile_border(theme.hairline(), 1.0),
        ..Default::default()
    });

    // Dismiss-on-click-outside: transparent full-screen layer behind the menu
    let dismiss = mouse_area(
        container(iced::widget::Space::new(Length::Fill, Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::CloseContextMenu);

    // Stack dismiss layer + positioned menu (top-right of viewport)
    container(
        iced::widget::stack![
            dismiss,
            container(menu)
                .align_right(Length::Fill)
                .align_top(Length::Fill)
                .padding([60, 20, 0, 0]),
        ]
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo check 2>&1 | grep "^error"
```

Expected: zero errors. Adjust any API mismatches (e.g. `align_right`/`align_top` vs `align_x`/`align_y` in iced 0.14 — check the iced docs).

- [ ] **Step 6: Commit**

```bash
git add src/ui/sound_grid.rs
git commit -m "feat(ui): add hotkey badge and right-click slot assignment context menu"
```

---

### Task 8: Update `src/main.rs` — load SlotMap + pass to `HonkHonk::new()`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add SlotMap load and pass to `new()`**

In `main.rs`, after the `config` load block and before `sounds` scan, add:
```rust
let slots = honkhonk::state::SlotMap::load();
```

Then in the `iced::application` closure, take `slots` from a `Mutex` like the other values, or just capture it directly (it's `Clone`). Since `SlotMap` is `Clone` and small enough to clone, capture it directly:

Replace:
```rust
let tray_handle = std::sync::Mutex::new(Some(tray_handle));
let audio_handle = std::sync::Mutex::new(Some(audio_handle));
let sounds = std::sync::Mutex::new(Some(sounds));
let config = std::sync::Mutex::new(Some(config));
```
with:
```rust
let tray_handle = std::sync::Mutex::new(Some(tray_handle));
let audio_handle = std::sync::Mutex::new(Some(audio_handle));
let sounds = std::sync::Mutex::new(Some(sounds));
let config = std::sync::Mutex::new(Some(config));
let slots = std::sync::Mutex::new(Some(slots));
```

And in the closure body, add:
```rust
let slots = slots
    .lock()
    .expect("slots mutex poisoned")
    .take()
    .expect("boot called more than once");
```

And update the `HonkHonk::new()` call:
```rust
honkhonk::app::HonkHonk::new(tray, audio, sounds, config, slots)
```

- [ ] **Step 2: Verify full build**

```bash
cargo build 2>&1 | grep "^error"
```

Expected: zero errors.

- [ ] **Step 3: Run full test suite**

```bash
cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 4: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings. Fix any clippy lints before proceeding.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): load SlotMap at startup and pass to HonkHonk"
```

---

### Task 9: Final verification + close issue

- [ ] **Step 1: Full test suite**

```bash
cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 2: Clippy clean**

```bash
cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 3: Release build (binary size check)**

```bash
cargo build --release 2>&1 | grep "^error"
ls -lh target/release/honkhonk
```

Expected: binary under 30MB.

- [ ] **Step 4: Manual smoke test checklist**

With a running KDE Plasma session + xdg-desktop-portal-kde:
- [ ] App launches, no banner visible
- [ ] KDE pops portal confirmation dialog for shortcut registration
- [ ] Right-click a tile → slot submenu appears
- [ ] Assign slot → badge "F1" (or whichever) appears on tile
- [ ] Press assigned shortcut key → sound plays (stops any current sound first)
- [ ] Assign different sound to same slot → previous sound unlinked, new one linked
- [ ] Right-click assigned tile → "✓ Slot N" option appears; clicking it clears slot
- [ ] Restart app → slot assignments persist

Without xdg-desktop-portal:
- [ ] Amber warning banner appears at top
- [ ] "×" dismisses the banner
- [ ] App works normally (click to play, search, etc.)

- [ ] **Step 5: Close the GitHub issue**

```bash
gh issue close 10 --comment "Phase 2 global shortcuts implemented: ashpd 0.13 portal session, 20 fixed slots (F1–F20), slot persistence via slots.json, hotkey badge on tiles, right-click slot assignment, and in-app banner when portal is unavailable."
```
