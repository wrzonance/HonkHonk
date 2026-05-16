# ShortcutConfigService Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace scattered portal/DE shortcut config fields in `app.rs` with a self-contained `ShortcutConfigService` that detects the desktop environment at startup and dispatches to the correct config UI — portal v2, KDE kcmshell6, GNOME gnome-control-center, or a logged no-op.

**Architecture:** New `src/shortcuts/config_ui.rs` owns `DesktopEnv` detection (via `$XDG_CURRENT_DESKTOP`), portal sender, portal v2 flag, and the `open()` / `can_open()` API. `app.rs` replaces two fields with one `shortcut_config: ShortcutConfigService` and delegates entirely — three update handlers become one-liners.

**Tech Stack:** Rust, `std::process::Command` (process spawn), `tokio::sync::mpsc` (portal channel), `std::env::var`.

**Spec:** `docs/superpowers/specs/2026-05-16-config-ui-shortcut-service-design.md`

**Branch:** `feat/in-app-shortcut-assignment` (already checked out).

---

## File Map

| File | Action | Change |
|------|--------|--------|
| `src/shortcuts/config_ui.rs` | **Create** | `DesktopEnv`, `parse_desktop_env`, `ShortcutConfigService` + unit tests |
| `src/shortcuts/mod.rs` | Modify | Add `pub mod config_ui;` |
| `src/app.rs` | Modify | Replace 2 fields → 1; simplify 3 handlers; update view; update 2 tests |

---

## Task 1 — Create `src/shortcuts/config_ui.rs`

**Files:**
- Create: `src/shortcuts/config_ui.rs`

- [ ] **Step 1.1 — Write the failing tests first**

Create `src/shortcuts/config_ui.rs` with only tests and the function signatures as `todo!()`:

```rust
use crate::shortcuts::PortalCommand;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DesktopEnv {
    Kde,
    Gnome,
    Hyprland,
    Sway,
    Windows,
    MacOs,
    Unknown(String),
}

pub(crate) fn parse_desktop_env(raw: &str) -> DesktopEnv {
    todo!()
}

pub(crate) struct ShortcutConfigService {
    desktop_env: DesktopEnv,
    portal_v2_available: bool,
    portal_cmd_tx: Option<tokio::sync::mpsc::Sender<PortalCommand>>,
}

impl ShortcutConfigService {
    pub(crate) fn new() -> Self {
        todo!()
    }

    pub(crate) fn set_portal_sender(&mut self, tx: tokio::sync::mpsc::Sender<PortalCommand>) {
        todo!()
    }

    pub(crate) fn set_portal_v2_available(&mut self, available: bool) {
        todo!()
    }

    pub(crate) fn can_open(&self) -> bool {
        todo!()
    }

    pub(crate) fn open(&self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kde() {
        assert_eq!(parse_desktop_env("KDE"), DesktopEnv::Kde);
    }

    #[test]
    fn parse_kde_lowercase() {
        assert_eq!(parse_desktop_env("kde"), DesktopEnv::Kde);
    }

    #[test]
    fn parse_gnome_colon_variant() {
        assert_eq!(parse_desktop_env("GNOME:GNOME"), DesktopEnv::Gnome);
    }

    #[test]
    fn parse_hyprland() {
        assert_eq!(parse_desktop_env("Hyprland"), DesktopEnv::Hyprland);
    }

    #[test]
    fn parse_sway() {
        assert_eq!(parse_desktop_env("sway"), DesktopEnv::Sway);
    }

    #[test]
    fn parse_unknown_preserves_value() {
        assert_eq!(
            parse_desktop_env("coolde"),
            DesktopEnv::Unknown("coolde".into())
        );
    }

    #[test]
    fn parse_empty_is_unknown() {
        assert_eq!(parse_desktop_env(""), DesktopEnv::Unknown("".into()));
    }

    #[test]
    fn can_open_kde_no_portal() {
        let mut svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Kde,
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        svc.set_portal_v2_available(false);
        assert!(svc.can_open()); // KDE has kcmshell6 fallback
    }

    #[test]
    fn can_open_gnome_no_portal() {
        let svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Gnome,
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        assert!(svc.can_open());
    }

    #[test]
    fn can_open_hyprland_no_portal() {
        let svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Hyprland,
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        assert!(!svc.can_open()); // no fallback tool
    }

    #[test]
    fn can_open_hyprland_with_portal_v2() {
        let svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Hyprland,
            portal_v2_available: true,
            portal_cmd_tx: None,
        };
        assert!(svc.can_open()); // portal v2 is available regardless of DE
    }

    #[test]
    fn set_portal_v2_available_updates_flag() {
        let mut svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Unknown(String::new()),
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        svc.set_portal_v2_available(true);
        assert!(svc.portal_v2_available);
    }

    #[test]
    fn open_sends_via_portal_when_v2_available() {
        use tokio::sync::mpsc;
        let (tx, mut rx) = mpsc::channel(8);
        let mut svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Unknown(String::new()),
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        svc.set_portal_sender(tx);
        svc.set_portal_v2_available(true);
        svc.open();
        // open() called try_send — command is in the channel
        assert!(rx.try_recv().is_ok());
    }
}
```

- [ ] **Step 1.2 — Run tests to verify they fail**

```bash
cd /home/adam/github/honkhonk && cargo test config_ui -- --nocapture 2>&1 | head -20
```

Expected: panics from `todo!()`.

- [ ] **Step 1.3 — Implement `parse_desktop_env`**

Replace the `todo!()` in `parse_desktop_env`:

```rust
pub(crate) fn parse_desktop_env(raw: &str) -> DesktopEnv {
    let first = raw.split(':').next().unwrap_or("").trim();
    match first.to_lowercase().as_str() {
        "kde" => DesktopEnv::Kde,
        "gnome" => DesktopEnv::Gnome,
        "hyprland" => DesktopEnv::Hyprland,
        "sway" => DesktopEnv::Sway,
        _ => DesktopEnv::Unknown(raw.to_owned()),
    }
}
```

- [ ] **Step 1.4 — Implement `ShortcutConfigService`**

Replace all remaining `todo!()` bodies:

```rust
impl ShortcutConfigService {
    pub(crate) fn new() -> Self {
        #[cfg(target_os = "windows")]
        let desktop_env = DesktopEnv::Windows;

        #[cfg(target_os = "macos")]
        let desktop_env = DesktopEnv::MacOs;

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let desktop_env = std::env::var("XDG_CURRENT_DESKTOP")
            .map(|v| parse_desktop_env(&v))
            .unwrap_or_else(|_| DesktopEnv::Unknown(String::new()));

        Self {
            desktop_env,
            portal_v2_available: false,
            portal_cmd_tx: None,
        }
    }

    pub(crate) fn set_portal_sender(&mut self, tx: tokio::sync::mpsc::Sender<PortalCommand>) {
        self.portal_cmd_tx = Some(tx);
    }

    pub(crate) fn set_portal_v2_available(&mut self, available: bool) {
        self.portal_v2_available = available;
    }

    pub(crate) fn can_open(&self) -> bool {
        self.portal_v2_available
            || matches!(self.desktop_env, DesktopEnv::Kde | DesktopEnv::Gnome)
    }

    pub(crate) fn open(&self) {
        // Portal v2 path — preferred when available
        if self.portal_v2_available {
            if let Some(tx) = &self.portal_cmd_tx {
                if let Err(e) = tx.try_send(PortalCommand::ConfigureShortcuts) {
                    eprintln!("honkhonk: configure_shortcuts command dropped: {e}");
                }
                return;
            }
        }
        // DE-specific fallback
        match &self.desktop_env {
            DesktopEnv::Kde => {
                if let Err(e) = std::process::Command::new("kcmshell6")
                    .arg("kcm_keys")
                    .spawn()
                {
                    eprintln!("honkhonk: failed to open KDE shortcuts: {e}");
                }
            }
            DesktopEnv::Gnome => {
                if let Err(e) = std::process::Command::new("gnome-control-center")
                    .arg("keyboard")
                    .spawn()
                {
                    eprintln!("honkhonk: failed to open GNOME keyboard settings: {e}");
                }
            }
            DesktopEnv::Hyprland => {
                eprintln!(
                    "honkhonk: configure_shortcuts requires portal v2 on Hyprland (not available)"
                );
            }
            DesktopEnv::Sway => {
                eprintln!(
                    "honkhonk: configure_shortcuts requires portal v2 on Sway (not available)"
                );
            }
            DesktopEnv::Windows | DesktopEnv::MacOs => {
                // Stubs — not yet implemented
            }
            DesktopEnv::Unknown(de) => {
                eprintln!(
                    "honkhonk: no shortcut config path for DE '{de}'; \
                     install xdg-desktop-portal v2"
                );
            }
        }
    }
}
```

- [ ] **Step 1.5 — Run tests**

```bash
cd /home/adam/github/honkhonk && cargo test config_ui -- --nocapture
```

Expected: all 12 tests pass.

- [ ] **Step 1.6 — Run clippy**

```bash
cd /home/adam/github/honkhonk && cargo clippy -- -D warnings 2>&1 | grep "^error" | head -10
```

Expected: no errors (only `app.rs` / `slot_manager.rs` compile errors — not yet fixed, coming in Task 3).

Note: `can_open_kde_no_portal` test constructs `ShortcutConfigService` with struct literal. This requires the fields to be accessible from within the module (`#[cfg(test)]` is in the same file, so private fields are accessible). ✓

- [ ] **Step 1.7 — Commit**

```bash
cd /home/adam/github/honkhonk && git add src/shortcuts/config_ui.rs && git commit -m "feat(shortcuts): ShortcutConfigService — DE detection + unified open() dispatch"
```

---

## Task 2 — Register `config_ui` module in `src/shortcuts/mod.rs`

**Files:**
- Modify: `src/shortcuts/mod.rs`

- [ ] **Step 2.1 — Add module declaration**

In `src/shortcuts/mod.rs`, add `pub mod config_ui;` as the first line (before `pub mod portal;`):

```rust
pub mod config_ui;
pub mod portal;
// ... rest unchanged
```

- [ ] **Step 2.2 — Verify the module compiles**

```bash
cd /home/adam/github/honkhonk && cargo build 2>&1 | grep "^error\[" | grep -v "app\.rs\|slot_manager\.rs" | head -10
```

Expected: no errors from `shortcuts/` itself. Errors from `app.rs` and `slot_manager.rs` are expected at this stage (fixed in Task 3).

- [ ] **Step 2.3 — Commit**

```bash
cd /home/adam/github/honkhonk && git add src/shortcuts/mod.rs && git commit -m "feat(shortcuts): register config_ui module"
```

---

## Task 3 — Update `src/app.rs`: replace fields, simplify handlers, update tests

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 3.1 — Replace struct fields**

In `pub struct HonkHonk`, find these two fields (around line 129):

```rust
    pub(crate) portal_cmd_tx: Option<tokio::sync::mpsc::Sender<crate::shortcuts::PortalCommand>>,
    pub(crate) shortcuts_configure_available: bool,
```

Replace with:

```rust
    shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService,
```

- [ ] **Step 3.2 — Update `new()` constructor**

Find (around line 265):

```rust
            portal_cmd_tx: None,
            shortcuts_configure_available: true,
```

Replace with:

```rust
            shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService::new(),
```

- [ ] **Step 3.3 — Update `new_for_test()` constructor**

Find (around line 299):

```rust
            portal_cmd_tx: None,
            shortcuts_configure_available: true,
```

Replace with:

```rust
            shortcut_config: crate::shortcuts::config_ui::ShortcutConfigService::new(),
```

- [ ] **Step 3.4 — Replace the three message handlers**

Find (around line 716):

```rust
            Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender)) => {
                self.portal_cmd_tx = Some(sender);
                Task::none()
            }
            Message::ShortcutsConfigureAvailable(available) => {
                self.shortcuts_configure_available = available;
                Task::none()
            }
            Message::OpenShortcutConfig => {
                if let Some(tx) = &self.portal_cmd_tx {
                    if let Err(e) = tx.try_send(crate::shortcuts::PortalCommand::ConfigureShortcuts)
                    {
                        eprintln!("honkhonk: configure_shortcuts command dropped: {e}");
                    }
                }
                Task::none()
            }
```

Replace with:

```rust
            Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender)) => {
                self.shortcut_config.set_portal_sender(sender);
                Task::none()
            }
            Message::ShortcutsConfigureAvailable(available) => {
                self.shortcut_config.set_portal_v2_available(available);
                Task::none()
            }
            Message::OpenShortcutConfig => {
                self.shortcut_config.open();
                Task::none()
            }
```

- [ ] **Step 3.5 — Update the view call**

Find (around line 1007):

```rust
                        configure_available: self.shortcuts_configure_available,
```

Replace with:

```rust
                        configure_available: self.shortcut_config.can_open(),
```

- [ ] **Step 3.6 — Update tests**

Find `open_shortcut_config_sends_command_when_handle_present` in the test module. Replace:

```rust
    #[test]
    fn open_shortcut_config_sends_command_when_handle_present() {
        use tokio::sync::mpsc;
        let mut app = HonkHonk::new_for_test();
        let (tx, mut rx) = mpsc::channel(8);
        app.portal_cmd_tx = Some(tx);
        let _ = app.update(Message::OpenShortcutConfig);
        assert!(rx.try_recv().is_ok());
    }
```

With:

```rust
    #[test]
    fn open_shortcut_config_sends_command_when_handle_present() {
        use tokio::sync::mpsc;
        let mut app = HonkHonk::new_for_test();
        let (tx, mut rx) = mpsc::channel(8);
        app.shortcut_config.set_portal_sender(tx);
        app.shortcut_config.set_portal_v2_available(true);
        let _ = app.update(Message::OpenShortcutConfig);
        assert!(rx.try_recv().is_ok());
    }
```

`open_shortcut_config_is_noop_when_no_handle` needs no change (it just calls `app.update(Message::OpenShortcutConfig)` and asserts no panic — still valid).

- [ ] **Step 3.7 — Verify full build and tests**

```bash
cd /home/adam/github/honkhonk && cargo build 2>&1 | grep "^error" | head -10
```

Expected: clean build.

```bash
cd /home/adam/github/honkhonk && cargo test -- --nocapture 2>&1 | grep -E "test result|FAILED"
```

Expected: all pass.

- [ ] **Step 3.8 — Run clippy and fmt check**

```bash
cd /home/adam/github/honkhonk && cargo clippy -- -D warnings && cargo fmt -- --check
```

Both must pass.

- [ ] **Step 3.9 — Commit**

```bash
cd /home/adam/github/honkhonk && git add src/app.rs && git commit -m "refactor(app): delegate shortcut config to ShortcutConfigService"
```

---

## Task 4 — Push and update PR #85

- [ ] **Step 4.1 — Push**

```bash
cd /home/adam/github/honkhonk && git push origin feat/in-app-shortcut-assignment
```

- [ ] **Step 4.2 — Verify CI**

```bash
gh pr checks 85 --repo wrzonance/HonkHonk || true
```

Wait for Build, Lint, Test to go green.
