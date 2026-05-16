# Shortcut Registration Fix — Design Spec

**Branch:** `fix/shortcut-registration` (branch from `feat/in-app-shortcut-assignment` since it amends that PR)
**Date:** 2026-05-15

## Problem

Two root causes make in-app shortcut assignment broken:

### Root Cause 1: Random session token → no persistence

ashpd generates a random `session_handle_token` (format: `"ashpd_XXXXXXXXXX"`) on every `CreateSession` call. KDE stores the 20 HonkHonk slots under `[token_ashpd_XXXXXXXXXX]` in `~/.config/kglobalshortcutsrc`. Each HonkHonk launch gets a different section. Shortcuts assigned via System Settings for one session's token are invisible to the next session's new token. Result: 16 orphaned sections in kglobalshortcutsrc, no persistence.

### Root Cause 2: `preferred_trigger` rebind doesn't work

The XDG portal spec states: *"An application can only attempt bind shortcuts of a session once."* Re-calling `BindShortcuts` on an existing session is prohibited. KDE enforces this: when slots already exist in kglobalshortcutsrc (with `default=none` written by the initial `BindShortcuts`), subsequent `BindShortcuts` calls with `preferred_trigger` are silently ignored. KDE returns the existing empty `trigger_description` → the app sees every key as "not saved". The entire capture → rebind → `RebindResult` → Saved/NotSaved flow is architecturally wrong.

**Evidence from kglobalshortcutsrc:**
```
[token_ashpd_CKwyAmVU0i]
_k_friendly_name=token_ashpd_CKwyAmVU0i
slot-1=Meta+Shift+F1,none,HonkHonk Slot 1   ← a real binding exists here (manually set)
slot-2=,none,HonkHonk Slot 2                ← all others empty default=none
```

The `default=none` written by the initial `BindShortcuts` call acts as a tombstone that blocks any new `preferred_trigger` from taking effect.

## Solution

### Fix A: Deterministic session handle token

Use a fixed `session_handle_token` of `"honkhonk_v1"` in `CreateSessionOptions` when calling `create_session`. This makes KDE store slots under `[token_honkhonk_v1]` permanently in `~/.config/kglobalshortcutsrc`. Shortcuts set via KDE System Settings → Shortcuts persist across HonkHonk restarts — present on next launch's `BindShortcuts` response as `trigger_description`. The 16 existing orphaned sections are harmless; leave them.

The token `"honkhonk_v1"` includes a version suffix. If the slot count or naming scheme changes in a future release, bump to `"honkhonk_v2"` — KDE creates a fresh section with no conflicting entries.

**Prerequisite: ashpd patch.** `CreateSessionOptions::session_handle_token` is `pub(crate)` in ashpd 0.13.10 — no public builder exists. This requires a minimal upstream patch:

```rust
// Add to CreateSessionOptions impl in ashpd:
pub fn session_handle_token(mut self, token: impl Into<HandleToken>) -> Self {
    self.session_handle_token = token.into();
    self
}
```

Fork ashpd (or use a local path dep), add this method, update `Cargo.toml`:

```toml
[patch.crates-io]
ashpd = { git = "https://github.com/wrzonance/ashpd", branch = "session-handle-token-builder" }
```

Submit the fix upstream — it is a clearly missing public API. Until merged, pin the fork. Usage in `portal.rs`:

```rust
use ashpd::desktop::session::CreateSessionOptions;
// HandleToken::try_from validates alphanumeric+underscore; "honkhonk_v1" is valid
let session = proxy.create_session(
    CreateSessionOptions::default()
        .session_handle_token(
            ashpd::desktop::HandleToken::try_from("honkhonk_v1")
                .expect("static token is valid")
        )
).await;
```

### Fix B: Replace in-app keyboard capture with `configure_shortcuts()`

`configure_shortcuts()` is the correct XDG portal API for in-app shortcut assignment (portal v2). It opens the DE's native shortcut dialog for the active session. The user assigns keys there. `ShortcutsChanged` fires → `slot_triggers` updates live. This works cross-DE (KDE, GNOME, Hyprland all implement it or degrade gracefully).

The "Set Hotkey" button in the slot sidebar becomes "Configure Shortcuts", firing `Message::OpenShortcutConfig`. If `configure_shortcuts()` returns an error (portal v1 or backend doesn't support it), log and silently ignore — the user can use System Settings directly.

## Code Changes

### `src/shortcuts/portal.rs`

**Session token (requires ashpd patch — see Fix A above):**
```rust
let session = proxy.create_session(
    CreateSessionOptions::default()
        .session_handle_token(
            ashpd::desktop::HandleToken::try_from("honkhonk_v1").unwrap()
        )
).await;
```

**`PortalCommand` simplification:**
```rust
// REMOVE:
PortalCommand::RebindSlot { idx: u8, trigger: String }

// ADD:
PortalCommand::ConfigureShortcuts
```

**`build_shortcuts` simplification** — no longer passes `preferred_trigger`:
```rust
fn build_shortcuts() -> Vec<NewShortcut> {
    (1..=SLOT_COUNT)
        .map(|n| NewShortcut::new(format!("slot-{n}"), format!("HonkHonk Slot {n}")))
        .collect()
}
```

**Select loop** — replace `RebindSlot` arm:
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

**Remove entirely:** `current_desired` field, rollback logic, the entire `RebindSlot` handler with `bind_shortcuts` re-call.

### `src/shortcuts/mod.rs`

- `PortalCommand`: `RebindSlot` → `ConfigureShortcuts` (no fields)
- Remove: `BindFeedback` enum
- Remove: `ShortcutEvent::RebindResult`
- Keep: `ShortcutEvent::Handle`, `ShortcutEvent::Changed`, `ShortcutEvent::Bindings`, `ShortcutEvent::Ready`, `ShortcutEvent::Failed`

### `src/app.rs`

**Remove messages:** `StartCapture(u8)`, `CancelCapture`, `KeyPressed { key, modifiers }`, `RebindResult { changed_idx, bindings }`

**Add message:** `OpenShortcutConfig`

**Remove state:** `capturing_slot: Option<u8>`, `bind_feedback: [BindFeedback; 20]`

**Remove:** keyboard capture subscription gating in `subscription()` (the `if self.capturing_slot.is_some()` block)

**Remove:** `self.capturing_slot = None` lines added to navigation handlers (`ShowSlots`, `ShowMain`, `ShowSettings`, `ShowSettingsSection`, `SelectSlot`)

**`OpenShortcutConfig` handler:**
```rust
Message::OpenShortcutConfig => {
    if let Some(tx) = &self.portal_cmd_tx {
        let _ = tx.try_send(crate::shortcuts::PortalCommand::ConfigureShortcuts);
    }
    Task::none()
}
```

**Remove state also:** `initial_desired_for_sub: Arc<[Option<String>; 20]>` (no longer passed to portal stream), `shortcuts_stream_with_initial` wrapper function. Restore `shortcuts_stream_sub_none` zero-arg wrapper; `subscription()` reverts to `Subscription::run(shortcuts_stream_sub_none)`.

**Keep:** `portal_cmd_tx`, `ShortcutHandle` handler, `ShortcutsChangedExternal` handler, `slot_triggers`, `ShortcutBindingsUpdated` handler.

### `src/state/config.rs`

Remove `desired_triggers: [Option<String>; 20]` field and all associated tests (field has no consumer once `preferred_trigger` hints are dropped).

### `src/shortcuts/capture.rs`

Remove entirely — `format_combo` is only called from the removed `KeyPressed` handler.

Remove `pub mod capture;` declaration from `src/shortcuts/mod.rs`.

### `src/ui/slot_manager.rs`

**`SlotManagerCtx`:** Remove `capturing_slot` and `bind_feedback` fields.

**`sidebar_bound()`:** Replace "Set Hotkey" button (was `StartCapture(idx)`) with "Configure Shortcuts" button (`OpenShortcutConfig`). No feedback badge.

**Remove functions:** `sidebar_capture_mode()`, `sidebar_bound_feedback()`, `status_dot()`

**Keep:** all existing trigger display logic (`slot_triggers`, trigger display box, portal status dot, Unbind button).

## Test Changes

**Remove tests:**
- `start_capture_sets_capturing_slot_for_bound_slot`
- `start_capture_ignored_for_empty_slot`
- `cancel_capture_clears_capturing_slot`
- `key_pressed_escape_cancels_capture` (now `close_context_menu_cancels_capture`)
- `key_pressed_bare_letter_does_not_snap`
- `key_pressed_valid_combo_clears_capture_and_saves_desired`
- `rebind_result_sets_saved_feedback_when_trigger_matches`
- `rebind_result_sets_not_saved_when_trigger_absent`
- `shortcuts_changed_external_updates_slot_triggers` (keep — unrelated to rebind)
- All `desired_triggers` config tests (field removed)
- All `capture.rs` tests (file removed)
- All `portal.rs` `build_shortcuts_with_some_desired` tests

**Add tests:**
- `open_shortcut_config_sends_command_when_handle_present` — verify `OpenShortcutConfig` calls `try_send` via stored sender
- `open_shortcut_config_is_noop_when_no_handle` — no panic when `portal_cmd_tx` is `None`
- `portal.rs: build_shortcuts_returns_20_entries` — keep, update to call parameterless `build_shortcuts()`

## Net Impact

Significant simplification — removing more code than adding. The PR that introduced #77 (PR #85) gets amended: the broken capture/rebind UX is replaced with the correct portal API, and shortcuts now actually persist across restarts.

## Out of Scope

- KDE-specific direct `KGlobalAccelD.setShortcut()` path (Phase 4 or later)
- Cleanup of orphaned `[token_ashpd_*]` kglobalshortcutsrc sections
- Deep-linking `configure_shortcuts()` to a specific slot (portal API doesn't support this)
- Conflict detection feedback (the native dialog handles it)
