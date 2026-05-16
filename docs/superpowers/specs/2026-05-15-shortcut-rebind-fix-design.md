# Shortcut Registration Fix — Design Spec

**Branch:** `fix/shortcut-registration` (amends `feat/in-app-shortcut-assignment` / PR #85)
**Date:** 2026-05-15

## Problem

Two root causes make in-app shortcut assignment broken:

### Root Cause 1 (out of scope this PR): Random session token → no persistence

ashpd generates a random `session_handle_token` (`"ashpd_XXXXXXXXXX"`) every `CreateSession` call. KDE stores HonkHonk slots under `[token_ashpd_XXXXXXXXXX]` in `~/.config/kglobalshortcutsrc`. Each launch gets a different section — 16 orphaned sections accumulate, and shortcuts assigned in System Settings for one session token are invisible to the next. This requires a patch to ashpd's private `session_handle_token` field. Tracked as a follow-up issue; out of scope here.

### Root Cause 2 (fixed here): `preferred_trigger` rebind doesn't work

The XDG portal spec states: *"An application can only attempt bind shortcuts of a session once."* Re-calling `BindShortcuts` on an existing session is prohibited. KDE enforces this: when slots already exist in kglobalshortcutsrc with `default=none` (written by the initial `BindShortcuts` with no preferred_trigger), subsequent calls with `preferred_trigger` are silently ignored. `trigger_description` comes back empty. The app marks every key as "not saved". The entire capture → rebind → `RebindResult` → Saved/NotSaved flow is architecturally wrong.

**Evidence:**
```
[token_ashpd_CKwyAmVU0i]
slot-1=Meta+Shift+F1,none,HonkHonk Slot 1   ← real binding (set manually via System Settings)
slot-2=,none,HonkHonk Slot 2                ← all others blocked by default=none tombstone
```

`default=none` is written by the initial `BindShortcuts` (no preferred_trigger) and acts as a tombstone blocking all future `preferred_trigger` hints on re-bind.

## Fix: Replace in-app keyboard capture with `configure_shortcuts()`

`configure_shortcuts()` is the correct XDG portal API for in-app shortcut assignment (portal v2). It opens the DE's native shortcut dialog for the active session. The user assigns keys there. `ShortcutsChanged` fires → `slot_triggers` updates live. Cross-DE: works on KDE, GNOME, Hyprland. If portal v1 (older backends), `configure_shortcuts()` returns an error — log silently, user falls back to System Settings.

The "Set Hotkey" button in the slot sidebar becomes "Configure Shortcuts" (fires `Message::OpenShortcutConfig`).

**Known limitation (not fixed here):** Assignments reset on restart due to the random session token problem. Users must reconfigure after each restart until the ashpd persistence fix lands. This is documented in the UI (see Code Changes → slot_manager.rs).

## Code Changes

### `src/shortcuts/portal.rs`

**`PortalCommand` simplification:**
```rust
// REMOVE:
PortalCommand::RebindSlot { idx: u8, trigger: String }

// REPLACE WITH:
PortalCommand::ConfigureShortcuts
```

**`build_shortcuts` simplification** — drop `preferred_trigger` entirely:
```rust
fn build_shortcuts() -> Vec<NewShortcut> {
    (1..=SLOT_COUNT)
        .map(|n| NewShortcut::new(format!("slot-{n}"), format!("HonkHonk Slot {n}")))
        .collect()
}
```

**Initial `bind_shortcuts` call** — no longer passes `preferred_trigger`-loaded shortcuts. Call `build_shortcuts()` (no args).

**Select loop** — replace `RebindSlot` arm with `ConfigureShortcuts`:
```rust
Some(PortalCommand::ConfigureShortcuts) = cmd_rx.recv() => {
    if let Err(e) = proxy
        .configure_shortcuts(&session, None, ConfigureShortcutsOptions::default())
        .await
    {
        eprintln!("honkhonk: configure_shortcuts unavailable: {e}");
    }
}
```

**Remove entirely:** `initial_desired: [Option<String>; 20]` parameter from `shortcut_stream`, `current_desired` field, rollback logic, entire `RebindSlot` handler with `bind_shortcuts` re-call.

**Restore:** `shortcut_stream` to a single `window_id` parameter (removes the `initial_desired` arg added in PR #85).

### `src/shortcuts/mod.rs`

- `PortalCommand`: `RebindSlot { idx, trigger }` → `ConfigureShortcuts` (no fields)
- Remove: `BindFeedback` enum
- Remove: `ShortcutEvent::RebindResult { changed_idx, bindings }`
- Keep: `ShortcutEvent::Handle`, `ShortcutEvent::Changed`, `ShortcutEvent::Bindings`, `ShortcutEvent::Ready`, `ShortcutEvent::Failed`

### `src/app.rs`

**Remove messages:** `StartCapture(u8)`, `CancelCapture`, `KeyPressed { key, modifiers }`, `RebindResult { changed_idx, bindings }`

**Add message:** `OpenShortcutConfig`

```rust
Message::OpenShortcutConfig => {
    if let Some(tx) = &self.portal_cmd_tx {
        let _ = tx.try_send(crate::shortcuts::PortalCommand::ConfigureShortcuts);
    }
    Task::none()
}
```

**Remove state:** `capturing_slot: Option<u8>`, `bind_feedback: [BindFeedback; 20]`, `initial_desired_for_sub: Arc<[Option<String>; 20]>`

**Remove:** keyboard capture subscription gating (`if self.capturing_slot.is_some()` block in `subscription()`), `shortcuts_stream_with_initial` wrapper function. Restore `shortcuts_stream_sub_none` zero-arg wrapper; `subscription()` reverts to `Subscription::run(shortcuts_stream_sub_none)`.

**Remove:** `self.capturing_slot = None` lines added to `ShowSlots`, `ShowMain`, `ShowSettings`, `ShowSettingsSection`, `SelectSlot` handlers.

**Keep:** `portal_cmd_tx`, `ShortcutHandle` handler, `ShortcutsChangedExternal` handler, `slot_triggers`, `ShortcutBindingsUpdated` handler.

### `src/state/config.rs`

Remove `desired_triggers: [Option<String>; 20]` field — it was added in PR #85 to store preferred_trigger hints for the rebind approach, now dead code.

Update `Default` impl and all struct-literal tests to remove the field.

### `src/shortcuts/capture.rs`

Remove entire file — `format_combo` was only called from the removed `KeyPressed` handler.

Remove `pub mod capture;` declaration from `src/shortcuts/mod.rs`.

### `src/ui/slot_manager.rs`

**`SlotManagerCtx`:** Remove `capturing_slot` and `bind_feedback` fields.

**`sidebar_bound()`:** Replace "Set Hotkey" button with "Configure Shortcuts" button:
```rust
button(text("Configure Shortcuts").size(theme::font::LABEL).color(t.ink()))
    .on_press(Message::OpenShortcutConfig)
    .width(Length::Fill)
    // ... same styling as existing Set Hotkey button
```

Add a small note below the trigger display:
```rust
text("Set keys via the dialog above, or in System Settings → Shortcuts")
    .size(theme::font::LABEL)
    .color(t.ink_faint())
```

**Remove functions:** `sidebar_capture_mode()`, `sidebar_bound_feedback()`, `status_dot()`

**Keep:** all trigger display logic, portal status dot, Unbind button.

## Test Changes

**Remove tests:**
- `start_capture_sets_capturing_slot_for_bound_slot`
- `start_capture_ignored_for_empty_slot`
- `cancel_capture_clears_capturing_slot`
- `close_context_menu_cancels_capture`
- `key_pressed_bare_letter_does_not_snap`
- `key_pressed_valid_combo_clears_capture_and_saves_desired`
- `rebind_result_sets_saved_feedback_when_trigger_matches`
- `rebind_result_sets_not_saved_when_trigger_absent`
- All `desired_triggers` config tests (`missing_desired_triggers_field_deserializes_to_empty`, `desired_triggers_round_trips_json`)
- All `capture.rs` tests (file removed)
- `portal.rs: build_shortcuts_with_some_desired_compiles` (no longer takes desired)

**Add tests:**
- `open_shortcut_config_sends_command_when_handle_present` — `OpenShortcutConfig` message calls `try_send(ConfigureShortcuts)` on stored sender
- `open_shortcut_config_is_noop_when_no_handle` — no panic when `portal_cmd_tx` is `None`
- `portal.rs: build_shortcuts_returns_20_entries` — keep, update to call parameterless `build_shortcuts()`

**Keep:** `shortcuts_changed_external_updates_slot_triggers`, existing portal slot parse tests.

## Net Impact

Significant simplification — this PR removes more code than it adds. PR #85's broken capture/rebind UX is replaced with the correct portal API. Shortcuts are configurable via the native dialog; display updates live via `ShortcutsChanged`.

## Follow-up Issues to Open

- **Shortcut persistence across restarts** — requires patching ashpd to expose `CreateSessionOptions::session_handle_token()` as a public builder (currently `pub(crate)`), or implementing raw zbus `CreateSession` with a fixed token. Track separately.
- **KDE-specific `setShortcut` path** — direct KGlobalAccelD assignment for better UX on KDE once the persistence issue is solved. Track separately.

## Out of Scope

- Session token persistence (see follow-up)
- KDE-direct `KGlobalAccelD.setShortcut()` path
- Conflict detection feedback (native dialog handles it)
- Cleanup of 16 orphaned `[token_ashpd_*]` kglobalshortcutsrc sections
