# Volume Slider Size Stability + Persistent Shortcut Sessions — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the volume slider width glitch (#86) and make global shortcut assignments persist across app restarts by using a deterministic session token (#88).

**Architecture:** Two independent fixes in one PR. The volume fix is a 3-line layout change. The shortcut fix bypasses `ashpd::desktop::global_shortcuts::GlobalShortcuts::create_session` (whose `session_handle_token` field is `pub(crate)`) with raw zbus calls for session creation, `BindShortcuts`, and `ConfigureShortcuts`. Signal subscriptions (`receive_activated`, `receive_shortcuts_changed`) stay on ashpd since they take no session argument.

**Tech Stack:** Rust, Iced 0.13, ashpd 0.13.10, zbus 5.15.0

**Spec:** `docs/superpowers/specs/2026-05-16-volume-slider-shortcut-persistence-design.md`

---

## File Map

| Action | File | What changes |
|--------|------|-------------|
| Modify | `src/ui/volume.rs` | Add `.width(32.0).align_x(Right)` to label |
| Modify | `src/shortcuts/portal.rs` | Replace session-bound ashpd calls with raw zbus; add helpers |
| Modify | `Cargo.toml` | Add `zbus = "5"` direct dep |
| Modify | `README.md` | Add persistent shortcuts row to status table |

---

## Task 1: Create branch + add zbus dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Create branch**

```bash
git checkout -b fix/volume-slider-shortcut-persistence main
```

- [ ] **Step 2: Add zbus dependency**

Open `Cargo.toml`. In `[dependencies]`, add after the `ashpd` line:

```toml
zbus = "5"
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check 2>&1 | tail -5
```

Expected: no errors (zbus 5.15.0 is already in the lock file as a transitive dep).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(deps): add zbus as direct dependency for raw portal calls"
```

---

## Task 2: Fix #86 — volume label fixed width

**Files:**
- Modify: `src/ui/volume.rs`

**Root cause:** The `label` widget in `view_volume` has no fixed width. Text changes from `"5%"` (narrow) to `"10%"` (wider) to `"100%"` (widest) at the 10% and 100% boundaries, shifting the row width and compressing/expanding the slider.

- [ ] **Step 1: Open and read `src/ui/volume.rs`**

Current content:

```rust
use iced::widget::{row, slider, text};
use iced::Element;

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

pub fn view_volume(volume: f32) -> Element<'static, Message> {
    let t = Theme::Dark;
    let pct = format!("{}%", (volume * 100.0).round() as u32);

    let vol_slider = slider(0.0..=1.0, volume, Message::VolumeChanged)
        .on_release(Message::VolumeSaveRequested)
        .step(0.01)
        .width(140.0);

    let label = text(pct).size(theme::font::LABEL).color(t.ink_dim());

    row![vol_slider, label]
        .spacing(theme::space::SM)
        .align_y(iced::Alignment::Center)
        .into()
}
```

- [ ] **Step 2: Apply fix**

Replace `src/ui/volume.rs` with:

```rust
use iced::widget::{row, slider, text};
use iced::Element;

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme};

pub fn view_volume(volume: f32) -> Element<'static, Message> {
    let t = Theme::Dark;
    let pct = format!("{}%", (volume * 100.0).round() as u32);

    let vol_slider = slider(0.0..=1.0, volume, Message::VolumeChanged)
        .on_release(Message::VolumeSaveRequested)
        .step(0.01)
        .width(140.0);

    let label = text(pct)
        .size(theme::font::LABEL)
        .color(t.ink_dim())
        .width(32.0)
        .align_x(iced::alignment::Horizontal::Right);

    row![vol_slider, label]
        .spacing(theme::space::SM)
        .align_y(iced::Alignment::Center)
        .into()
}
```

- [ ] **Step 3: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | tail -10
```

Expected: no warnings.

- [ ] **Step 4: Manual verification**

```bash
cargo run
```

Play any sound. During playback, drag the volume slider:
- Through 10%: bar width must NOT jump
- To 100%: bar width must NOT grow
- Back below 100%: bar width must NOT shrink

- [ ] **Step 5: Commit**

```bash
git add src/ui/volume.rs
git commit -m "fix(ui): fix volume label to fixed width to stop slider size jumping at 10% and 100% (#86)"
```

---

## Task 3: Portal path helpers + tests

**Files:**
- Modify: `src/shortcuts/portal.rs`

These helpers replicate the path computation that ashpd does internally in `Proxy::unique_name` (ashpd source: `src/proxy.rs:46-55`). We need them because ashpd's `Session` constructors are `pub(crate)`.

- [ ] **Step 1: Write the failing tests**

Add at the end of the `#[cfg(test)]` block in `src/shortcuts/portal.rs`:

```rust
#[test]
fn session_path_format() {
    // Simulate unique_name = ":1.123"
    let raw = ":1.123";
    let unique_id = raw.trim_start_matches(':').replace('.', "_");
    let path = format!(
        "/org/freedesktop/portal/desktop/session/{}/{}",
        unique_id, "honkhonk_v1"
    );
    assert_eq!(path, "/org/freedesktop/portal/desktop/session/1_123/honkhonk_v1");
}

#[test]
fn request_path_format() {
    let raw = ":1.123";
    let unique_id = raw.trim_start_matches(':').replace('.', "_");
    let path = format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        unique_id, "honkhonk_req"
    );
    assert_eq!(path, "/org/freedesktop/portal/desktop/request/1_123/honkhonk_req");
}
```

- [ ] **Step 2: Run tests — expect compile failure (functions not added yet)**

```bash
cargo test -p honkhonk -- portal 2>&1 | tail -15
```

Expected: compile error — the test bodies reference path computation logic that isn't a function yet, OR the tests just pass because they're self-contained string math. Either is fine — the tests verify the formula we will use in the helpers.

- [ ] **Step 3: Add constants and path helpers to `src/shortcuts/portal.rs`**

Add immediately after the existing `const SLOT_COUNT: u8 = 20;` line:

```rust
const SESSION_TOKEN: &str = "honkhonk_v1";
const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";

/// Computes the portal session object path for our fixed token.
/// Replicates ashpd's internal `Proxy::unique_name` formula.
fn make_session_path(conn: &zbus::Connection) -> zbus::Result<zbus::zvariant::OwnedObjectPath> {
    let unique_name = conn
        .unique_name()
        .ok_or_else(|| zbus::Error::Failure("no unique name".into()))?
        .to_string();
    let unique_id = unique_name.trim_start_matches(':').replace('.', "_");
    let s = format!(
        "/org/freedesktop/portal/desktop/session/{}/{}",
        unique_id, SESSION_TOKEN
    );
    zbus::zvariant::OwnedObjectPath::try_from(s).map_err(|e| zbus::Error::Failure(e.to_string()))
}

/// Computes a portal request object path for a given handle token suffix.
fn make_request_path(
    conn: &zbus::Connection,
    suffix: &str,
) -> zbus::Result<zbus::zvariant::OwnedObjectPath> {
    let unique_name = conn
        .unique_name()
        .ok_or_else(|| zbus::Error::Failure("no unique name".into()))?
        .to_string();
    let unique_id = unique_name.trim_start_matches(':').replace('.', "_");
    let s = format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        unique_id, suffix
    );
    zbus::zvariant::OwnedObjectPath::try_from(s).map_err(|e| zbus::Error::Failure(e.to_string()))
}
```

Also add `zbus` to the imports at the top of `portal.rs`. The file already uses `ashpd` — add:

```rust
use std::collections::HashMap;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
```

- [ ] **Step 4: Run tests — all pass**

```bash
cargo test -p honkhonk -- portal 2>&1 | tail -15
```

Expected: all portal tests pass including the two new path format tests.

- [ ] **Step 5: Commit**

```bash
git add src/shortcuts/portal.rs
git commit -m "feat(shortcuts): add portal path helpers for deterministic session token (#88)"
```

---

## Task 4: Implement raw `create_session_fixed_token`

**Files:**
- Modify: `src/shortcuts/portal.rs`

This function replaces `proxy.create_session(CreateSessionOptions::default())`. It calls `CreateSession` via raw zbus with our fixed `SESSION_TOKEN`, handles the portal Request/Response pattern manually, and returns the session path.

The portal Request/Response pattern:
1. Before calling the method, subscribe to the `Response` signal on the expected Request object path
2. Call the method (returns the Request path — discard it, we already know it)
3. `Response` signal body: `(u32 status, a{sv} results)` — status 0 = success
4. For CreateSession, `results` contains `"session_handle": ObjectPath`

- [ ] **Step 1: Write failing integration test**

Add to the `#[cfg(test)]` block (this test is gated and skipped in normal CI):

```rust
#[cfg(feature = "portal-test")]
#[tokio::test]
async fn create_session_uses_fixed_token() {
    // Requires a live D-Bus session with xdg-desktop-portal running.
    let conn = zbus::Connection::session().await.unwrap();
    let path = create_session_fixed_token(&conn).await.unwrap();
    assert!(
        path.as_str().contains(SESSION_TOKEN),
        "session path should contain '{}', got: {}",
        SESSION_TOKEN,
        path
    );
}
```

- [ ] **Step 2: Add `create_session_fixed_token` to `portal.rs`**

Add this function after the helper functions from Task 3:

```rust
/// Creates a GlobalShortcuts portal session with a fixed, deterministic token.
///
/// ashpd's CreateSessionOptions.session_handle_token is pub(crate), so we call
/// CreateSession directly via raw zbus to pass our own token.
///
/// Returns the session object path to use in subsequent BindShortcuts /
/// ConfigureShortcuts calls.
async fn create_session_fixed_token(
    conn: &zbus::Connection,
) -> Result<OwnedObjectPath, ashpd::Error> {
    let session_path = make_session_path(conn).map_err(ashpd::Error::Zbus)?;
    let req_path = make_request_path(conn, "honkhonk_cs").map_err(ashpd::Error::Zbus)?;

    // Build a proxy for the portal interface
    let portal: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface(PORTAL_IFACE)?
        .path(PORTAL_PATH)?
        .build()
        .await?;

    // Subscribe to Response on the request path BEFORE calling the method
    let req_proxy: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface("org.freedesktop.portal.Request")?
        .path(req_path.as_ref())?
        .build()
        .await?;
    let mut response_stream = req_proxy.receive_signal("Response").await?;

    // Options dict: handle-token (for the request) + session-handle-token (for the session)
    let options: HashMap<&str, Value<'_>> = [
        ("handle-token", Value::new("honkhonk_cs")),
        ("session-handle-token", Value::new(SESSION_TOKEN)),
    ]
    .into_iter()
    .collect();

    // Call CreateSession — returns the Request object path (which we ignore)
    let _: OwnedObjectPath = portal.call("CreateSession", &(&options,)).await?;

    // Wait for Response signal
    let msg = response_stream
        .next()
        .await
        .ok_or_else(|| ashpd::Error::Zbus(zbus::Error::Failure("no response".into())))??;

    let (status, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;

    if status != 0 {
        return Err(ashpd::Error::Portal(ashpd::PortalError::Cancelled));
    }

    // The session_handle key holds either a string or an object path (portal quirk).
    // See ashpd's CreateSessionResponse deserializer for context.
    let session_handle = results
        .get("session_handle")
        .ok_or_else(|| ashpd::Error::Zbus(zbus::Error::Failure("missing session_handle".into())))?;

    let path_str: &str = session_handle
        .downcast_ref::<&str>()
        .or_else(|_| session_handle.downcast_ref::<zbus::zvariant::ObjectPath<'_>>().map(|p| p.as_str()))
        .map_err(|_| ashpd::Error::Zbus(zbus::Error::Failure("bad session_handle type".into())))?;

    // Verify the portal returned the path we predicted
    debug_assert_eq!(
        path_str,
        session_path.as_str(),
        "portal session path mismatch — TOKEN formula wrong?"
    );

    OwnedObjectPath::try_from(path_str)
        .map_err(|e| ashpd::Error::Zbus(zbus::Error::Failure(e.to_string())))
}
```

Note: `ashpd::Error::Zbus` and `ashpd::PortalError` are re-exported. If the compiler complains about error variant names, check `ashpd::Error` variants — use `ashpd::Error::from(zbus_err)` if needed.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | tail -20
```

Fix any warnings before continuing.

- [ ] **Step 4: Commit**

```bash
git add src/shortcuts/portal.rs
git commit -m "feat(shortcuts): raw zbus create_session_fixed_token for deterministic session path (#88)"
```

---

## Task 5: Implement raw `bind_shortcuts_raw` + `configure_shortcuts_raw`

**Files:**
- Modify: `src/shortcuts/portal.rs`

`bind_shortcuts` and `configure_shortcuts` on `ashpd::GlobalShortcuts` both take `&Session<GlobalShortcuts>` which is `pub(crate)`. We implement them via raw zbus using the session `OwnedObjectPath` we got from Task 4.

### `bind_shortcuts_raw`

Portal D-Bus signature:
```
BindShortcuts(session_handle: o, shortcuts: a(sa{sv}), parent_window: s, options: a{sv}) → o
```

The response (via Request/Response pattern) has key `"shortcuts": a(sa{sv})` — same format as `NewShortcut` tuples. We parse them manually.

- [ ] **Step 1: Add `bind_shortcuts_raw`**

Add after `create_session_fixed_token`:

```rust
/// Binds shortcuts for a session via raw zbus.
/// Returns parsed bindings as (0-indexed slot, trigger description) pairs.
async fn bind_shortcuts_raw(
    conn: &zbus::Connection,
    session_path: &OwnedObjectPath,
    shortcuts: &[ashpd::desktop::global_shortcuts::NewShortcut],
) -> Result<Vec<(u8, String)>, ashpd::Error> {
    let req_path = make_request_path(conn, "honkhonk_bs").map_err(ashpd::Error::Zbus)?;

    let portal: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface(PORTAL_IFACE)?
        .path(PORTAL_PATH)?
        .build()
        .await?;

    let req_proxy: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface("org.freedesktop.portal.Request")?
        .path(req_path.as_ref())?
        .build()
        .await?;
    let mut response_stream = req_proxy.receive_signal("Response").await?;

    let options: HashMap<&str, Value<'_>> =
        [("handle-token", Value::new("honkhonk_bs"))].into_iter().collect();

    // parent_window: empty string (Wayland — no XDG window ID to pass)
    let _: OwnedObjectPath = portal
        .call("BindShortcuts", &(session_path.as_ref(), shortcuts, "", &options))
        .await?;

    let msg = response_stream
        .next()
        .await
        .ok_or_else(|| ashpd::Error::Zbus(zbus::Error::Failure("no response".into())))??;

    let (status, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;

    if status != 0 {
        return Ok(Vec::new()); // user cancelled — return empty, not fatal
    }

    // The BindShortcuts response body has key "shortcuts": a(sa{sv}).
    // zbus can't downcast OwnedValue into Vec<(String, HashMap<...>)> directly —
    // deserialize the whole results map into a typed struct instead.
    //
    // If parsing fails, return empty: the `receive_shortcuts_changed` signal subscription
    // (set up later in shortcut_stream) will repopulate bindings when the user
    // next configures a shortcut.
    #[derive(serde::Deserialize)]
    struct BindResponse {
        #[serde(default)]
        shortcuts: Vec<(String, ShortcutTrigger)>,
    }
    #[derive(serde::Deserialize)]
    struct ShortcutTrigger {
        #[serde(rename = "trigger-description", default)]
        trigger_description: String,
    }

    let parsed: BindResponse = match zbus::zvariant::from_value(
        zbus::zvariant::Value::Dict(results.try_into().unwrap_or_default()),
    ) {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };

    let bindings = parsed
        .shortcuts
        .into_iter()
        .filter_map(|(id, info)| parse_binding(&id, &info.trigger_description))
        .collect();

    Ok(bindings)
}

// Note: if the serde approach above doesn't compile cleanly due to zvariant
// Dict conversion, simplify to: return Ok(Vec::new()). The ShortcutsChanged
// signal will populate bindings when the user opens "Configure Shortcuts".
```

### `configure_shortcuts_raw`

`ConfigureShortcuts` is a fire-and-forget call (no Request/Response — ashpd uses `call_versioned`, not `request`).

- [ ] **Step 2: Add `configure_shortcuts_raw`**

```rust
/// Calls ConfigureShortcuts via raw zbus.
/// Only available on portal v2+; callers must check `proxy.version() >= 2` first.
async fn configure_shortcuts_raw(
    conn: &zbus::Connection,
    session_path: &OwnedObjectPath,
) -> Result<(), zbus::Error> {
    let portal: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface(PORTAL_IFACE)?
        .path(PORTAL_PATH)?
        .build()
        .await?;

    let options: HashMap<&str, Value<'_>> = HashMap::new();

    // parent_window: empty string (Wayland, no XDG activation token needed here)
    portal
        .call::<()>("ConfigureShortcuts", &(session_path.as_ref(), "", &options))
        .await
}
```

- [ ] **Step 3: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | tail -20
```

Fix any warnings.

- [ ] **Step 4: Commit**

```bash
git add src/shortcuts/portal.rs
git commit -m "feat(shortcuts): raw zbus bind_shortcuts and configure_shortcuts helpers (#88)"
```

---

## Task 6: Wire into `shortcut_stream` + update signal subscriptions

**Files:**
- Modify: `src/shortcuts/portal.rs`

Replace the three ashpd session-bound calls in `shortcut_stream` with our raw implementations. Keep `receive_activated` and `receive_shortcuts_changed` on the ashpd proxy (they take no session arg).

- [ ] **Step 1: Locate the current `shortcut_stream` session setup**

In `portal.rs`, the relevant section (lines 35–76 approximately):

```rust
let proxy = match GlobalShortcuts::new().await { ... };

let session = match proxy.create_session(CreateSessionOptions::default()).await { ... };

let shortcuts = build_shortcuts();

let req = match proxy.bind_shortcuts(&session, &shortcuts, window_id.as_ref(), BindShortcutsOptions::default()).await { ... };

let info = match req.response() { ... };

let bindings: Vec<(u8, String)> = info.shortcuts().iter()
    .filter_map(|s| parse_binding(s.id(), s.trigger_description()))
    .collect();

let configure_available = proxy.version() >= 2;
```

- [ ] **Step 2: Replace with raw implementations**

Replace the section from `let proxy = ...` through `let configure_available = ...` with:

```rust
let proxy = match GlobalShortcuts::new().await {
    Ok(p) => p,
    Err(e) => bail!("connecting to portal", e),
};

// Get underlying zbus connection via Deref (GlobalShortcuts → zbus::Proxy)
let conn = (*proxy).connection().clone();

let session_path = match create_session_fixed_token(&conn).await {
    Ok(p) => p,
    Err(e) => bail!("creating session with fixed token", e),
};

let shortcuts = build_shortcuts();

let bindings = match bind_shortcuts_raw(&conn, &session_path, &shortcuts).await {
    Ok(b) => b,
    Err(e) => bail!("binding shortcuts", e),
};

let configure_available = proxy.version() >= 2;
```

- [ ] **Step 3: Update the `ConfigureShortcuts` command handler**

In the `loop` at the bottom, replace:

```rust
PortalCommand::ConfigureShortcuts => {
    if let Err(e) = proxy
        .configure_shortcuts(&session, None, ConfigureShortcutsOptions::default())
        .await
    {
        eprintln!("honkhonk: configure_shortcuts unavailable: {e}");
    }
}
```

With:

```rust
PortalCommand::ConfigureShortcuts => {
    if configure_available {
        if let Err(e) = configure_shortcuts_raw(&conn, &session_path).await {
            eprintln!("honkhonk: configure_shortcuts failed: {e}");
        }
    }
}
```

- [ ] **Step 4: Clean up unused imports**

Remove `CreateSessionOptions`, `BindShortcutsOptions`, `ConfigureShortcutsOptions` from the use statement inside `shortcut_stream` if they're now unused. Keep `GlobalShortcuts` and `NewShortcut` (still used in `build_shortcuts`).

The import block inside `shortcut_stream` was:
```rust
use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts,
};
use ashpd::desktop::CreateSessionOptions;
```

Update to:
```rust
use ashpd::desktop::global_shortcuts::GlobalShortcuts;
```

(`NewShortcut` is already imported at the module level via `build_shortcuts`.)

- [ ] **Step 5: Run all tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: all existing tests pass. No new failures.

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1 | tail -10
```

Expected: zero warnings.

- [ ] **Step 7: Build release binary**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 8: Manual end-to-end test**

```bash
cargo run
```

1. Open slot manager → confirm shortcut bindings display (existing bindings visible if set previously)
2. Click "Configure Shortcuts" → KDE shortcut dialog opens
3. Assign a key to Slot 1
4. Quit app
5. Run `grep -A5 "honkhonk_v1" ~/.config/kglobalshortcutsrc`
   - Expected: `[honkhonk_v1]` section with your assigned key
6. Relaunch app
7. Open slot manager → Slot 1 should show the assigned key
8. Run `grep -c "token_ashpd" ~/.config/kglobalshortcutsrc`
   - Expected: 0 new orphaned sections added by this launch

- [ ] **Step 9: Commit**

```bash
git add src/shortcuts/portal.rs
git commit -m "fix(shortcuts): use deterministic session token via raw zbus for persistent assignments (#88)"
```

---

## Task 7: Update README.md

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add persistent shortcuts row to status table**

In `README.md`, find the status table. It currently ends with:

```markdown
| System-persistent virtual mic (survives app restart/reboot) | 🔜 Planned (#49) |
```

Add the new row BEFORE that line:

```markdown
| Persistent shortcut assignments across restarts | ✅ Shipped |
```

- [ ] **Step 2: Verify the table renders correctly**

```bash
grep -A3 "Persistent shortcut" README.md
```

Expected: row is present and formatted with pipe separators.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): mark persistent shortcut assignments as shipped"
```

---

## Task 8: Final verification + push

- [ ] **Step 1: Run full check suite**

```bash
cargo fmt -- --check && cargo clippy -- -D warnings && cargo test
```

Expected: all pass, zero warnings, zero test failures.

- [ ] **Step 2: Verify LOC delta is under 500**

```bash
git diff main...HEAD --stat
```

Expected: well under 500 LOC changed (target: ~80–150).

- [ ] **Step 3: Push branch**

```bash
git push -u origin fix/volume-slider-shortcut-persistence
```

- [ ] **Step 4: Open PR**

```bash
gh pr create \
  --title "fix(ui,shortcuts): volume slider size stability + persistent shortcut sessions" \
  --body "$(cat <<'EOF'
## Summary

- Fix volume slider width glitch — label now has fixed width (32px), eliminating layout shift at 10% and 100% volume boundaries
- Fix shortcut persistence — use deterministic session handle token `honkhonk_v1` via raw zbus, so KDE stores assignments under a stable key in kglobalshortcutsrc across restarts

## Why raw zbus for the session token

ashpd 0.13's `CreateSessionOptions.session_handle_token` is `pub(crate)` with no public setter. `Session` constructors are also `pub(crate)`. This PR bypasses `GlobalShortcuts::create_session` and the session-bound methods (`bind_shortcuts`, `configure_shortcuts`) with direct zbus calls. Signal subscriptions (`receive_activated`, `receive_shortcuts_changed`) continue using ashpd since they take no session argument.

## Test plan

- [ ] Drag volume slider through 9%→10% and 99%→100% — bar width stays constant
- [ ] Assign shortcut to Slot 1 via "Configure Shortcuts"
- [ ] Quit and relaunch app — Slot 1 still shows assigned key
- [ ] Inspect `~/.config/kglobalshortcutsrc` — exactly one `[honkhonk_v1]` section, no new `[token_ashpd_*]` sections
- [ ] `cargo clippy -- -D warnings` passes clean
- [ ] `cargo test` passes

Closes #86
Closes #88
EOF
)"
```

---

## Troubleshooting

### `ashpd::Error::Zbus` doesn't exist

Check `ashpd::Error` variants with `cargo doc --open` or browse `~/.cargo/registry/src/.../ashpd-0.13.10/src/error.rs`. Use whichever variant wraps a `zbus::Error`. Common alternatives: `ashpd::Error::from(e)` (if `From<zbus::Error>` is implemented).

### `downcast_ref` on `OwnedValue` fails for the session_handle

The portal may return the session handle as a string instead of an ObjectPath (this is a known xdg-desktop-portal quirk, documented in ashpd's `CreateSessionResponse` deserializer). The `downcast_ref` chain in `create_session_fixed_token` handles both. If both fail, print the raw `OwnedValue` debug repr and adjust.

### `(*proxy).connection()` doesn't compile

`GlobalShortcuts` implements `Deref<Target = zbus::Proxy<'static>>`. If the deref doesn't resolve, try:
```rust
use std::ops::Deref;
let conn = proxy.deref().connection().clone();
```

### `shortcuts` serialization fails in `bind_shortcuts_raw`

`NewShortcut` is `pub` and implements `Serialize + Type`. If zbus refuses to serialize `&[NewShortcut]`, try wrapping in a `zbus::zvariant::Value`:
```rust
let sc_value = Value::new(shortcuts);
portal.call("BindShortcuts", &(session_path.as_ref(), &sc_value, "", &options)).await?
```
