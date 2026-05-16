# In-App Shortcut Assignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users click "Set Hotkey" on any bound slot, press a key combo, and have HonkHonk attempt to register it with the XDG GlobalShortcuts portal — showing "Saved" or "Not saved, may be in use" feedback inline.

**Architecture:** The portal stream (`shortcut_stream`) gains a bidirectional command channel: it creates a `(cmd_tx, cmd_rx)` pair, reports `cmd_tx` to the app via `ShortcutEvent::Handle`, and uses `tokio::select!` to multiplex activated events, external `ShortcutsChanged` signals, and app-initiated rebind commands. The app captures key combos via a gated `iced::event::listen_with` subscription active only in capture mode, then sends a `PortalCommand::RebindSlot` through the stored sender.

**Tech Stack:** Rust, Iced 0.13 (subscriptions, `listen_with`), ashpd 0.13.10 (`NewShortcut::preferred_trigger`, `receive_shortcuts_changed`), tokio mpsc channels.

**Spec:** `docs/superpowers/specs/2026-05-15-in-app-shortcut-assignment-design.md`

**Branch:** Create `feat/in-app-shortcut-assignment` from `main` before starting.

```bash
git fetch origin && git checkout -b feat/in-app-shortcut-assignment origin/main
```

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src/state/config.rs` | Modify | Add `desired_triggers` field |
| `src/shortcuts/mod.rs` | Modify | Add `PortalCommand`, `BindFeedback`; extend `ShortcutEvent` |
| `src/shortcuts/capture.rs` | **Create** | `format_combo` — pure key combo formatter |
| `src/shortcuts/portal.rs` | Modify | Bidirectional stream with `tokio::select!` and `ShortcutsChanged` |
| `src/app.rs` | Modify | New messages, state fields, subscription, update handlers |
| `src/ui/slot_manager.rs` | Modify | "Set Hotkey" button, capture overlay, feedback badge |
| `README.md` | Modify | Update status table |
| `ARCHITECTURE.md` | Modify | Update Phase 2 checklist |

---

## Task 1 — Config: Add `desired_triggers` field

**Files:**
- Modify: `src/state/config.rs`

- [ ] **Step 1.1 — Write the failing tests**

Add to the `#[cfg(test)]` block in `src/state/config.rs`:

```rust
#[test]
fn missing_desired_triggers_field_deserializes_to_empty() {
    // Simulates loading a config written before this field existed.
    let json = r#"{"sound_directories":[],"volume":0.85,"window_width":900,"window_height":600,"theme":"Dark","density":"regular","mic_passthrough":true,"mic_passthrough_level":1.0,"renderer":"wgpu"}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert!(config.desired_triggers.iter().all(|t| t.is_none()));
}

#[test]
fn desired_triggers_round_trips_json() {
    let mut config = AppConfig::default();
    config.desired_triggers[0] = Some("Meta+1".into());
    config.desired_triggers[4] = Some("Ctrl+Alt+F5".into());
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.desired_triggers[0].as_deref(), Some("Meta+1"));
    assert_eq!(back.desired_triggers[4].as_deref(), Some("Ctrl+Alt+F5"));
    assert!(back.desired_triggers[1].is_none());
}
```

- [ ] **Step 1.2 — Run tests to verify they fail**

```bash
cargo test -p honkhonk config -- --nocapture 2>&1 | grep -E "FAILED|error|missing_desired|round_trips"
```

Expected: compilation error — field `desired_triggers` does not exist.

- [ ] **Step 1.3 — Add the field to `AppConfig`**

In `src/state/config.rs`, add to the `AppConfig` struct after `monitor_device`:

```rust
    #[serde(default)]
    pub desired_triggers: [Option<String>; 20],
```

The `[Option<String>; 20]` type implements `Default` (all `None`) because `N ≤ 32` and `Option<String>: Default`. No custom default function needed.

- [ ] **Step 1.4 — Update the `Default` impl**

In `AppConfig::default()`, add after `monitor_device: None`:

```rust
            desired_triggers: std::array::from_fn(|_| None),
```

- [ ] **Step 1.5 — Update `round_trip_serialize_deserialize` test**

The existing test constructs `AppConfig` with named fields. Add `desired_triggers`:

```rust
    fn round_trip_serialize_deserialize() {
        let config = AppConfig {
            sound_directories: vec![PathBuf::from("/tmp/sounds")],
            volume: 0.5,
            window_width: 1024,
            window_height: 768,
            theme: Theme::Dark,
            density: Density::Compact,
            mic_passthrough: true,
            mic_passthrough_level: 0.75,
            renderer: Renderer::Wgpu,
            monitor_device: None,
            desired_triggers: std::array::from_fn(|_| None),
        };
        // ... rest unchanged
    }
```

Do the same for `save_and_load_from_path`.

- [ ] **Step 1.6 — Run tests to verify they pass**

```bash
cargo test -p honkhonk config -- --nocapture
```

Expected: All config tests pass.

- [ ] **Step 1.7 — Commit**

```bash
git add src/state/config.rs
git commit -m "feat(state): add desired_triggers to AppConfig for shortcut persistence"
```

---

## Task 2 — Shortcuts Types: `PortalCommand`, `BindFeedback`, extended `ShortcutEvent`

**Files:**
- Modify: `src/shortcuts/mod.rs`

- [ ] **Step 2.1 — Write failing test**

Add to `src/shortcuts/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_feedback_default_is_unset() {
        assert_eq!(BindFeedback::default(), BindFeedback::Unset);
    }
}
```

- [ ] **Step 2.2 — Run to verify it fails**

```bash
cargo test -p honkhonk shortcuts -- --nocapture 2>&1 | head -20
```

Expected: compilation error — `BindFeedback` not defined.

- [ ] **Step 2.3 — Add `PortalCommand`, `BindFeedback`, extend `ShortcutEvent`**

Replace the full contents of `src/shortcuts/mod.rs` with:

```rust
pub mod error;
pub mod portal;
pub mod capture;

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
    /// The stream's command sender — store this to send rebind requests.
    Handle(tokio::sync::mpsc::Sender<PortalCommand>),
    Activated(u8),
    /// Initial bindings from BindShortcuts response: (0-indexed slot, trigger string).
    Bindings(Vec<(u8, String)>),
    /// Result of a RebindSlot command: full binding set returned by portal.
    RebindResult {
        changed_idx: u8,
        bindings: Vec<(u8, String)>,
    },
    /// DE changed shortcuts externally (user reconfigured in System Settings).
    Changed(Vec<(u8, String)>),
    Failed(String),
}

/// Commands sent into the running portal stream.
#[derive(Debug)]
pub enum PortalCommand {
    RebindSlot { idx: u8, trigger: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindFeedback {
    #[default]
    Unset,
    Saved,
    NotSaved,
}
```

- [ ] **Step 2.4 — Run tests to verify they pass**

```bash
cargo test -p honkhonk shortcuts -- --nocapture
```

Expected: `bind_feedback_default_is_unset` passes.

- [ ] **Step 2.5 — Commit**

```bash
git add src/shortcuts/mod.rs
git commit -m "feat(shortcuts): add PortalCommand, BindFeedback, extend ShortcutEvent"
```

---

## Task 3 — Keyboard Capture Utility: `src/shortcuts/capture.rs`

**Files:**
- Create: `src/shortcuts/capture.rs`

This file contains only `format_combo` — a pure function with no Iced runtime dependency, making it fully unit-testable.

- [ ] **Step 3.1 — Write failing tests**

Create `src/shortcuts/capture.rs` with tests only:

```rust
use iced::keyboard::{self, key::Named};

/// Formats a key press event into a portal-compatible trigger string.
///
/// Returns `Some("Meta+1")` for valid combos, `None` for:
/// - Bare non-F-key characters without a modifier
/// - Modifier-only presses (Ctrl, Alt, Shift, Super alone)
/// - Escape (caller treats this as Cancel)
/// - Unidentified keys
///
/// F-keys (F1–F12) are accepted without a modifier.
pub fn format_combo(modifiers: keyboard::Modifiers, key: &keyboard::Key) -> Option<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::{self, key::Named};

    fn key(named: Named) -> keyboard::Key {
        keyboard::Key::Named(named)
    }

    fn ch(c: &str) -> keyboard::Key {
        keyboard::Key::Character(c.into())
    }

    fn mods(ctrl: bool, alt: bool, shift: bool, logo: bool) -> keyboard::Modifiers {
        let mut m = keyboard::Modifiers::empty();
        if ctrl { m |= keyboard::Modifiers::CTRL; }
        if alt { m |= keyboard::Modifiers::ALT; }
        if shift { m |= keyboard::Modifiers::SHIFT; }
        if logo { m |= keyboard::Modifiers::LOGO; }
        m
    }

    #[test]
    fn meta_plus_digit() {
        let result = format_combo(mods(false, false, false, true), &ch("1"));
        assert_eq!(result.as_deref(), Some("Meta+1"));
    }

    #[test]
    fn ctrl_alt_f5() {
        let result = format_combo(mods(true, true, false, false), &key(Named::F5));
        assert_eq!(result.as_deref(), Some("Ctrl+Alt+F5"));
    }

    #[test]
    fn f1_without_modifier_is_valid() {
        let result = format_combo(keyboard::Modifiers::empty(), &key(Named::F1));
        assert_eq!(result.as_deref(), Some("F1"));
    }

    #[test]
    fn f12_without_modifier_is_valid() {
        let result = format_combo(keyboard::Modifiers::empty(), &key(Named::F12));
        assert_eq!(result.as_deref(), Some("F12"));
    }

    #[test]
    fn bare_letter_without_modifier_is_none() {
        let result = format_combo(keyboard::Modifiers::empty(), &ch("a"));
        assert!(result.is_none());
    }

    #[test]
    fn bare_digit_without_modifier_is_none() {
        let result = format_combo(keyboard::Modifiers::empty(), &ch("1"));
        assert!(result.is_none());
    }

    #[test]
    fn escape_is_none() {
        let result = format_combo(keyboard::Modifiers::empty(), &key(Named::Escape));
        assert!(result.is_none());
    }

    #[test]
    fn modifier_only_ctrl_is_none() {
        let result = format_combo(mods(true, false, false, false), &key(Named::Control));
        assert!(result.is_none());
    }

    #[test]
    fn ctrl_shift_a_uppercases_character() {
        let result = format_combo(mods(true, false, true, false), &ch("a"));
        assert_eq!(result.as_deref(), Some("Ctrl+Shift+A"));
    }

    #[test]
    fn modifier_order_is_ctrl_alt_shift_meta() {
        let result = format_combo(mods(true, true, true, true), &ch("x"));
        assert_eq!(result.as_deref(), Some("Ctrl+Alt+Shift+Meta+X"));
    }

    #[test]
    fn meta_space() {
        let result = format_combo(mods(false, false, false, true), &key(Named::Space));
        assert_eq!(result.as_deref(), Some("Meta+Space"));
    }

    #[test]
    fn bare_space_is_none() {
        let result = format_combo(keyboard::Modifiers::empty(), &key(Named::Space));
        assert!(result.is_none());
    }
}
```

- [ ] **Step 3.2 — Run tests to verify they fail**

```bash
cargo test -p honkhonk capture -- --nocapture 2>&1 | head -20
```

Expected: panics with `not yet implemented` from `todo!()`.

- [ ] **Step 3.3 — Implement `format_combo`**

Replace the `todo!()` body:

```rust
pub fn format_combo(modifiers: keyboard::Modifiers, key: &keyboard::Key) -> Option<String> {
    let has_modifier = modifiers.control()
        || modifiers.alt()
        || modifiers.shift()
        || modifiers.logo();

    let key_str: String = match key {
        keyboard::Key::Named(named) => match named {
            // Cancel / modifier-only — caller handles Escape separately
            Named::Escape
            | Named::Control
            | Named::Alt
            | Named::Shift
            | Named::Super => return None,
            // F-keys: valid without modifier
            Named::F1 => "F1".into(),
            Named::F2 => "F2".into(),
            Named::F3 => "F3".into(),
            Named::F4 => "F4".into(),
            Named::F5 => "F5".into(),
            Named::F6 => "F6".into(),
            Named::F7 => "F7".into(),
            Named::F8 => "F8".into(),
            Named::F9 => "F9".into(),
            Named::F10 => "F10".into(),
            Named::F11 => "F11".into(),
            Named::F12 => "F12".into(),
            // Named keys that need a modifier
            Named::Space if has_modifier => "Space".into(),
            Named::Enter if has_modifier => "Return".into(),
            Named::Tab if has_modifier => "Tab".into(),
            Named::Delete if has_modifier => "Delete".into(),
            Named::Backspace if has_modifier => "Backspace".into(),
            Named::Home if has_modifier => "Home".into(),
            Named::End if has_modifier => "End".into(),
            Named::PageUp if has_modifier => "PageUp".into(),
            Named::PageDown if has_modifier => "PageDown".into(),
            Named::ArrowUp if has_modifier => "Up".into(),
            Named::ArrowDown if has_modifier => "Down".into(),
            Named::ArrowLeft if has_modifier => "Left".into(),
            Named::ArrowRight if has_modifier => "Right".into(),
            _ => return None,
        },
        keyboard::Key::Character(c) if has_modifier => c.to_uppercase(),
        keyboard::Key::Character(_) | keyboard::Key::Unidentified => return None,
    };

    let mut parts: Vec<&str> = Vec::new();
    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if modifiers.logo() {
        parts.push("Meta");
    }
    parts.push(&key_str);
    Some(parts.join("+"))
}
```

- [ ] **Step 3.4 — Run tests to verify they pass**

```bash
cargo test -p honkhonk capture -- --nocapture
```

Expected: all 12 tests pass.

- [ ] **Step 3.5 — Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep -E "error|warning" | head -20
```

Expected: no warnings or errors.

- [ ] **Step 3.6 — Commit**

```bash
git add src/shortcuts/capture.rs src/shortcuts/mod.rs
git commit -m "feat(shortcuts): keyboard combo formatter for capture mode"
```

---

## Task 4 — Portal Stream: Bidirectional with `tokio::select!` and `ShortcutsChanged`

**Files:**
- Modify: `src/shortcuts/portal.rs`

The current `shortcut_stream` runs a simple `while let Some(ev) = activated.next()` loop. Replace it with a `tokio::select!` loop that also handles `ShortcutsChanged` and `PortalCommand`.

- [ ] **Step 4.1 — Write failing tests**

Add to the `#[cfg(test)]` block in `src/shortcuts/portal.rs`:

```rust
    #[test]
    fn build_shortcuts_sets_preferred_trigger() {
        let mut desired: [Option<String>; 20] = std::array::from_fn(|_| None);
        desired[0] = Some("Meta+1".into());
        desired[4] = Some("Ctrl+Alt+F".into());
        let shortcuts = build_shortcuts(&desired);
        assert_eq!(shortcuts.len(), 20);
        // We can't inspect preferred_trigger directly (it's private in ashpd),
        // but we can verify the slot IDs are correct.
        // The function must compile and return 20 entries.
    }

    #[test]
    fn parse_changed_binding_extracts_valid_slots() {
        // parse_binding is already tested above. parse_changed_bindings uses the same logic.
        // Just verify the helper exists and compiles by calling it with known data.
        assert_eq!(parse_binding("slot-1", "Meta+1"), Some((0, "Meta+1".to_owned())));
        assert_eq!(parse_binding("slot-20", "Ctrl+F1"), Some((19, "Ctrl+F1".to_owned())));
    }
```

- [ ] **Step 4.2 — Run to verify test compilation fails**

```bash
cargo test -p honkhonk portal -- --nocapture 2>&1 | head -30
```

Expected: error — `build_shortcuts` not defined.

- [ ] **Step 4.3 — Rewrite `portal.rs`**

Replace the full contents of `src/shortcuts/portal.rs`:

```rust
use ashpd::WindowIdentifier;
use iced::futures::{SinkExt, Stream, StreamExt};
use tokio::sync::mpsc;

use super::{PortalCommand, ShortcutEvent};

const SLOT_COUNT: u8 = 20;

/// Returns a stream of shortcut events.
///
/// Yields `ShortcutEvent::Handle` once with the command sender, then
/// `ShortcutEvent::Bindings` with current key assignments, then
/// `ShortcutEvent::Ready` once the portal session is established, then
/// `ShortcutEvent::Activated(idx)` on each trigger press.
/// Yields `ShortcutEvent::Failed(reason)` once on error, then ends.
pub fn shortcut_stream(
    window_id: Option<WindowIdentifier>,
    initial_desired: [Option<String>; 20],
) -> impl Stream<Item = ShortcutEvent> {
    iced::stream::channel(32, async move |mut tx| {
        use ashpd::desktop::global_shortcuts::{
            BindShortcutsOptions, GlobalShortcuts, NewShortcut,
        };
        use ashpd::desktop::CreateSessionOptions;

        macro_rules! bail {
            ($ctx:expr, $err:expr) => {{
                let _ = tx
                    .send(ShortcutEvent::Failed(format!("{}: {}", $ctx, $err)))
                    .await;
                return;
            }};
        }

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<PortalCommand>(8);

        let proxy = match GlobalShortcuts::new().await {
            Ok(p) => p,
            Err(e) => bail!("connecting to portal", e),
        };

        let session = match proxy.create_session(CreateSessionOptions::default()).await {
            Ok(s) => s,
            Err(e) => bail!("creating session", e),
        };

        let mut current_desired = initial_desired;
        let shortcuts = build_shortcuts(&current_desired);

        let req = match proxy
            .bind_shortcuts(
                &session,
                &shortcuts,
                window_id.as_ref(),
                BindShortcutsOptions::default(),
            )
            .await
        {
            Ok(req) => req,
            Err(e) => bail!("binding shortcuts", e),
        };

        let info = match req.response() {
            Ok(info) => info,
            Err(e) => bail!("reading bind response", e),
        };

        let bindings: Vec<(u8, String)> = info
            .shortcuts()
            .iter()
            .filter_map(|s| parse_binding(s.id(), s.trigger_description()))
            .collect();

        let _ = tx.send(ShortcutEvent::Handle(cmd_tx)).await;
        let _ = tx.send(ShortcutEvent::Bindings(bindings)).await;

        let mut activated = match proxy.receive_activated().await {
            Ok(s) => s,
            Err(e) => bail!("subscribing to activations", e),
        };

        let mut changed = match proxy.receive_shortcuts_changed().await {
            Ok(s) => s,
            Err(e) => bail!("subscribing to shortcut changes", e),
        };

        let _ = tx.send(ShortcutEvent::Ready).await;

        tokio::pin!(activated);
        tokio::pin!(changed);

        loop {
            tokio::select! {
                Some(event) = activated.next() => {
                    if let Some(idx) = parse_slot_index(event.shortcut_id()) {
                        if tx.send(ShortcutEvent::Activated(idx)).await.is_err() {
                            break;
                        }
                    }
                }
                Some(changed_event) = changed.next() => {
                    let bindings: Vec<(u8, String)> = changed_event
                        .shortcuts()
                        .iter()
                        .filter_map(|s| parse_binding(s.id(), s.trigger_description()))
                        .collect();
                    if tx.send(ShortcutEvent::Changed(bindings)).await.is_err() {
                        break;
                    }
                }
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        PortalCommand::RebindSlot { idx, trigger } => {
                            current_desired[idx as usize] = Some(trigger);
                            let shortcuts = build_shortcuts(&current_desired);
                            let rebind_result = proxy
                                .bind_shortcuts(
                                    &session,
                                    &shortcuts,
                                    window_id.as_ref(),
                                    BindShortcutsOptions::default(),
                                )
                                .await;
                            let event = match rebind_result {
                                Ok(req) => match req.response() {
                                    Ok(info) => {
                                        let bindings = info
                                            .shortcuts()
                                            .iter()
                                            .filter_map(|s| {
                                                parse_binding(s.id(), s.trigger_description())
                                            })
                                            .collect();
                                        ShortcutEvent::RebindResult {
                                            changed_idx: idx,
                                            bindings,
                                        }
                                    }
                                    Err(e) => ShortcutEvent::Failed(format!(
                                        "rebind response error: {e}"
                                    )),
                                },
                                Err(e) => {
                                    ShortcutEvent::Failed(format!("rebind portal error: {e}"))
                                }
                            };
                            if tx.send(event).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                else => break,
            }
        }
    })
}

/// Builds the full 20-slot shortcut list with preferred_trigger hints.
fn build_shortcuts(desired: &[Option<String>; 20]) -> Vec<NewShortcut> {
    (1..=SLOT_COUNT)
        .map(|n| {
            let idx = (n - 1) as usize;
            NewShortcut::new(format!("slot-{n}"), format!("HonkHonk Slot {n}"))
                .preferred_trigger(desired[idx].as_deref())
        })
        .collect()
}

/// Returns `Some((0-indexed slot, trigger))` for a valid, non-empty binding.
fn parse_binding(id: &str, trigger: &str) -> Option<(u8, String)> {
    if trigger.is_empty() {
        return None;
    }
    let idx = parse_slot_index(id)?;
    Some((idx, trigger.to_owned()))
}

/// Parses "slot-N" → 0-indexed slot index.
fn parse_slot_index(id: &str) -> Option<u8> {
    let n_str = id.strip_prefix("slot-")?;
    let n: u8 = n_str.parse().ok()?;
    if !(1..=SLOT_COUNT).contains(&n) {
        return None;
    }
    Some(n - 1)
}

#[cfg(test)]
mod tests {
    use super::{build_shortcuts, parse_binding, parse_slot_index};

    #[test]
    fn parse_valid_slot_ids() {
        assert_eq!(parse_slot_index("slot-1"), Some(0));
        assert_eq!(parse_slot_index("slot-10"), Some(9));
        assert_eq!(parse_slot_index("slot-20"), Some(19));
    }

    #[test]
    fn parse_invalid_slot_ids() {
        assert_eq!(parse_slot_index("slot-0"), None);
        assert_eq!(parse_slot_index("slot-21"), None);
        assert_eq!(parse_slot_index("f1"), None);
        assert_eq!(parse_slot_index("slot-"), None);
        assert_eq!(parse_slot_index(""), None);
    }

    #[test]
    fn bindings_parse_skips_empty_triggers() {
        assert_eq!(
            parse_binding("slot-1", "Meta+1"),
            Some((0, "Meta+1".to_owned()))
        );
        assert_eq!(
            parse_binding("slot-3", "Ctrl+3"),
            Some((2, "Ctrl+3".to_owned()))
        );
        assert_eq!(parse_binding("slot-1", ""), None);
        assert_eq!(parse_binding("slot-0", "X"), None);
    }

    #[test]
    fn build_shortcuts_returns_20_entries() {
        let desired: [Option<String>; 20] = std::array::from_fn(|_| None);
        let shortcuts = build_shortcuts(&desired);
        assert_eq!(shortcuts.len(), 20);
    }

    #[test]
    fn build_shortcuts_with_some_desired_compiles() {
        let mut desired: [Option<String>; 20] = std::array::from_fn(|_| None);
        desired[0] = Some("Meta+1".into());
        desired[4] = Some("Ctrl+Alt+F".into());
        let shortcuts = build_shortcuts(&desired);
        assert_eq!(shortcuts.len(), 20);
    }
}
```

- [ ] **Step 4.4 — Run tests**

```bash
cargo test -p honkhonk portal -- --nocapture
```

Expected: all portal tests pass.

- [ ] **Step 4.5 — Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep -E "^error|^warning" | head -20
```

Fix any warnings before continuing.

- [ ] **Step 4.6 — Commit**

```bash
git add src/shortcuts/portal.rs
git commit -m "feat(shortcuts): bidirectional portal stream with ShortcutsChanged and rebind"
```

---

## Task 5 — App State, Messages, Subscription, Update Handlers

**Files:**
- Modify: `src/app.rs`

This is the largest task. Work through each sub-step carefully.

- [ ] **Step 5.1 — Add new `Message` variants**

In the `pub enum Message` block, add after `MonitorDeviceChanged`:

```rust
    // Shortcut capture
    StartCapture(u8),
    CancelCapture,
    /// Raw key press — only processed during capture mode.
    KeyPressed {
        key: iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    },
    // Portal handle + rebind results
    ShortcutHandle(tokio::sync::mpsc::Sender<crate::shortcuts::PortalCommand>),
    RebindResult {
        changed_idx: u8,
        bindings: Vec<(u8, String)>,
    },
    ShortcutsChangedExternal(Vec<(u8, String)>),
```

- [ ] **Step 5.2 — Add new fields to `HonkHonk` struct**

In `pub struct HonkHonk`, add after `monitor_devices`:

```rust
    portal_cmd_tx: Option<tokio::sync::mpsc::Sender<crate::shortcuts::PortalCommand>>,
    pub(crate) capturing_slot: Option<u8>,
    pub(crate) bind_feedback: [crate::shortcuts::BindFeedback; 20],
    /// Snapshot of desired_triggers at startup — passed to the portal subscription once.
    /// Never updated after init so the subscription ID stays stable.
    initial_desired_for_sub: std::sync::Arc<[Option<String>; 20]>,
```

- [ ] **Step 5.3 — Initialize new fields in `HonkHonk::new()` / test constructors**

Find the two places where struct fields are initialized (production path and test path, both around line 230-280). In both, add:

```rust
            portal_cmd_tx: None,
            capturing_slot: None,
            bind_feedback: std::array::from_fn(|_| crate::shortcuts::BindFeedback::Unset),
            initial_desired_for_sub: std::sync::Arc::new(config.desired_triggers.clone()),
```

Note: `initial_desired_for_sub` is initialized from `config` before `config` is moved into the struct. Access it as `config.desired_triggers.clone()` before the struct literal, or clone beforehand:

```rust
            let initial_desired = config.desired_triggers.clone();
            // ... then use initial_desired in the struct:
            initial_desired_for_sub: std::sync::Arc::new(initial_desired),
```

- [ ] **Step 5.4 — Update `shortcuts_stream_sub` to accept `initial_desired` and map new events**

Replace the `shortcuts_stream_sub` function:

```rust
fn shortcuts_stream_sub(
    window_id: Option<ashpd::WindowIdentifier>,
    initial_desired: [Option<String>; 20],
) -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    use iced::futures::StreamExt;
    iced::stream::channel(16, async move |mut tx| {
        use crate::shortcuts::{portal, ShortcutEvent};
        let stream = portal::shortcut_stream(window_id, initial_desired);
        let mut stream = std::pin::pin!(stream);
        while let Some(ev) = stream.next().await {
            let msg = match ev {
                ShortcutEvent::Ready => Message::ShortcutsReady,
                ShortcutEvent::Handle(sender) => Message::ShortcutHandle(sender),
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Bindings(b) => Message::ShortcutBindingsUpdated(b),
                ShortcutEvent::RebindResult { changed_idx, bindings } => {
                    Message::RebindResult { changed_idx, bindings }
                }
                ShortcutEvent::Changed(b) => Message::ShortcutsChangedExternal(b),
                ShortcutEvent::Failed(r) => Message::ShortcutsUnavailable(r),
            };
            if tx.send(msg).await.is_err() {
                break;
            }
        }
        let _ = tx
            .send(Message::ShortcutsUnavailable("portal connection lost".into()))
            .await;
        iced::futures::future::pending::<()>().await;
    })
}
```

Replace `shortcuts_stream_sub_none` with a typed wrapper for `run_with`:

```rust
/// Wrapper for `Subscription::run_with` — takes the initial desired triggers Arc.
/// The Arc value is set once at startup and never changes, keeping the subscription ID stable.
fn shortcuts_stream_with_initial(
    initial: &std::sync::Arc<[Option<String>; 20]>,
) -> impl iced::futures::Stream<Item = Message> {
    let initial = (**initial).clone();
    shortcuts_stream_sub(None, initial)
}
```

- [ ] **Step 5.5 — Update `subscription()` to use `run_with` and add keyboard capture**

Replace the `subscription()` method:

```rust
    pub fn subscription(&self) -> Subscription<Message> {
        let shortcuts = Subscription::run_with(
            std::sync::Arc::clone(&self.initial_desired_for_sub),
            shortcuts_stream_with_initial,
        );

        let tray_poll =
            iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayPoll);

        let events = iced::event::listen_with(|event, _, _window_id| match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => Some(Message::CloseContextMenu),
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::CursorMoved(position))
            }
            iced::Event::Window(iced::window::Event::Opened { size, .. }) => {
                Some(Message::WindowResized(size.width, size.height))
            }
            iced::Event::Window(iced::window::Event::Resized(size)) => {
                Some(Message::WindowResized(size.width, size.height))
            }
            _ => None,
        });

        let mut subs = vec![shortcuts, tray_poll, events];

        // Keyboard capture subscription — active only during capture mode.
        // Both this and `events` may fire on Escape; the handlers are both no-ops
        // in the wrong context (no context menu open during capture, CancelCapture
        // is a no-op when capturing_slot is None).
        if self.capturing_slot.is_some() {
            let capture = iced::event::listen_with(|event, _, _| match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    ..
                }) => Some(Message::KeyPressed { key, modifiers }),
                _ => None,
            });
            subs.push(capture);
        }

        if !self.durations_loaded {
            subs.push(Subscription::run_with(
                std::sync::Arc::clone(&self.duration_scan_pairs),
                duration_scan_builder,
            ));
        }

        Subscription::batch(subs)
    }
```

- [ ] **Step 5.6 — Add update handlers for new messages**

In `fn update()`, add the following match arms. Add them after `Message::MonitorDeviceChanged`:

```rust
            Message::ShortcutHandle(sender) => {
                self.portal_cmd_tx = Some(sender);
                Task::none()
            }
            Message::StartCapture(idx) => {
                // Only allow capture on bound slots
                if self.slots.get(idx).is_some() {
                    self.capturing_slot = Some(idx);
                    self.bind_feedback[idx as usize] = crate::shortcuts::BindFeedback::Unset;
                }
                Task::none()
            }
            Message::CancelCapture => {
                self.capturing_slot = None;
                Task::none()
            }
            Message::KeyPressed { key, modifiers } => {
                use iced::keyboard::key::Named;
                use crate::shortcuts::capture::format_combo;

                let Some(idx) = self.capturing_slot else {
                    return Task::none();
                };

                match &key {
                    iced::keyboard::Key::Named(Named::Escape) => {
                        self.capturing_slot = None;
                    }
                    _ => {
                        if let Some(combo) = format_combo(modifiers, &key) {
                            self.capturing_slot = None;
                            self.config.desired_triggers[idx as usize] = Some(combo.clone());
                            if let Err(e) = self.config.save() {
                                eprintln!("honkhonk: config save: {e}");
                            }
                            if let Some(tx) = &self.portal_cmd_tx {
                                let _ = tx.try_send(
                                    crate::shortcuts::PortalCommand::RebindSlot { idx, trigger: combo },
                                );
                            }
                        }
                        // Bare key without modifier: do nothing (capture stays open,
                        // UI shows static hint to add a modifier)
                    }
                }
                Task::none()
            }
            Message::RebindResult { changed_idx, bindings } => {
                // Update all slot_triggers from the full rebind response
                self.slot_triggers = std::array::from_fn(|_| None);
                for (idx, trigger) in &bindings {
                    if let Some(slot) = self.slot_triggers.get_mut(*idx as usize) {
                        *slot = Some(trigger.clone());
                    }
                }
                // Determine feedback for the specifically changed slot
                let requested = self.config.desired_triggers[changed_idx as usize].as_deref();
                let granted = self.slot_triggers[changed_idx as usize].as_deref();
                self.bind_feedback[changed_idx as usize] = match (requested, granted) {
                    (Some(req), Some(got)) if req == got => {
                        crate::shortcuts::BindFeedback::Saved
                    }
                    (Some(_), _) => crate::shortcuts::BindFeedback::NotSaved,
                    _ => crate::shortcuts::BindFeedback::Unset,
                };
                Task::none()
            }
            Message::ShortcutsChangedExternal(bindings) => {
                // Live sync when DE changes shortcuts externally (no feedback update —
                // this reflects the DE's authoritative state)
                for (idx, trigger) in bindings {
                    if let Some(slot) = self.slot_triggers.get_mut(idx as usize) {
                        *slot = Some(trigger);
                    }
                }
                Task::none()
            }
```

- [ ] **Step 5.7 — Update view calls to pass new fields**

Find calls to `view_slot_manager` in `app.rs` (around line 970). The current call is:

```rust
view_slot_manager(&self.slots, &self.slot_triggers, &self.sounds, self.selected_slot, t)
```

Update to:

```rust
view_slot_manager(
    &self.slots,
    &self.slot_triggers,
    &self.sounds,
    self.selected_slot,
    self.capturing_slot,
    &self.bind_feedback,
    t,
)
```

- [ ] **Step 5.8 — Write tests for new message handlers**

Add to the `#[cfg(test)]` block at the bottom of `src/app.rs`:

```rust
    #[test]
    fn start_capture_sets_capturing_slot_for_bound_slot() {
        let mut app = HonkHonk::new_test();
        // Assign a dummy sound path to slot 0 so it's "bound"
        let path = std::path::PathBuf::from("/tmp/test.wav");
        let _ = app.update(Message::AssignSlot(0, path));
        assert!(app.capturing_slot.is_none());
        let _ = app.update(Message::StartCapture(0));
        assert_eq!(app.capturing_slot, Some(0));
    }

    #[test]
    fn start_capture_ignored_for_empty_slot() {
        let mut app = HonkHonk::new_test();
        let _ = app.update(Message::StartCapture(3));
        assert!(app.capturing_slot.is_none());
    }

    #[test]
    fn cancel_capture_clears_capturing_slot() {
        let mut app = HonkHonk::new_test();
        let path = std::path::PathBuf::from("/tmp/test.wav");
        let _ = app.update(Message::AssignSlot(0, path));
        let _ = app.update(Message::StartCapture(0));
        assert!(app.capturing_slot.is_some());
        let _ = app.update(Message::CancelCapture);
        assert!(app.capturing_slot.is_none());
    }

    #[test]
    fn key_pressed_escape_cancels_capture() {
        let mut app = HonkHonk::new_test();
        let path = std::path::PathBuf::from("/tmp/test.wav");
        let _ = app.update(Message::AssignSlot(0, path));
        let _ = app.update(Message::StartCapture(0));
        let _ = app.update(Message::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            modifiers: iced::keyboard::Modifiers::empty(),
        });
        assert!(app.capturing_slot.is_none());
    }

    #[test]
    fn key_pressed_bare_letter_does_not_snap() {
        let mut app = HonkHonk::new_test();
        let path = std::path::PathBuf::from("/tmp/test.wav");
        let _ = app.update(Message::AssignSlot(0, path));
        let _ = app.update(Message::StartCapture(0));
        let _ = app.update(Message::KeyPressed {
            key: iced::keyboard::Key::Character("a".into()),
            modifiers: iced::keyboard::Modifiers::empty(),
        });
        // Capture still active — bare key rejected
        assert_eq!(app.capturing_slot, Some(0));
    }

    #[test]
    fn rebind_result_sets_saved_feedback_when_trigger_matches() {
        let mut app = HonkHonk::new_test();
        app.config.desired_triggers[0] = Some("Meta+1".into());
        let _ = app.update(Message::RebindResult {
            changed_idx: 0,
            bindings: vec![(0, "Meta+1".into())],
        });
        assert_eq!(app.bind_feedback[0], crate::shortcuts::BindFeedback::Saved);
        assert_eq!(app.slot_triggers[0].as_deref(), Some("Meta+1"));
    }

    #[test]
    fn rebind_result_sets_not_saved_when_trigger_absent() {
        let mut app = HonkHonk::new_test();
        app.config.desired_triggers[0] = Some("Meta+1".into());
        let _ = app.update(Message::RebindResult {
            changed_idx: 0,
            bindings: vec![], // portal rejected it — absent from response
        });
        assert_eq!(app.bind_feedback[0], crate::shortcuts::BindFeedback::NotSaved);
    }

    #[test]
    fn shortcuts_changed_external_updates_slot_triggers() {
        let mut app = HonkHonk::new_test();
        let _ = app.update(Message::ShortcutsChangedExternal(vec![(2, "Ctrl+F3".into())]));
        assert_eq!(app.slot_triggers[2].as_deref(), Some("Ctrl+F3"));
    }
```

- [ ] **Step 5.9 — Run tests**

```bash
cargo test -p honkhonk -- --nocapture 2>&1 | tail -20
```

Expected: all tests pass (including new ones).

- [ ] **Step 5.10 — Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep "^error" | head -20
```

Fix any errors.

- [ ] **Step 5.11 — Commit**

```bash
git add src/app.rs
git commit -m "feat(app): shortcut capture state, messages, subscription, update handlers"
```

---

## Task 6 — Slot Manager UI: "Set Hotkey", Capture Overlay, Feedback Badge

**Files:**
- Modify: `src/ui/slot_manager.rs`

- [ ] **Step 6.1 — Update `view_slot_manager` signature**

Change the function signature of `view_slot_manager` to accept the two new parameters:

```rust
pub fn view_slot_manager<'a>(
    slots: &'a SlotMap,
    slot_triggers: &'a [Option<String>; 20],
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    capturing_slot: Option<u8>,
    bind_feedback: &'a [crate::shortcuts::BindFeedback; 20],
    t: Theme,
) -> Element<'a, Message>
```

Pass them through to `sidebar`:

```rust
    let side = sidebar(slots, slot_triggers, sounds, selected_slot, capturing_slot, bind_feedback, t);
```

- [ ] **Step 6.2 — Update `sidebar` function signature**

```rust
fn sidebar<'a>(
    slots: &'a SlotMap,
    slot_triggers: &'a [Option<String>; 20],
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    capturing_slot: Option<u8>,
    bind_feedback: &'a [crate::shortcuts::BindFeedback; 20],
    t: Theme,
) -> Element<'a, Message>
```

In the `Some(idx)` branch, pass the new params:

```rust
        Some(idx) => {
            let sound = slots
                .get(idx)
                .and_then(|p| sounds.iter().find(|s| &s.path == p));
            match sound {
                Some(s) => {
                    let trigger = slot_triggers.get(idx as usize).and_then(|t| t.as_deref());
                    let feedback = bind_feedback.get(idx as usize).copied()
                        .unwrap_or(crate::shortcuts::BindFeedback::Unset);
                    let is_capturing = capturing_slot == Some(idx);
                    if is_capturing {
                        sidebar_capture_mode(idx, s, t)
                    } else {
                        sidebar_bound(idx, s, trigger, feedback, t)
                    }
                }
                None => sidebar_empty(idx, t),
            }
        }
```

- [ ] **Step 6.3 — Update `sidebar_bound` to accept feedback and add "Set Hotkey" button**

Update the signature:

```rust
fn sidebar_bound<'a>(
    idx: u8,
    sound: &'a SoundEntry,
    trigger: Option<&'a str>,
    feedback: crate::shortcuts::BindFeedback,
    t: Theme,
) -> Element<'a, Message>
```

Add a "Set Hotkey" button and feedback badge after `hk_display` in the column:

```rust
    let set_hotkey_btn = button(
        text("Set Hotkey")
            .size(theme::font::LABEL)
            .color(t.ink()),
    )
    .on_press(Message::StartCapture(idx))
    .width(Length::Fill)
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink(),
        border: theme::tile_border(t.hairline(), 1.0),
        ..Default::default()
    });

    // The hotkey display box already shows the current trigger; the badge just
    // communicates the outcome of the last bind attempt without repeating the key.
    let feedback_el: Option<Element<'_, Message>> = match feedback {
        crate::shortcuts::BindFeedback::Saved => Some(
            row![
                status_dot(t.good()),
                text("Saved")
                    .size(theme::font::LABEL)
                    .color(t.good()),
            ]
            .spacing(theme::space::XS)
            .align_y(iced::Alignment::Center)
            .into(),
        ),
        crate::shortcuts::BindFeedback::NotSaved => Some(
            row![
                status_dot(t.accent()),
                text("Not saved — may be in use by another app")
                    .size(theme::font::LABEL)
                    .color(t.accent()),
            ]
            .spacing(theme::space::XS)
            .align_y(iced::Alignment::Center)
            .into(),
        ),
        crate::shortcuts::BindFeedback::Unset => None,
    };
```

Build the column dynamically:

```rust
    let mut col = column![
        slot_label,
        sound_header(sound, t),
        text("GLOBAL HOTKEY")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        hk_display,
        set_hotkey_btn,
    ]
    .spacing(theme::space::MD);

    if let Some(fb) = feedback_el {
        col = col.push(fb);
    }

    col.push(
        text("PORTAL STATUS")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
    )
    .push(portal)
    .push(unbind)
    .into()
```

- [ ] **Step 6.4 — Add `status_dot` helper**

Add before `sidebar_bound`. Do NOT modify the existing `sidebar_bound_portal` — leave it untouched.

```rust
fn status_dot<'a>(color: iced::Color) -> Element<'a, Message> {
    container(Space::new())
        .width(theme::space::SM)
        .height(theme::space::SM)
        .style(move |_t| container::Style {
            background: Some(theme::bg_color(color)),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
```

- [ ] **Step 6.5 — Add `sidebar_capture_mode` function**

```rust
fn sidebar_capture_mode<'a>(idx: u8, sound: &'a SoundEntry, t: Theme) -> Element<'a, Message> {
    let slot_label = text(format!("SLOT #{:02}", idx + 1))
        .size(theme::font::LABEL)
        .color(t.ink_dim());

    let prompt = container(
        column![
            text("Press a key combo…")
                .size(theme::font::BODY)
                .color(t.ink()),
            text("e.g. Meta+1, Ctrl+Alt+F.  F-keys work alone.")
                .size(theme::font::LABEL)
                .color(t.ink_dim()),
        ]
        .spacing(theme::space::XS)
        .padding(theme::space::MD),
    )
    .width(Length::Fill)
    .style(move |_t| container::Style {
        background: Some(theme::bg_color(t.panel())),
        border: iced::Border {
            color: t.accent(),
            width: 1.5,
            radius: 10.0.into(),
        },
        ..Default::default()
    });

    let cancel_btn = button(
        text("Cancel")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
    )
    .on_press(Message::CancelCapture)
    .width(Length::Fill)
    .style(move |_t, _s| button::Style {
        background: None,
        text_color: t.ink_dim(),
        border: theme::tile_border(t.hairline(), 1.0),
        ..Default::default()
    });

    column![
        slot_label,
        sound_header(sound, t),
        text("CAPTURING HOTKEY")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        prompt,
        cancel_btn,
    ]
    .spacing(theme::space::MD)
    .into()
}
```

- [ ] **Step 6.6 — Build to verify compilation**

```bash
cargo build 2>&1 | grep "^error" | head -20
```

Fix any compile errors. Common issues:
- `theme::bg_color` vs `iced::Background::Color` — follow existing patterns in the file
- Missing imports (`Space`, `row!` etc.) — they're already imported at top of file

- [ ] **Step 6.7 — Run all tests**

```bash
cargo test -- --nocapture 2>&1 | tail -15
```

Expected: all tests pass.

- [ ] **Step 6.8 — Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep "^error" | head -20
```

- [ ] **Step 6.9 — Commit**

```bash
git add src/ui/slot_manager.rs
git commit -m "feat(ui): slot capture overlay, Set Hotkey button, feedback badge"
```

---

## Task 7 — Update README and ARCHITECTURE.md

**Files:**
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`

- [ ] **Step 7.1 — Update README status table**

In `README.md`, find the status table and update these rows:

```markdown
| Mic passthrough toggle + level slider UI (audio mixer wiring in future dev) | ✅ Shipped |
| GPU renderer (wgpu default) / software fallback (tiny-skia) | ✅ Shipped |
| Monitor output device selection | ✅ Shipped |
| XDG global shortcuts (20 fixed slots) | ✅ Shipped |
| In-app shortcut assignment with conflict feedback | ✅ Shipped |
| System-persistent virtual mic (survives app restart/reboot) | 🔜 Planned (#49) |
```

Remove the stale `🔜 Planned (#72)` and `🔜 Next (#77)` rows.

- [ ] **Step 7.2 — Update ARCHITECTURE.md Phase 2 checklist**

Find the Phase 2 section and update:

```markdown
### Phase 2: Global Shortcuts — Complete ✅
- ~~ashpd GlobalShortcuts integration~~ ✅
- ~~20 fixed slots, user assigns sounds to slots~~ ✅
- ~~Settings panel for slot management~~ ✅
- ~~In-app shortcut assignment with conflict feedback (#77)~~ ✅
- ~~Monitor output device selection (#72)~~ ✅
- ~~Renderer selection — wgpu vs tiny-skia (#73)~~ ✅
```

- [ ] **Step 7.3 — Build release to confirm no regressions**

```bash
cargo build --release 2>&1 | grep "^error" | head -10
```

Expected: clean build.

- [ ] **Step 7.4 — Run full test suite**

```bash
cargo test -- --nocapture 2>&1 | tail -5
```

Expected: `test result: ok. N passed`.

- [ ] **Step 7.5 — Run clippy + fmt check**

```bash
cargo clippy -- -D warnings && cargo fmt -- --check
```

Both must pass clean.

- [ ] **Step 7.6 — Final commit**

```bash
git add README.md ARCHITECTURE.md
git commit -m "docs: update README and ARCHITECTURE for Phase 2 completion"
```

---

## Manual Smoke Test (before opening PR)

With a real PipeWire + xdg-desktop-portal session running:

1. `cargo run`
2. Open slot manager — click any bound slot
3. Verify "Set Hotkey" button appears in sidebar
4. Click "Set Hotkey" — verify capture overlay appears with prompt text
5. Press `Meta+1` — overlay closes, sidebar shows trigger; check `wpctl` or KDE System Settings for the binding
6. If granted: verify "✓ Saved" badge (green dot)
7. If rejected: verify "⚠ Not saved — may be in use by another app" badge (amber dot)
8. Press Escape during capture — overlay closes, no binding change
9. Change the binding in KDE System Settings → slot manager updates live without restart
10. Restart app → previously saved trigger appears in slot sidebar (from config persistence)

---

## Opening the PR

```bash
git push -u origin feat/in-app-shortcut-assignment
gh pr create \
  --title "feat(shortcuts,ui,state): in-app shortcut assignment with conflict feedback (#77)" \
  --body "$(cat <<'EOF'
## Summary

- Click-to-capture key combo entry in slot manager sidebar
- Portal re-bind via \`BindShortcuts\` with \`preferred_trigger\` hint on the existing session (no dialog re-prompt)
- "Saved" / "Not saved — may be in use by another app" feedback inline after each bind attempt
- Live sync when DE changes bindings externally via \`ShortcutsChanged\` signal
- \`desired_triggers\` persisted in config so hints survive app restart
- Completes Phase 2

## Test plan
- [ ] Set Hotkey button appears on bound slots
- [ ] Capture overlay shows correct prompt
- [ ] Valid combo (Meta+1) exits capture and triggers portal rebind
- [ ] Bare letter without modifier keeps capture open (no snap)
- [ ] Escape cancels capture
- [ ] Saved / NotSaved feedback badge appears after bind
- [ ] ShortcutsChanged: change in KDE System Settings reflects live
- [ ] App restart: desired_triggers loaded, passed as preferred_trigger hints
- [ ] All unit tests pass
- [ ] clippy -D warnings clean
EOF
)"
```
