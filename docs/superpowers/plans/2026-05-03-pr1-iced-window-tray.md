# PR 1: Iced Window + Tray With Quit — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a minimal Iced application that renders an empty window on Wayland with a system tray icon (Show/Hide + Quit menu).

**Architecture:** Single-binary Rust app. `main.rs` selects renderer and launches both tray-icon (on main thread before event loop) and Iced application. Tray events flow to Iced via `std::sync::mpsc` channel polled by an Iced `Subscription`. Window visibility toggled via Iced's `window` commands.

**Tech Stack:** Rust 1.75+, Iced 0.13 (features: tokio, tiny-skia), tray-icon 0.19, muda 0.15, tokio 1

**UI Vision:** This PR uses `Theme::Dark` as default but does NOT implement the full Confetti theme system yet. That's PR 6 (sound grid). However, the window background MUST use the Confetti dark bg color (`#171410`) to validate that future theming has a home. The `theme.rs` file from `docs/design-reference/src-rust/ui/` will be integrated in the grid PR, not here — keep this PR minimal.

---

## File Structure

```
honkhonk/
├── Cargo.toml              # Workspace root, dependencies
├── clippy.toml             # Strict lint thresholds
├── rustfmt.toml            # Format config
├── src/
│   ├── main.rs             # Entry: renderer selection, tray init, app launch
│   ├── app.rs              # Iced Application: state, update, view, subscription
│   └── tray/
│       ├── mod.rs          # pub use re-exports
│       └── icon.rs         # Tray setup, menu creation, event channel
└── tests/
    └── app_test.rs         # Unit tests for app state transitions
```

---

## Task 1: Project Scaffold (Cargo.toml + clippy.toml + rustfmt.toml)

**Files:**
- Create: `Cargo.toml`
- Create: `clippy.toml`
- Create: `rustfmt.toml`
- Create: `src/main.rs` (placeholder)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "honkhonk"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT"
description = "Wayland-native Linux soundboard"
repository = "https://github.com/thewrz/HonkHonk"

[dependencies]
iced = { version = "0.13", features = ["tokio", "tiny-skia"] }
tray-icon = "0.19"
muda = "0.15"
tokio = { version = "1", features = ["full"] }

[features]
default = []

[profile.release]
lto = true
strip = true
```

- [ ] **Step 2: Create clippy.toml**

```toml
cognitive-complexity-threshold = 10
too-many-arguments-threshold = 5
too-many-lines-threshold = 50
type-complexity-threshold = 200
```

- [ ] **Step 3: Create rustfmt.toml**

```toml
edition = "2021"
max_width = 100
```

- [ ] **Step 4: Create placeholder main.rs**

```rust
fn main() {
    println!("HonkHonk");
}
```

- [ ] **Step 5: Verify build compiles**

Run: `cargo build`
Expected: Compiles successfully (dependencies download, binary links)

- [ ] **Step 6: Verify clippy passes**

Run: `cargo clippy -- -D warnings`
Expected: No warnings or errors

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml clippy.toml rustfmt.toml src/main.rs
git commit -m "chore: initialize cargo project with dependencies"
```

---

## Task 2: Tray Module — Icon + Menu + Event Channel

**Files:**
- Create: `src/tray/mod.rs`
- Create: `src/tray/icon.rs`

- [ ] **Step 1: Create tray module re-export**

Create `src/tray/mod.rs`:

```rust
mod icon;

pub use icon::{build_tray, TrayEvent};
```

- [ ] **Step 2: Write tray/icon.rs with TrayEvent enum and build_tray function**

Create `src/tray/icon.rs`:

```rust
use muda::{Menu, MenuItem, PredefinedMenuItem};
use std::sync::mpsc::{self, Receiver, Sender};
use tray_icon::{TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    ToggleVisibility,
    Quit,
}

pub struct TrayHandle {
    pub event_rx: Receiver<TrayEvent>,
    _icon: TrayIcon,
    show_hide_id: muda::MenuId,
    quit_id: muda::MenuId,
}

pub fn build_tray() -> Result<TrayHandle, Box<dyn std::error::Error>> {
    let (event_tx, event_rx) = mpsc::channel::<TrayEvent>();

    let menu = Menu::new();
    let show_hide = MenuItem::new("Show/Hide", true, None);
    let quit = MenuItem::new("Quit", true, None);

    menu.append(&show_hide)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    let show_hide_id = show_hide.id().clone();
    let quit_id = quit.id().clone();

    let icon = load_icon();
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("HonkHonk")
        .with_icon(icon)
        .build()?;

    let tx = event_tx.clone();
    let sh_id = show_hide_id.clone();
    let q_id = quit_id.clone();

    muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        if event.id() == &sh_id {
            let _ = tx.send(TrayEvent::ToggleVisibility);
        } else if event.id() == &q_id {
            let _ = tx.send(TrayEvent::Quit);
        }
    }));

    Ok(TrayHandle {
        event_rx,
        _icon: tray,
        show_hide_id,
        quit_id,
    })
}

fn load_icon() -> tray_icon::Icon {
    let rgba = vec![96u8; 64 * 64 * 4];
    tray_icon::Icon::from_rgba(rgba, 64, 64).expect("valid icon dimensions")
}
```

- [ ] **Step 3: Verify build compiles with tray module**

Update `src/main.rs` temporarily:

```rust
mod tray;

fn main() {
    println!("HonkHonk - tray module compiled");
}
```

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 4: Verify clippy passes**

Run: `cargo clippy -- -D warnings`
Expected: No warnings (fix any dead_code warnings with `#[allow(dead_code)]` on TrayHandle fields that are intentionally held for lifetime, or prefix with underscore as already done for `_icon`)

- [ ] **Step 5: Commit**

```bash
git add src/tray/
git commit -m "feat(tray): add tray icon with show/hide and quit menu"
```

---

## Task 3: App Module — Iced Application State + Update

**Files:**
- Create: `src/app.rs`
- Create: `tests/app_test.rs`

- [ ] **Step 1: Write failing test for app update logic**

Create `tests/app_test.rs`:

```rust
use honkhonk::app::{HonkHonk, Message};
use iced::Task;

#[test]
fn quit_message_sets_should_exit() {
    let mut app = HonkHonk::new_for_test();
    let _task = app.update(Message::Quit);
    assert!(app.should_exit());
}

#[test]
fn toggle_visibility_flips_visible_state() {
    let mut app = HonkHonk::new_for_test();
    assert!(app.is_visible());
    let _task = app.update(Message::ToggleVisibility);
    assert!(!app.is_visible());
    let _task = app.update(Message::ToggleVisibility);
    assert!(app.is_visible());
}

#[test]
fn tray_event_quit_maps_to_quit_message() {
    let msg = Message::from_tray_event(honkhonk::tray::TrayEvent::Quit);
    assert_eq!(msg, Message::Quit);
}

#[test]
fn tray_event_toggle_maps_to_toggle_message() {
    let msg = Message::from_tray_event(honkhonk::tray::TrayEvent::ToggleVisibility);
    assert_eq!(msg, Message::ToggleVisibility);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test`
Expected: FAIL — `honkhonk::app` module doesn't exist yet

- [ ] **Step 3: Write app.rs with state and update logic**

Create `src/app.rs`:

```rust
use crate::tray::TrayEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    ToggleVisibility,
    Quit,
    TrayEvent(TrayEvent),
}

impl Message {
    pub fn from_tray_event(event: TrayEvent) -> Self {
        match event {
            TrayEvent::ToggleVisibility => Message::ToggleVisibility,
            TrayEvent::Quit => Message::Quit,
        }
    }
}

pub struct HonkHonk {
    visible: bool,
    exit: bool,
}

impl HonkHonk {
    pub fn new_for_test() -> Self {
        Self {
            visible: true,
            exit: false,
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::ToggleVisibility => {
                self.visible = !self.visible;
                iced::Task::none()
            }
            Message::Quit => {
                self.exit = true;
                iced::Task::none()
            }
            Message::TrayEvent(event) => {
                let msg = Message::from_tray_event(event);
                self.update(msg)
            }
        }
    }
}
```

- [ ] **Step 4: Update src/main.rs to expose modules as library**

Update `src/main.rs`:

```rust
pub mod app;
pub mod tray;

fn main() {
    println!("HonkHonk");
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test`
Expected: All 4 tests PASS

- [ ] **Step 6: Verify clippy passes**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 7: Commit**

```bash
git add src/app.rs tests/app_test.rs src/main.rs
git commit -m "feat(app): add iced application state with update logic"
```

---

## Task 4: Main Entry — Renderer Selection + Iced Launch

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Implement full Iced Application trait in app.rs**

Replace `src/app.rs` with:

```rust
use crate::tray::{TrayEvent, TrayHandle};
use iced::widget::{center, text};
use iced::{Element, Subscription, Task, Theme};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    ToggleVisibility,
    Quit,
    TrayEvent(TrayEvent),
    TrayPoll,
}

impl Message {
    pub fn from_tray_event(event: TrayEvent) -> Self {
        match event {
            TrayEvent::ToggleVisibility => Message::ToggleVisibility,
            TrayEvent::Quit => Message::Quit,
        }
    }
}

pub struct HonkHonk {
    visible: bool,
    exit: bool,
    tray_rx: Arc<Mutex<Receiver<TrayEvent>>>,
}

impl HonkHonk {
    pub fn new(tray_rx: Receiver<TrayEvent>) -> Self {
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(tray_rx)),
        }
    }

    pub fn new_for_test() -> Self {
        let (_tx, rx) = std::sync::mpsc::channel();
        Self {
            visible: true,
            exit: false,
            tray_rx: Arc::new(Mutex::new(rx)),
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleVisibility => {
                self.visible = !self.visible;
                Task::none()
            }
            Message::Quit => {
                self.exit = true;
                iced::exit()
            }
            Message::TrayEvent(event) => {
                let msg = Message::from_tray_event(event);
                self.update(msg)
            }
            Message::TrayPoll => {
                if let Ok(rx) = self.tray_rx.lock() {
                    while let Ok(event) = rx.try_recv() {
                        let msg = Message::from_tray_event(event);
                        return self.update(msg);
                    }
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        center(text("HonkHonk").size(32)).into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(std::time::Duration::from_millis(100))
            .map(|_| Message::TrayPoll)
    }

    pub fn title(&self) -> String {
        String::from("HonkHonk")
    }
}
```

- [ ] **Step 2: Implement main.rs with renderer selection and app launch**

Replace `src/main.rs` with:

```rust
pub mod app;
pub mod tray;

use app::HonkHonk;
use tray::build_tray;

fn main() -> iced::Result {
    let tray_handle = build_tray().expect("failed to initialize system tray");

    let renderer = std::env::var("HONKHONK_RENDERER")
        .unwrap_or_default();

    let mut settings = iced::application::Settings::default();
    settings.antialiasing = true;

    if renderer == "software" {
        iced::application("HonkHonk", HonkHonk::update, HonkHonk::view)
            .subscription(HonkHonk::subscription)
            .theme(HonkHonk::theme)
            .settings(settings)
            .run_with(move || {
                (HonkHonk::new(tray_handle.event_rx), Task::none())
            })
    } else {
        iced::application("HonkHonk", HonkHonk::update, HonkHonk::view)
            .subscription(HonkHonk::subscription)
            .theme(HonkHonk::theme)
            .settings(settings)
            .run_with(move || {
                (HonkHonk::new(tray_handle.event_rx), Task::none())
            })
    }
}
```

Note: Iced 0.13 renderer selection is handled at the application builder level. The above keeps the env var check as a placeholder — actual renderer backend selection in Iced 0.13 may require feature-gating at compile time rather than runtime. We handle this by compiling with both features enabled (`tiny-skia` feature in Cargo.toml). If runtime selection isn't supported, document the limitation and remove the dead branch.

- [ ] **Step 3: Run tests to verify they still pass**

Run: `cargo test`
Expected: All 4 tests PASS (test code uses `new_for_test()` which doesn't require tray)

- [ ] **Step 4: Run the app to verify window appears**

Run: `cargo run`
Expected: A window appears with "HonkHonk" text centered. Tray icon visible in system tray. Clicking "Quit" in tray menu closes app.

- [ ] **Step 5: Test renderer env var**

Run: `HONKHONK_RENDERER=software cargo run`
Expected: Window appears using software renderer (may look identical visually, confirms no crash)

- [ ] **Step 6: Verify clippy passes**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 7: Verify fmt**

Run: `cargo fmt -- --check`
Expected: No formatting issues

- [ ] **Step 8: Commit**

```bash
git add src/main.rs src/app.rs
git commit -m "feat: iced window with tray integration and renderer selection"
```

---

## Task 5: Window Visibility Toggle

**Files:**
- Modify: `src/app.rs`
- Modify: `tests/app_test.rs`

- [ ] **Step 1: Write failing test for window close behavior on hide**

Add to `tests/app_test.rs`:

```rust
#[test]
fn toggle_visibility_does_not_exit() {
    let mut app = HonkHonk::new_for_test();
    let _task = app.update(Message::ToggleVisibility);
    assert!(!app.should_exit());
    assert!(!app.is_visible());
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test`
Expected: PASS (this validates the existing behavior is correct — hide doesn't exit)

- [ ] **Step 3: Manual test — tray Show/Hide toggles window**

Run: `cargo run`
Manual test plan:
1. Window appears
2. Click "Show/Hide" in tray → window should hide (note: actual window hide requires Iced window commands which may need `iced::window::Id` — if Iced 0.13's API doesn't support programmatic hide from within `update`, document as known limitation for this PR and track as follow-up)
3. Click "Show/Hide" again → window should reappear

Note: If Iced 0.13 doesn't expose window visibility toggling directly, the `visible` state is still tracked internally and the UI can show/hide content. Full window-level hide may require the `iced::window::close`/`iced::window::open` pattern or platform-specific code. For PR 1, we track the state and log it. Window-level visibility is a fast follow-up.

- [ ] **Step 4: Commit**

```bash
git add tests/app_test.rs
git commit -m "test: add visibility toggle does not exit assertion"
```

---

## Task 6: Final Verification + Format

**Files:**
- No new files

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Clean

- [ ] **Step 3: Run fmt**

Run: `cargo fmt -- --check`
Expected: Clean

- [ ] **Step 4: Check LOC count**

Run: `find src tests -name '*.rs' | xargs wc -l | tail -1`
Expected: Under 300 lines total

- [ ] **Step 5: Run the app one final time**

Run: `cargo run`
Expected: Window renders "HonkHonk", tray icon present, "Quit" from tray exits cleanly.

- [ ] **Step 6: Final commit if any formatting fixes**

```bash
cargo fmt
git add -A
git commit -m "style: apply rustfmt formatting"
```

(Skip if no changes)

---

## Notes for Implementer

1. **Iced 0.13 API:** The Application trait was replaced with a functional API (`iced::application(title, update, view)`). Check docs.rs/iced/0.13 if the builder pattern above doesn't compile — the API shifted between 0.12 and 0.13.

2. **tray-icon on Linux:** Requires a running D-Bus session and StatusNotifierWatcher. Works on KDE6, GNOME with AppIndicator extension, Hyprland with waybar. If tray doesn't show, check `dbus-monitor` for SNI registration.

3. **Window visibility:** Iced 0.13 may not support `window.set_visible(false)` directly. The state is tracked; actual hiding is a platform concern. If it doesn't work, PR 1 delivers: window + tray + quit. Visibility toggle becomes a fast follow-up PR.

4. **Renderer selection:** If Iced 0.13 doesn't support runtime renderer switching (only compile-time), simplify main.rs to remove the branch and document that `tiny-skia` is available as a feature flag but requires recompilation.

5. **Test in Wayland session:** Run under `WAYLAND_DISPLAY=wayland-1` or native Wayland session. Do NOT test under X11/XWayland — that's explicitly out of scope.
