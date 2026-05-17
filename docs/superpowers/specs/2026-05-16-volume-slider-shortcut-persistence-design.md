# Design: Volume Slider Size Stability + Persistent Shortcut Sessions

**Date:** 2026-05-16  
**Closes:** #86, #88  
**Branch:** `fix/volume-slider-shortcut-persistence`  
**PR title:** `fix(ui,shortcuts): volume slider size stability + persistent shortcut sessions`

---

## Overview

Two independent fixes shipped as one PR. Both are small (<100 LOC combined) and stay well under the 500 LOC cap.

**Out of scope:** #49 (persistent virtual mic), any Phase 3 features, upstream ashpd PR (separate effort).

---

## Fix 1 — #86: Volume Label Fixed Width

### Root Cause

`view_volume` builds `row![vol_slider, label]` where `label` has no fixed width. The text content changes character count at three breakpoints:

| Range | Text | Width |
|-------|------|-------|
| 0–9% | `"5%"` | narrow |
| 10–99% | `"50%"` | medium |
| 100% | `"100%"` | widest |

Each boundary shifts row width, compressing/expanding the slider despite its `.width(140.0)`.

### Fix

Give the label `iced::Length::Fixed(32.0)` — wide enough for `"100%"` at `theme::font::LABEL` size with breathing room. Right-align the text so the percent sign stays pinned to the right edge.

```rust
// src/ui/volume.rs
let label = text(pct)
    .size(theme::font::LABEL)
    .color(t.ink_dim())
    .width(32.0)
    .align_x(iced::alignment::Horizontal::Right);
```

Slider retains `.width(140.0)`. Row width is now constant across all volume values.

### Files Changed

- `src/ui/volume.rs` — ~3 lines

---

## Fix 2 — #88: Deterministic Session Token via Raw zbus

### Root Cause

`CreateSessionOptions::default()` generates a random `session_handle_token` each launch. KDE stores slot assignments under that token in `~/.config/kglobalshortcutsrc` — a new orphaned `[token_ashpd_XXXXXXXXXX]` section every restart. Shortcuts configured in one session are invisible to the next.

`session_handle_token` is `pub(crate)` in ashpd 0.13 — no public builder to set it.

### Fix

Bypass ashpd's `create_session` for session creation only. Call `org.freedesktop.portal.GlobalShortcuts.CreateSession` directly via zbus with a fixed token `"honkhonk_v1"`. All subsequent calls (`bind_shortcuts`, `configure_shortcuts`, `receive_activated`, `receive_shortcuts_changed`) continue to use ashpd unchanged.

### Token Policy

- Constant: `const SESSION_TOKEN: &str = "honkhonk_v1";`
- Characters: alphanumeric + underscore only (HandleToken spec)
- Bump suffix (`v2`, `v3`, …) only when the slot schema changes and users must re-confirm assignments in their DE

### Implementation

**New helper in `portal.rs`:**

```rust
/// Creates a GlobalShortcuts session with a fixed, deterministic token.
/// Returns the session object path for use with ashpd session APIs.
async fn create_session_with_token(
    conn: &zbus::Connection,
    token: &str,
) -> Result<zbus::zvariant::OwnedObjectPath, ashpd::Error> {
    // Build request handle token (random is fine for the request itself)
    let request_token = format!("honkhonk_req_{}", rand_suffix());

    // D-Bus call: org.freedesktop.portal.GlobalShortcuts.CreateSession
    // Args: options dict with "session-handle-token" => token
    let options: HashMap<&str, zbus::zvariant::Value<'_>> = [
        ("session-handle-token", Value::new(token)),
        ("handle_token", Value::new(request_token.as_str())),
    ]
    .into();

    let portal = zbus::Proxy::new(
        conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.GlobalShortcuts",
    )
    .await?;

    let request_path: OwnedObjectPath = portal
        .call_method("CreateSession", &(options,))
        .await?
        .body()
        .deserialize()?;

    // Wait for the Request signal to get the actual session path
    // (portal uses the Request pattern: CreateSession returns a Request path,
    //  the Response signal on that request carries the Session path)
    let session_path = await_request_response(conn, &request_path).await?;
    Ok(session_path)
}
```

**Construct ashpd Session from path:**

ashpd `Session` is a thin `zbus::Proxy` wrapper. If a public from-path constructor exists, use it to hand the session back to the existing ashpd API surface. If not (implementation discovery required at plan time), implement `bind_shortcuts`, `configure_shortcuts`, and signal subscriptions via raw zbus as well — keeping the same logical flow as the current `shortcut_stream`.

**Cargo.toml:** Add `zbus = "4"` as direct dependency. Already compiled in transitively via ashpd — no new binary weight.

### Files Changed

- `src/shortcuts/portal.rs` — ~50 lines (helper + wiring)
- `Cargo.toml` — 1 line (`zbus = "4"`)

---

## Testing

### #86

- Manual: drag volume slider through 9%→10%, 99%→100% boundaries — bar width must not change
- Visual: no layout reflow visible at any value 0–100%

### #88

- Manual: launch app, assign shortcut to slot 1 via "Configure Shortcuts", quit, relaunch — shortcut persists in slot manager display
- Manual: inspect `~/.config/kglobalshortcutsrc` — exactly one `[honkhonk_v1]` section, no orphaned `[token_ashpd_*]` sections after two launches
- Integration test (gated behind `#[cfg(feature = "portal-test")]`): `create_session_with_token` with token `"honkhonk_v1"` returns `Ok`

---

## PR Details

| Field | Value |
|-------|-------|
| Branch | `fix/volume-slider-shortcut-persistence` |
| Base | `main` |
| Closes | `#86`, `#88` |
| LOC estimate | ~80 lines changed |
| LOC cap | 500 (well within) |

### README Update

Add to status table in `README.md`:

```
| Persistent shortcut assignments across restarts | ✅ Shipped |
```

### PR Description (closing statements)

```
Closes #86
Closes #88
```
