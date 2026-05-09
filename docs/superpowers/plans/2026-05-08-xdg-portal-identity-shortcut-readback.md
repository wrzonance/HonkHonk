# XDG Portal Identity & Shortcut Readback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix HonkHonk's XDG GlobalShortcuts portal integration: correct app identity, rename slot labels, and surface bound key strings in the UI.

**Architecture:** Enable ashpd's `raw-window-handle` feature; capture the Wayland window handle on `window::Event::Opened` via `iced::window::run_with_handle`; pass it to `shortcut_stream` so KDE's portal sees the correct app identity. Parse the `ShortcutsInformation` returned by `bind_shortcuts` to read assigned key combos, store them in `slot_triggers: [Option<String>; 20]`, and thread them into the slot manager grid tiles and sidebar.

**Tech Stack:** Rust, Iced 0.14, ashpd 0.13 (`raw-window-handle` feature), raw-window-handle 0.6

---

## File Map

| File | Change |
|------|--------|
| `Cargo.toml` | Add `raw-window-handle` to ashpd features |
| `src/shortcuts/mod.rs` | Add `ShortcutEvent::Bindings(Vec<(u8, String)>)` |
| `src/shortcuts/portal.rs` | Accept `Option<WindowIdentifier>`, fix label, emit `Bindings` |
| `src/app.rs` | Add field + message + handler + window handle flow + subscription update |
| `src/ui/slot_manager.rs` | Thread `slot_triggers` to `bound_tile` + `sidebar_bound_hotkey` |

---

### Task 1: Enable ashpd feature + add Bindings event variant

**Files:**
- Modify: `Cargo.toml:22`
- Modify: `src/shortcuts/mod.rs:14-18`

- [ ] **Step 1: Enable ashpd raw-window-handle feature**

In `Cargo.toml`, change line 22:
```toml
ashpd = { version = "0.13", features = ["global_shortcuts", "raw-window-handle"] }
```

- [ ] **Step 2: Add Bindings variant to ShortcutEvent**

In `src/shortcuts/mod.rs`, replace the `ShortcutEvent` enum:
```rust
#[derive(Debug, Clone)]
pub enum ShortcutEvent {
    Ready,
    Activated(u8),                  // 0-indexed slot (0 = Slot 1)
    Bindings(Vec<(u8, String)>),    // (0-indexed slot, trigger string e.g. "Meta+1")
    Failed(String),
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check 2>&1
```
Expected: no errors (Bindings not yet matched anywhere — that's fine for now).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/shortcuts/mod.rs
git commit -m "feat(shortcuts): enable ashpd raw-window-handle + add Bindings event"
```

---

### Task 2: Fix portal labels, accept WindowIdentifier, parse bindings

**Files:**
- Modify: `src/shortcuts/portal.rs` (full rewrite of `shortcut_stream`)
- Modify: `src/app.rs:93` (update call site to pass `None` — temporary until Task 4)

- [ ] **Step 1: Write the failing test for bindings parsing**

Add this test to `src/shortcuts/portal.rs` at the bottom of the `tests` module:
```rust
#[test]
fn bindings_parse_skips_empty_triggers() {
    // parse_slot_index is tested separately; here we verify the filtering logic
    // A trigger_description of "" means no key assigned — should be excluded
    let cases: &[(&str, &str, Option<(u8, &str)>)] = &[
        ("slot-1", "Meta+1", Some((0, "Meta+1"))),
        ("slot-3", "Ctrl+3", Some((2, "Ctrl+3"))),
        ("slot-1", "",       None),   // empty trigger excluded
        ("slot-0", "X",      None),   // out-of-range id excluded
    ];
    for (id, trigger, expected) in cases {
        let result = parse_binding(id, trigger);
        assert_eq!(result, expected.map(|(i, t)| (i, t.to_owned())), "id={id} trigger={trigger}");
    }
}
```

- [ ] **Step 2: Run test to confirm it fails**

```bash
cargo test parse_binding 2>&1
```
Expected: `error[E0425]: cannot find function 'parse_binding'`

- [ ] **Step 3: Rewrite portal.rs with WindowIdentifier param, fixed labels, Bindings emit**

Replace the full contents of `src/shortcuts/portal.rs`:

```rust
use ashpd::WindowIdentifier;
use iced::futures::{SinkExt, Stream, StreamExt};

use super::ShortcutEvent;

const SLOT_COUNT: u8 = 20;

/// Returns a stream of shortcut events.
///
/// Yields `ShortcutEvent::Bindings` once with current key assignments, then
/// `ShortcutEvent::Ready` once the portal session is established, then
/// `ShortcutEvent::Activated(idx)` (0-indexed) on each trigger.
/// Yields `ShortcutEvent::Failed(reason)` once on error, then ends.
pub fn shortcut_stream(window_id: Option<WindowIdentifier>) -> impl Stream<Item = ShortcutEvent> {
    iced::stream::channel(32, async move |mut tx| {
        use ashpd::desktop::global_shortcuts::{
            BindShortcutsOptions, GlobalShortcuts, NewShortcut,
        };
        use ashpd::desktop::CreateSessionOptions;

        macro_rules! bail {
            ($err:expr) => {{
                let _ = tx.send(ShortcutEvent::Failed($err.to_string())).await;
                return;
            }};
        }

        let proxy = match GlobalShortcuts::new().await {
            Ok(p) => p,
            Err(e) => bail!(e),
        };

        let session = match proxy.create_session(CreateSessionOptions::default()).await {
            Ok(s) => s,
            Err(e) => bail!(e),
        };

        let shortcuts: Vec<NewShortcut> = (1..=SLOT_COUNT)
            .map(|n| NewShortcut::new(format!("slot-{n}"), format!("HonkHonk Slot {n}")))
            .collect();

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
            Err(e) => bail!(e),
        };

        let info = match req.response() {
            Ok(info) => info,
            Err(e) => bail!(e),
        };

        let bindings: Vec<(u8, String)> = info
            .shortcuts()
            .iter()
            .filter_map(|s| parse_binding(s.shortcut_id(), s.trigger_description()))
            .collect();

        let _ = tx.send(ShortcutEvent::Bindings(bindings)).await;

        let mut activated = match proxy.receive_activated().await {
            Ok(s) => s,
            Err(e) => bail!(e),
        };

        let _ = tx.send(ShortcutEvent::Ready).await;

        while let Some(event) = activated.next().await {
            if let Some(idx) = parse_slot_index(event.shortcut_id()) {
                if tx.send(ShortcutEvent::Activated(idx)).await.is_err() {
                    break;
                }
            }
        }
    })
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
///
/// "slot-1" → `Some(0)`, "slot-20" → `Some(19)`, everything else → `None`.
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
    use super::{parse_binding, parse_slot_index};

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
        let cases: &[(&str, &str, Option<(u8, &str)>)] = &[
            ("slot-1", "Meta+1", Some((0, "Meta+1"))),
            ("slot-3", "Ctrl+3", Some((2, "Ctrl+3"))),
            ("slot-1", "",       None),
            ("slot-0", "X",      None),
        ];
        for (id, trigger, expected) in cases {
            let result = parse_binding(id, trigger);
            assert_eq!(
                result,
                expected.map(|(i, t)| (i, t.to_owned())),
                "id={id} trigger={trigger}"
            );
        }
    }
}
```

- [ ] **Step 4: Fix the call site in app.rs to pass None (temporary)**

In `src/app.rs`, line 93, change:
```rust
        let stream = portal::shortcut_stream();
```
to:
```rust
        let stream = portal::shortcut_stream(None);
```

Also add the `Bindings` arm to the match in `shortcuts_stream_sub` (lines 96-99). Change:
```rust
            let msg = match ev {
                ShortcutEvent::Ready => Message::ShortcutsReady,
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Failed(r) => Message::ShortcutsUnavailable(r),
            };
```
to:
```rust
            let msg = match ev {
                ShortcutEvent::Ready => Message::ShortcutsReady,
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Failed(r) => Message::ShortcutsUnavailable(r),
                ShortcutEvent::Bindings(_) => continue,
            };
```
(The `continue` is a placeholder — Task 3 replaces it with the real message.)

- [ ] **Step 5: Run all tests**

```bash
cargo test 2>&1
```
Expected: all existing tests pass + `bindings_parse_skips_empty_triggers` passes.

- [ ] **Step 6: Commit**

```bash
git add src/shortcuts/portal.rs src/shortcuts/mod.rs src/app.rs
git commit -m "feat(shortcuts): HonkHonk Slot labels, WindowIdentifier param, parse Bindings"
```

---

### Task 3: Add slot_triggers state + ShortcutBindingsUpdated message + handler

**Files:**
- Modify: `src/app.rs` (struct, Message enum, new/new_for_test, update, tests)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/app.rs`:
```rust
#[test]
fn shortcut_bindings_updated_stores_triggers() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutBindingsUpdated(vec![
        (0, "Meta+1".into()),
        (4, "Ctrl+5".into()),
    ]));
    assert_eq!(app.slot_triggers()[0], Some("Meta+1"));
    assert_eq!(app.slot_triggers()[4], Some("Ctrl+5"));
    assert!(app.slot_triggers()[1].is_none());
}

#[test]
fn shortcut_bindings_updated_ignores_out_of_range() {
    let mut app = HonkHonk::new_for_test();
    // slot index 20 is out of range — should not panic
    let _ = app.update(Message::ShortcutBindingsUpdated(vec![(20, "X".into())]));
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test shortcut_bindings_updated 2>&1
```
Expected: compile error — `Message::ShortcutBindingsUpdated` and `slot_triggers()` do not exist yet.

- [ ] **Step 3: Add Message variant**

In `src/app.rs`, in the `Message` enum, add after `ShortcutActivated`:
```rust
    ShortcutBindingsUpdated(Vec<(u8, String)>),
```

- [ ] **Step 4: Add slot_triggers field to HonkHonk struct**

In `src/app.rs`, in the `HonkHonk` struct, add after `slots`:
```rust
    slot_triggers: [Option<String>; 20],
```

- [ ] **Step 5: Initialize slot_triggers in new() and new_for_test()**

In `new()`, add after `slots,`:
```rust
            slot_triggers: std::array::from_fn(|_| None),
```

In `new_for_test()`, add after `slots: SlotMap::default(),`:
```rust
            slot_triggers: std::array::from_fn(|_| None),
```

- [ ] **Step 6: Add public accessor**

After `pub fn slots(&self)`, add:
```rust
    pub fn slot_triggers(&self) -> &[Option<String>; 20] {
        &self.slot_triggers
    }
```

- [ ] **Step 7: Add handler in update()**

In `update()`, after the `Message::ShortcutActivated` arm, add:
```rust
            Message::ShortcutBindingsUpdated(bindings) => {
                for (idx, trigger) in bindings {
                    if let Some(slot) = self.slot_triggers.get_mut(idx as usize) {
                        *slot = Some(trigger);
                    }
                }
                Task::none()
            }
```

- [ ] **Step 8: Replace the `continue` placeholder in shortcuts_stream_sub**

In `src/app.rs` `shortcuts_stream_sub`, change:
```rust
                ShortcutEvent::Bindings(_) => continue,
```
to:
```rust
                ShortcutEvent::Bindings(b) => Message::ShortcutBindingsUpdated(b),
```

- [ ] **Step 9: Run all tests**

```bash
cargo test 2>&1
```
Expected: all pass including both new tests.

- [ ] **Step 10: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): slot_triggers state + ShortcutBindingsUpdated message handler"
```

---

### Task 4: Window identifier acquisition + wire to subscription

**Files:**
- Modify: `src/app.rs` (Message enum, struct, new/new_for_test, event listener, update handlers, subscription, shortcuts_stream_sub)

- [ ] **Step 1: Add new Message variants**

In the `Message` enum, add:
```rust
    // Window handle acquisition
    WindowOpened(iced::window::Id, f32, f32),
    WindowIdentifierReady(Option<ashpd::WindowIdentifier>),
```

Remove or keep `WindowResized` — it is still used for resize-only events. Keep it.

- [ ] **Step 2: Add window_identifier field to struct**

In `HonkHonk` struct, add after `window_size`:
```rust
    window_identifier: Option<ashpd::WindowIdentifier>,
```

- [ ] **Step 3: Initialize in new() and new_for_test()**

In both constructors, add after `window_size: (1280.0, 800.0),`:
```rust
            window_identifier: None,
```

- [ ] **Step 4: Update the event listener to capture window Id on open**

In `subscription()`, update the `listen_with` closure. Change the third parameter from `_` to `window_id` and update the `Opened` arm:

```rust
        let events = iced::event::listen_with(|event, _, window_id| match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => Some(Message::CloseContextMenu),
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::CursorMoved(position))
            }
            iced::Event::Window(iced::window::Event::Opened { size, .. }) => {
                Some(Message::WindowOpened(window_id, size.width, size.height))
            }
            iced::Event::Window(iced::window::Event::Resized(size)) => {
                Some(Message::WindowResized(size.width, size.height))
            }
            _ => None,
        });
```

- [ ] **Step 5: Handle WindowOpened in update()**

In `update()`, replace the `Message::WindowResized` arm that currently handles `Opened` (it now only handles `Resized` — no change needed there). Add a new arm:

```rust
            Message::WindowOpened(id, w, h) => {
                self.window_size = (w, h);
                iced::window::run_with_handle(id, |handle| {
                    let wid = ashpd::WindowIdentifier::try_from(handle).ok();
                    Message::WindowIdentifierReady(wid)
                })
            }
            Message::WindowIdentifierReady(wid) => {
                self.window_identifier = wid;
                Task::none()
            }
```

> **Implementation note:** `iced::window::run_with_handle` requires the `iced` crate to expose raw window handles. If this function does not exist in iced 0.14, search for `iced::window::raw_handle` or similar. If no raw handle API is available, replace both arms with:
> ```rust
> Message::WindowOpened(_, w, h) => {
>     self.window_size = (w, h);
>     Task::none()
> }
> Message::WindowIdentifierReady(_) => Task::none(),
> ```
> The window_identifier will stay `None`, meaning shortcuts register with no parent window — functional but KDE may still show "Konsole" in dev.

- [ ] **Step 6: Update shortcuts_stream_sub to accept and pass window_identifier**

Replace the `shortcuts_stream_sub` function:

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
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Failed(r) => Message::ShortcutsUnavailable(r),
                ShortcutEvent::Bindings(b) => Message::ShortcutBindingsUpdated(b),
            };
            if tx.send(msg).await.is_err() {
                break;
            }
        }
        let _ = tx
            .send(Message::ShortcutsUnavailable(
                "portal connection lost".into(),
            ))
            .await;
        iced::futures::future::pending::<()>().await;
    })
}
```

- [ ] **Step 7: Update subscription() to key on window_identifier presence**

In `subscription()`, replace:
```rust
        let shortcuts = Subscription::run(shortcuts_stream_sub);
```
with:
```rust
        // Gate: don't start the portal session until we have the window identifier.
        // Subscription::none() while waiting; starts once with the real identifier.
        let shortcuts = if self.window_identifier.is_some() {
            let wid = self.window_identifier.clone();
            Subscription::run_with(1u8, move |_| shortcuts_stream_sub(wid))
        } else {
            Subscription::none()
        };
```

> **Note:** `ashpd::WindowIdentifier` must implement `Clone`. If it doesn't, wrap in `Arc` and clone the Arc. If `Subscription::run_with` signature differs from expected, check iced 0.14 docs — the key must implement `Hash + 'static`.

- [ ] **Step 8: Add ashpd import at top of app.rs**

Ensure `ashpd` is imported where used. Add to existing imports if not present:
```rust
use ashpd::WindowIdentifier;
```
Or use inline `ashpd::WindowIdentifier` as written above. Either is fine.

- [ ] **Step 9: Compile check**

```bash
cargo check 2>&1
```
Fix any type errors before proceeding.

- [ ] **Step 10: Run all tests**

```bash
cargo test 2>&1
```
Expected: all existing tests pass.

- [ ] **Step 11: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): capture Wayland window identifier on open, gate shortcuts subscription"
```

---

### Task 5: Thread slot_triggers through slot manager UI

**Files:**
- Modify: `src/ui/slot_manager.rs` (signatures + render)
- Modify: `src/app.rs:634` (call site)

- [ ] **Step 1: Update view_slot_manager signature**

In `src/ui/slot_manager.rs`, change the public function signature from:
```rust
pub fn view_slot_manager<'a>(
    slots: &'a SlotMap,
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message>
```
to:
```rust
pub fn view_slot_manager<'a>(
    slots: &'a SlotMap,
    slot_triggers: &'a [Option<String>; 20],
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message>
```

- [ ] **Step 2: Thread slot_triggers down to sidebar()**

In `view_slot_manager`, find the call to `sidebar(...)` and add `slot_triggers`:
```rust
    let sidebar = sidebar(slots, slot_triggers, sounds, selected_slot, t);
```

And find the call to `grid_tile(...)` (inside the grid closure) — the grid iterates over slots and calls `grid_tile(idx, slots, selected_slot, t)`. Update that call to also pass `slot_triggers`.

Locate the grid section of `view_slot_manager` (the column of rows that calls `grid_tile`). Pass `slot_triggers` through.

- [ ] **Step 3: Update grid_tile signature and bound_tile call**

Find `fn grid_tile` in the file (it dispatches to `bound_tile` or `empty_tile`). Update its signature:
```rust
fn grid_tile<'a>(
    idx: u8,
    slots: &'a SlotMap,
    slot_triggers: &'a [Option<String>; 20],
    selected: bool,
    t: Theme,
) -> Element<'a, Message>
```

Inside `grid_tile`, pass the trigger to `bound_tile`:
```rust
        Some(s) => bound_tile(idx, s, slot_triggers[idx as usize].as_deref(), selected, t),
        None => empty_tile(idx, selected, t),
```

- [ ] **Step 4: Update bound_tile signature and render**

Change `bound_tile` signature:
```rust
fn bound_tile<'a>(
    idx: u8,
    sound: &'a SoundEntry,
    trigger: Option<&'a str>,
    selected: bool,
    t: Theme,
) -> Element<'a, Message>
```

Change line 177 (`text("no hotkey")`):
```rust
            text(trigger.unwrap_or("no hotkey")).size(10).color(t.ink_faint()),
```

- [ ] **Step 5: Update sidebar() to accept and pass slot_triggers**

Change `sidebar` signature:
```rust
fn sidebar<'a>(
    slots: &'a SlotMap,
    slot_triggers: &'a [Option<String>; 20],
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    t: Theme,
) -> Element<'a, Message>
```

Inside `sidebar`, pass trigger to `sidebar_bound`:
```rust
                Some(s) => sidebar_bound(idx, s, slot_triggers[idx as usize].as_deref(), t),
```

- [ ] **Step 6: Update sidebar_bound and sidebar_bound_hotkey**

Change `sidebar_bound` signature:
```rust
fn sidebar_bound<'a>(idx: u8, sound: &'a SoundEntry, trigger: Option<&'a str>, t: Theme) -> Element<'a, Message>
```

Inside `sidebar_bound`, change the call to `sidebar_bound_hotkey`:
```rust
    let hk_display = sidebar_bound_hotkey(trigger, t);
```

Change `sidebar_bound_hotkey` signature and render:
```rust
fn sidebar_bound_hotkey<'a>(trigger: Option<&'a str>, t: Theme) -> Element<'a, Message> {
    container(text(trigger.unwrap_or("—")).size(13).color(t.ink()))
        .padding([theme::space::SM, theme::space::MD])
        .width(Length::Fill)
        .style(move |_t| container::Style {
            border: iced::Border {
                color: t.accent(),
                width: 1.5,
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .into()
}
```

- [ ] **Step 7: Update call site in app.rs**

In `src/app.rs`, line ~634, update:
```rust
                slot_manager::view_slot_manager(&self.slots, &self.sounds, self.selected_slot, t)
```
to:
```rust
                slot_manager::view_slot_manager(
                    &self.slots,
                    &self.slot_triggers,
                    &self.sounds,
                    self.selected_slot,
                    t,
                )
```

- [ ] **Step 8: Compile check**

```bash
cargo check 2>&1
```
Fix any remaining errors (mismatched param counts, lifetime issues).

- [ ] **Step 9: Run all tests**

```bash
cargo test 2>&1
```
Expected: all tests pass.

- [ ] **Step 10: Lint**

```bash
cargo clippy -- -D warnings 2>&1
```
Fix any warnings.

- [ ] **Step 11: Commit**

```bash
git add src/ui/slot_manager.rs src/app.rs
git commit -m "feat(ui): display bound shortcut trigger in slot tiles and sidebar"
```

---

## Final Verification

After all tasks complete:

```bash
cargo test 2>&1 && cargo clippy -- -D warnings 2>&1 && cargo build --release 2>&1
```

All three must pass clean. Then smoke-test manually:
1. `cargo run`
2. Open KDE System Settings → Shortcuts → HonkHonk
3. Verify slots appear as "HonkHonk Slot 1" … "HonkHonk Slot 20"
4. Verify app appears as "HonkHonk" (not "Konsole") in the app list
5. Assign a key to Slot 1 in KDE settings
6. Relaunch `cargo run` — open slot manager → Slot 1 tile shows the assigned key
