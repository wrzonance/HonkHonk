# Shortcut Registration Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the broken keyboard-capture → portal-rebind flow with `configure_shortcuts()` — the correct portal API for in-app shortcut assignment — and remove all the dead code the old approach introduced.

**Architecture:** The entire capture UX (keyboard overlay, `StartCapture`/`CancelCapture`/`KeyPressed` messages, `capturing_slot`/`bind_feedback` state, `RebindSlot` portal command) is deleted. The "Set Hotkey" button becomes "Configure Shortcuts" which calls `proxy.configure_shortcuts()` to open the DE's native shortcut dialog. `ShortcutsChanged` already fires on changes from that dialog, so slot displays update live with no extra work.

**Tech Stack:** Rust, Iced 0.13, ashpd 0.13 (`configure_shortcuts` portal v2), tokio mpsc channels.

**Spec:** `docs/superpowers/specs/2026-05-15-shortcut-rebind-fix-design.md`

**Branch:** Work on `feat/in-app-shortcut-assignment` (already checked out — this amends PR #85).

---

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `src/shortcuts/mod.rs` | Modify | Remove `BindFeedback`, `ShortcutEvent::RebindResult`, `pub mod capture`; `PortalCommand::RebindSlot` → `ConfigureShortcuts` |
| `src/shortcuts/capture.rs` | **Delete** | Entire file removed |
| `src/shortcuts/portal.rs` | Modify | Remove `initial_desired` param, `current_desired`, `RebindSlot` handler; add `ConfigureShortcuts` handler; `build_shortcuts()` becomes parameterless |
| `src/state/config.rs` | Modify | Remove `desired_triggers` field + its tests |
| `src/app.rs` | Modify | Remove 4 messages, 3 state fields, capture subscription; add `OpenShortcutConfig`; restore `shortcuts_stream_sub_none` |
| `src/ui/slot_manager.rs` | Modify | Remove `capturing_slot`/`bind_feedback` from ctx; drop `sidebar_capture_mode`, `status_dot`, `sidebar_bound_feedback`; "Set Hotkey" → "Configure Shortcuts" |

---

## Task 1 — `shortcuts/mod.rs`: Remove dead types, delete `capture.rs`

**Files:**
- Modify: `src/shortcuts/mod.rs`
- Delete: `src/shortcuts/capture.rs`

- [ ] **Step 1.1 — Remove `src/shortcuts/capture.rs`**

```bash
rm src/shortcuts/capture.rs
```

- [ ] **Step 1.2 — Rewrite `src/shortcuts/mod.rs`**

Replace the full file contents:

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
    /// The stream's command sender — store this to send commands to the portal.
    Handle(tokio::sync::mpsc::Sender<PortalCommand>),
    Activated(u8),
    /// Initial bindings from BindShortcuts response: (0-indexed slot, trigger string).
    Bindings(Vec<(u8, String)>),
    /// DE changed shortcuts externally (user reconfigured in System Settings).
    Changed(Vec<(u8, String)>),
    Failed(String),
}

/// Commands sent into the running portal stream.
#[derive(Debug, Clone)]
pub enum PortalCommand {
    ConfigureShortcuts,
}

/// Newtype wrapping `tokio::sync::mpsc::Sender<PortalCommand>` so it can be
/// included in `Message`, which derives `PartialEq`. Senders are never
/// meaningfully equal; this impl always returns `false`.
#[derive(Debug, Clone)]
pub struct PortalCmdSender(pub tokio::sync::mpsc::Sender<PortalCommand>);

impl PartialEq for PortalCmdSender {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}
```

- [ ] **Step 1.3 — Verify compilation**

```bash
cargo build 2>&1 | grep "^error" | head -20
```

Expected: errors about `capture::format_combo` usage in `app.rs` (not yet fixed) and `BindFeedback` references. These will be fixed in later tasks. If errors are ONLY from `app.rs` and `slot_manager.rs` referencing the removed types — that's expected.

- [ ] **Step 1.4 — Commit**

```bash
git add src/shortcuts/mod.rs src/shortcuts/capture.rs
git commit -m "refactor(shortcuts): remove BindFeedback, RebindResult, capture module; ConfigureShortcuts cmd"
```

---

## Task 2 — `shortcuts/portal.rs`: Simplify stream

**Files:**
- Modify: `src/shortcuts/portal.rs`

- [ ] **Step 2.1 — Write failing test**

Add to the `#[cfg(test)]` block in `src/shortcuts/portal.rs`, replacing the two `build_shortcuts` tests with one simpler one:

```rust
    #[test]
    fn build_shortcuts_returns_20_entries_no_preferred_trigger() {
        let shortcuts = build_shortcuts();
        assert_eq!(shortcuts.len(), 20);
    }
```

Remove the two existing `build_shortcuts` tests:
- `build_shortcuts_returns_20_entries`
- `build_shortcuts_with_some_desired_compiles`

- [ ] **Step 2.2 — Run to verify it fails**

```bash
cargo test -p honkhonk portal -- --nocapture 2>&1 | head -20
```

Expected: compilation error — `build_shortcuts()` currently takes a `&[Option<String>; 20]` arg.

- [ ] **Step 2.3 — Rewrite `src/shortcuts/portal.rs`**

Replace the full file:

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
pub fn shortcut_stream(window_id: Option<WindowIdentifier>) -> impl Stream<Item = ShortcutEvent> {
    iced::stream::channel(32, async move |mut tx| {
        use ashpd::desktop::global_shortcuts::{
            BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts, NewShortcut,
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

        let shortcuts = build_shortcuts();

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
                        PortalCommand::ConfigureShortcuts => {
                            if let Err(e) = proxy
                                .configure_shortcuts(
                                    &session,
                                    None,
                                    ConfigureShortcutsOptions::default(),
                                )
                                .await
                            {
                                eprintln!("honkhonk: configure_shortcuts unavailable: {e}");
                            }
                        }
                    }
                }
                else => break,
            }
        }
    })
}

/// Builds the full 20-slot shortcut list with no preferred_trigger hints.
fn build_shortcuts() -> Vec<NewShortcut> {
    (1..=SLOT_COUNT)
        .map(|n| NewShortcut::new(format!("slot-{n}"), format!("HonkHonk Slot {n}")))
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
    fn build_shortcuts_returns_20_entries_no_preferred_trigger() {
        let shortcuts = build_shortcuts();
        assert_eq!(shortcuts.len(), 20);
    }
}
```

- [ ] **Step 2.4 — Run portal tests**

```bash
cargo test -p honkhonk portal -- --nocapture
```

Expected: all 4 portal tests pass.

- [ ] **Step 2.5 — Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep "^error" | head -10
```

Expected: errors only in `app.rs` (still references old types — fixed in Task 4).

- [ ] **Step 2.6 — Commit**

```bash
git add src/shortcuts/portal.rs
git commit -m "refactor(shortcuts): simplify portal stream — no rebind, add configure_shortcuts handler"
```

---

## Task 3 — `state/config.rs`: Remove `desired_triggers`

**Files:**
- Modify: `src/state/config.rs`

- [ ] **Step 3.1 — Remove `desired_triggers` field from `AppConfig`**

In `src/state/config.rs`, find and delete the field declaration (around line 85):

```rust
// DELETE this line:
    #[serde(default)]
    pub desired_triggers: [Option<String>; 20],
```

- [ ] **Step 3.2 — Remove from `Default` impl**

In `AppConfig::default()`, delete:

```rust
// DELETE this line:
            desired_triggers: std::array::from_fn(|_| None),
```

- [ ] **Step 3.3 — Update struct-literal tests**

Two existing tests construct `AppConfig` with named fields. Each has `desired_triggers: std::array::from_fn(|_| None)` — delete that line from both:

In `round_trip_serialize_deserialize` (around line 258):
```rust
// DELETE:
            desired_triggers: std::array::from_fn(|_| None),
```

In `save_and_load_from_path` (around line 283):
```rust
// DELETE:
            desired_triggers: std::array::from_fn(|_| None),
```

- [ ] **Step 3.4 — Delete the two `desired_triggers`-specific tests**

Remove these two test functions entirely from the `#[cfg(test)]` block:

```rust
// DELETE the entire fn missing_desired_triggers_field_deserializes_to_empty() { ... }
// DELETE the entire fn desired_triggers_round_trips_json() { ... }
```

- [ ] **Step 3.5 — Run config tests**

```bash
cargo test -p honkhonk config -- --nocapture
```

Expected: all remaining config tests pass (fewer tests than before, no failures).

- [ ] **Step 3.6 — Commit**

```bash
git add src/state/config.rs
git commit -m "refactor(state): remove desired_triggers from AppConfig"
```

---

## Task 4 — `app.rs`: Messages, state, subscription, handlers

**Files:**
- Modify: `src/app.rs`

This is the largest task. Work through each sub-step carefully.

- [ ] **Step 4.1 — Write new tests first (TDD)**

In the `#[cfg(test)]` block at the bottom of `src/app.rs`, add these two tests (before the closing `}`):

```rust
    #[test]
    fn open_shortcut_config_sends_command_when_handle_present() {
        use tokio::sync::mpsc;
        let mut app = HonkHonk::new_for_test();
        let (tx, mut rx) = mpsc::channel(8);
        app.portal_cmd_tx = Some(tx);
        let _ = app.update(Message::OpenShortcutConfig);
        // try_recv succeeds — command was sent
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn open_shortcut_config_is_noop_when_no_handle() {
        let mut app = HonkHonk::new_for_test();
        // portal_cmd_tx is None by default — must not panic
        let _ = app.update(Message::OpenShortcutConfig);
    }
```

- [ ] **Step 4.2 — Run to verify tests fail**

```bash
cargo test -p honkhonk open_shortcut -- --nocapture 2>&1 | head -10
```

Expected: compilation error — `Message::OpenShortcutConfig` not defined.

- [ ] **Step 4.3 — Remove old `Message` variants, add `OpenShortcutConfig`**

Find the `pub enum Message` block. Remove these four variants:

```rust
// DELETE:
    StartCapture(u8),
    CancelCapture,
    /// Raw key press — only processed during capture mode.
    KeyPressed {
        key: iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    },
    RebindResult {
        changed_idx: u8,
        bindings: Vec<(u8, String)>,
    },
```

Add in their place (keep near the shortcut section):

```rust
    /// Opens the DE's native shortcut configuration dialog for this session.
    OpenShortcutConfig,
```

Also remove the `ShortcutsChangedExternal` entry in the stream mapper — wait, keep it. Only remove the `RebindResult` arm.

- [ ] **Step 4.4 — Remove state fields from `HonkHonk` struct**

Find the struct definition and delete these three fields:

```rust
// DELETE:
    pub(crate) capturing_slot: Option<u8>,
    pub(crate) bind_feedback: [crate::shortcuts::BindFeedback; 20],
    /// Snapshot of desired_triggers at startup — passed to the portal subscription once.
    /// Never updated after init so the subscription ID stays stable.
    initial_desired_for_sub: std::sync::Arc<[Option<String>; 20]>,
```

- [ ] **Step 4.5 — Update `shortcuts_stream_sub` and restore zero-arg wrapper**

Replace the existing `shortcuts_stream_sub` and `shortcuts_stream_with_initial` functions with:

```rust
fn shortcuts_stream_sub(
    window_id: Option<ashpd::WindowIdentifier>,
) -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    use iced::futures::StreamExt;
    iced::stream::channel(16, async move |mut tx| {
        use crate::shortcuts::{portal, ShortcutEvent};
        let stream = portal::shortcut_stream(window_id);
        let mut stream = std::pin::pin!(stream);
        while let Some(ev) = stream.next().await {
            let msg = match ev {
                ShortcutEvent::Ready => Message::ShortcutsReady,
                ShortcutEvent::Handle(sender) => {
                    Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender))
                }
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Bindings(b) => Message::ShortcutBindingsUpdated(b),
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

/// Zero-arg wrapper for `Subscription::run`.
fn shortcuts_stream_sub_none() -> impl iced::futures::Stream<Item = Message> {
    shortcuts_stream_sub(None)
}
```

Delete the `shortcuts_stream_with_initial` function if it still exists.

- [ ] **Step 4.6 — Update both constructors**

In `pub fn new(...)` (around line 250): remove these lines:

```rust
// DELETE:
        let initial_desired = config.desired_triggers.clone();
```

And from the struct literal, delete:

```rust
// DELETE:
            capturing_slot: None,
            bind_feedback: std::array::from_fn(|_| crate::shortcuts::BindFeedback::Unset),
            initial_desired_for_sub: std::sync::Arc::new(initial_desired),
```

Do the same for `pub fn new_for_test()` (around line 298).

- [ ] **Step 4.7 — Update `subscription()`**

Replace the shortcuts subscription line:

```rust
// OLD (delete):
        let shortcuts = Subscription::run_with(
            std::sync::Arc::clone(&self.initial_desired_for_sub),
            shortcuts_stream_with_initial,
        );

// NEW:
        let shortcuts = Subscription::run(shortcuts_stream_sub_none);
```

Delete the entire keyboard capture subscription block:

```rust
// DELETE the entire block:
        if self.capturing_slot.is_some() {
            let capture = iced::event::listen_with(|event, _, _| match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) => None,
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) => Some(Message::KeyPressed { key, modifiers }),
                _ => None,
            });
            subs.push(capture);
        }
```

Also delete the comment block above it (the one about Escape double-dispatch).

- [ ] **Step 4.8 — Remove `capturing_slot = None` from navigation handlers**

Find and delete `self.capturing_slot = None;` from these five handlers:
- `Message::CloseContextMenu`
- `Message::ShowSlots`
- `Message::ShowMain`
- `Message::ShowSettings`
- `Message::ShowSettingsSection`
- `Message::SelectSlot`

Each handler had one line added — delete only that line, leaving the rest of the handler unchanged.

- [ ] **Step 4.9 — Remove old update handlers, add `OpenShortcutConfig`**

Delete these four match arms from `fn update()`:

```rust
// DELETE Message::StartCapture handler (the whole arm including braces)
// DELETE Message::CancelCapture handler
// DELETE Message::KeyPressed handler
// DELETE Message::RebindResult handler
```

Add the new handler (place it after `Message::ShortcutsChangedExternal`):

```rust
            Message::OpenShortcutConfig => {
                if let Some(tx) = &self.portal_cmd_tx {
                    let _ = tx.try_send(crate::shortcuts::PortalCommand::ConfigureShortcuts);
                }
                Task::none()
            }
```

- [ ] **Step 4.10 — Update `view_slot_manager` call**

Find the `SlotManagerCtx { ... }` construction (around line 1128). Remove the two extra fields:

```rust
// OLD:
                slot_manager::SlotManagerCtx {
                    slots: &self.slots,
                    slot_triggers: &self.slot_triggers,
                    sounds: &self.sounds,
                    selected_slot: self.selected_slot,
                    capturing_slot: self.capturing_slot,    // DELETE
                    bind_feedback: &self.bind_feedback,     // DELETE
                },

// NEW:
                slot_manager::SlotManagerCtx {
                    slots: &self.slots,
                    slot_triggers: &self.slot_triggers,
                    sounds: &self.sounds,
                    selected_slot: self.selected_slot,
                },
```

- [ ] **Step 4.11 — Delete old tests**

Remove these test functions from the `#[cfg(test)]` block:

- `start_capture_sets_capturing_slot_for_bound_slot`
- `start_capture_ignored_for_empty_slot`
- `cancel_capture_clears_capturing_slot`
- `close_context_menu_cancels_capture`
- `key_pressed_bare_letter_does_not_snap`
- `key_pressed_valid_combo_clears_capture_and_saves_desired`
- `rebind_result_sets_saved_feedback_when_trigger_matches`
- `rebind_result_sets_not_saved_when_trigger_absent`

**Keep:** `shortcuts_changed_external_updates_slot_triggers` and all other existing tests.

- [ ] **Step 4.12 — Run tests**

```bash
cargo test -- --nocapture 2>&1 | grep -E "test result|FAILED" | head -10
```

Expected: all pass (fewer tests than before). Fix any remaining compile errors — they will all be in `slot_manager.rs` (Task 5, not yet done) or stale imports.

- [ ] **Step 4.13 — Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep "^error" | head -10
```

- [ ] **Step 4.14 — Commit**

```bash
git add src/app.rs
git commit -m "refactor(app): remove capture flow, add OpenShortcutConfig, restore simple shortcut subscription"
```

---

## Task 5 — `ui/slot_manager.rs`: Remove capture UI, add Configure Shortcuts button

**Files:**
- Modify: `src/ui/slot_manager.rs`

- [ ] **Step 5.1 — Update `SlotManagerCtx` struct**

Find the `SlotManagerCtx` struct definition (around line 11). Remove `capturing_slot` and `bind_feedback`:

```rust
pub struct SlotManagerCtx<'a> {
    pub slots: &'a SlotMap,
    pub slot_triggers: &'a [Option<String>; 20],
    pub sounds: &'a [SoundEntry],
    pub selected_slot: Option<u8>,
    // DELETE: pub capturing_slot: Option<u8>,
    // DELETE: pub bind_feedback: &'a [crate::shortcuts::BindFeedback; 20],
}
```

- [ ] **Step 5.2 — Delete three functions**

Delete these three functions entirely:
- `fn status_dot<'a>(...)` (around line 322)
- `fn sidebar_bound_feedback<'a>(...)` (around line 337)
- `fn sidebar_capture_mode<'a>(...)` (around line 427)

- [ ] **Step 5.3 — Update `sidebar_bound`**

Change the signature — remove `feedback` parameter:

```rust
fn sidebar_bound<'a>(
    idx: u8,
    sound: &'a SoundEntry,
    trigger: Option<&'a str>,
    t: Theme,
) -> Element<'a, Message>
```

Replace the "Set Hotkey" button and feedback push with the "Configure Shortcuts" button and hint:

Find and replace the `set_hotkey_btn` variable and the `if let Some(fb)` block. The new `sidebar_bound` column assembly:

```rust
    let configure_btn = button(
        text("Configure Shortcuts")
            .size(theme::font::LABEL)
            .color(t.ink()),
    )
    .on_press(Message::OpenShortcutConfig)
    .width(Length::Fill)
    .style(move |_t, _s| button::Style {
        background: Some(theme::bg_color(t.panel())),
        text_color: t.ink(),
        border: theme::tile_border(t.hairline(), 1.0),
        ..Default::default()
    });

    let hint = text("Set keys via the dialog above, or in System Settings → Shortcuts")
        .size(theme::font::LABEL)
        .color(t.ink_faint());

    column![
        slot_label,
        sound_header(sound, t),
        text("GLOBAL HOTKEY")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        hk_display,
        configure_btn,
        hint,
        text("PORTAL STATUS")
            .size(theme::font::LABEL)
            .color(t.ink_dim()),
        portal,
        unbind,
    ]
    .spacing(theme::space::MD)
    .into()
```

- [ ] **Step 5.4 — Update `sidebar` function**

Remove the `feedback` and `is_capturing` logic. The `Some(s)` branch simplifies to:

```rust
                Some(s) => {
                    let trigger = ctx
                        .slot_triggers
                        .get(idx as usize)
                        .and_then(|t| t.as_deref());
                    sidebar_bound(idx, s, trigger, t)
                }
```

Delete these lines from `sidebar`:
```rust
// DELETE:
                    let feedback = ctx
                        .bind_feedback
                        .get(idx as usize)
                        .copied()
                        .unwrap_or(crate::shortcuts::BindFeedback::Unset);
                    let is_capturing = ctx.capturing_slot == Some(idx);
                    if is_capturing {
                        sidebar_capture_mode(idx, s, t)
                    } else {
                        sidebar_bound(idx, s, trigger, feedback, t)
                    }
```

- [ ] **Step 5.5 — Build to verify compilation**

```bash
cargo build 2>&1 | grep "^error" | head -20
```

Expected: clean build.

- [ ] **Step 5.6 — Run all tests**

```bash
cargo test -- --nocapture 2>&1 | grep -E "test result|FAILED"
```

Expected: all pass.

- [ ] **Step 5.7 — Run clippy and fmt**

```bash
cargo clippy -- -D warnings && cargo fmt -- --check
```

Both must pass.

- [ ] **Step 5.8 — Commit**

```bash
git add src/ui/slot_manager.rs
git commit -m "feat(ui): replace capture overlay with Configure Shortcuts button"
```

---

## Task 6 — Final verification and push

- [ ] **Step 6.1 — Full test suite**

```bash
cargo test 2>&1 | grep -E "test result|FAILED"
```

Expected across all test binaries: all `test result: ok`.

- [ ] **Step 6.2 — Release build**

```bash
cargo build --release 2>&1 | grep "^error"
```

Expected: clean.

- [ ] **Step 6.3 — Clippy + fmt**

```bash
cargo clippy -- -D warnings && cargo fmt -- --check
```

Both must exit 0.

- [ ] **Step 6.4 — Verify net removal**

```bash
git diff main...HEAD --stat
```

Expected: significantly more deletions than insertions. `src/shortcuts/capture.rs` shows as deleted.

- [ ] **Step 6.5 — Push**

```bash
git push origin feat/in-app-shortcut-assignment
```

The existing PR #85 will update automatically with the new commits.

- [ ] **Step 6.6 — Open follow-up issue**

```bash
gh issue create \
  --title "feat(shortcuts): persistent shortcut assignments across restarts (ashpd session token fix)" \
  --body "$(cat <<'EOF'
## Problem

HonkHonk uses a random session handle token per launch (ashpd default). KDE stores slot assignments under \`[token_ashpd_XXXXXXXXXX]\` in kglobalshortcutsrc — a new section each launch. Shortcuts configured in System Settings for one session are invisible to the next.

## Root Cause

\`ashpd::desktop::session::CreateSessionOptions::session_handle_token\` is \`pub(crate)\` — no public builder exists. We cannot request a deterministic token.

## Fix Options

1. **Upstream ashpd PR** — add \`pub fn session_handle_token(mut self, t: impl Into<HandleToken>) -> Self\` to \`CreateSessionOptions\`. 4-line change. Submit to ashpd maintainer.
2. **Raw zbus CreateSession** — bypass ashpd for session creation, pass fixed token directly via D-Bus. ~50 lines in portal.rs.

## Impact

Until fixed, users must re-assign shortcuts via "Configure Shortcuts" after every restart.

## References

- Spec: docs/superpowers/specs/2026-05-15-shortcut-rebind-fix-design.md
- kglobalshortcutsrc shows 16 orphaned [token_ashpd_*] sections from repeated launches
EOF
)" \
  --label "enhancement,shortcuts"
```

---

## Self-Review

**Spec coverage:**
- ✅ `configure_shortcuts()` replaces rebind — Task 2, 4, 5
- ✅ `PortalCommand::ConfigureShortcuts` — Task 1, 2
- ✅ `BindFeedback` removed — Task 1
- ✅ `ShortcutEvent::RebindResult` removed — Task 1, 2
- ✅ `capture.rs` deleted — Task 1
- ✅ `desired_triggers` removed from config — Task 3
- ✅ All 8 old tests removed — Task 4
- ✅ 2 new `OpenShortcutConfig` tests — Task 4
- ✅ `sidebar_capture_mode`, `status_dot`, `sidebar_bound_feedback` removed — Task 5
- ✅ "Configure Shortcuts" button + hint text — Task 5
- ✅ `SlotManagerCtx` fields removed — Task 5
- ✅ Follow-up issue for persistence — Task 6
- ✅ `initial_desired_for_sub`, `shortcuts_stream_with_initial` removed — Task 4
- ✅ `capturing_slot = None` removed from navigation handlers — Task 4

**Placeholder scan:** No TBD/TODO. All code blocks complete.

**Type consistency:**
- `Message::OpenShortcutConfig` — defined Task 4.3, tested Task 4.1, handled Task 4.9: consistent
- `PortalCommand::ConfigureShortcuts` — defined Task 1, handled Task 2: consistent
- `shortcut_stream(window_id: Option<WindowIdentifier>)` — defined Task 2, called in Task 4.5: consistent
- `SlotManagerCtx` without `capturing_slot`/`bind_feedback` — defined Task 5.1, constructed Task 4.10: consistent
- `sidebar_bound(idx, sound, trigger, t)` — 4 args in Task 5.3, called Task 5.4: consistent
