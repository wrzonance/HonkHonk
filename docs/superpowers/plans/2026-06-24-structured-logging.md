# Structured Leveled Logging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace HonkHonk's ad-hoc `eprintln!` output with `tracing`-based, environment-controlled, leveled logging that carries real context (notably the file path on a decode failure).

**Architecture:** A small `honkhonk::logging` module installs a `tracing-subscriber` `fmt` layer (stderr, compact, ANSI-on-TTY, no timestamps) filtered by `HONKHONK_LOG`; `main()` calls `logging::init()` first; every `eprintln!` site is migrated to the level-mapped `tracing` macro.

**Tech Stack:** Rust, `tracing` 0.1, `tracing-subscriber` 0.3 (`env-filter`).

## Global Constraints

- **Prerequisite:** Rebase this branch onto a `main` that already contains #151. After #151, `app.rs` is `app/mod.rs` + `app/playback.rs`, and the decode-error site is in `app/playback.rs::handle_decoded`. Locate every site by grepping its message text, not by line number.
- **Files ≤ 400 lines; functions ≤ 50 lines.** No `.unwrap()` / `panic!()` in non-test code (the subscriber setup must not panic on a bad directive — use `parse_lossy`).
- **`cargo clippy --all-targets -- -D warnings` clean** (default and `--features pipewire-test`); `cargo fmt --all --check` clean.
- **`cargo deny check` must pass** for the new dependencies; commit `Cargo.lock`.
- **TDD** for the one pure helper; the subscriber/output is third-party I/O verified manually.
- New crate justification (for the PR): stdlib has no logging facade; `tracing`'s structured fields are what attach the filename, and it underpins the future GUI surfacing (#156) and background pipeline (#158).

---

### Task 1: `tracing` dependency + `logging` module + init in `main`

**Files:**
- Modify: `Cargo.toml` (add deps)
- Create: `src/logging.rs`
- Modify: `src/lib.rs` (add `pub mod logging;`)
- Modify: `src/main.rs` (call `honkhonk::logging::init()` first)
- Test: inline `#[cfg(test)] mod tests` in `src/logging.rs`

**Interfaces:**
- Produces:
  - `honkhonk::logging::log_directive(env: Option<&str>) -> String` — the `EnvFilter` directive: the env string if set and non-blank, else `"warn,honkhonk=info"`.
  - `honkhonk::logging::init()` — installs the global subscriber; call once at the top of `main()`.

- [ ] **Step 1: Add the dependencies**

In `Cargo.toml`, under `[dependencies]`, after the `symphonia` line, add:

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 2: Write the failing tests**

Create `src/logging.rs` with ONLY the doc header and the test module (the helper is implemented in Step 4, so the tests fail to compile first):

```rust
//! Process-wide logging setup (#154). Replaces ad-hoc `eprintln!` with
//! `tracing`. `init()` is called once at the very top of `main()`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_falls_back_to_default_when_unset() {
        assert_eq!(log_directive(None), "warn,honkhonk=info");
    }

    #[test]
    fn directive_uses_env_value_when_set() {
        assert_eq!(log_directive(Some("debug")), "debug");
        assert_eq!(log_directive(Some("honkhonk=trace")), "honkhonk=trace");
    }

    #[test]
    fn directive_ignores_blank_env() {
        assert_eq!(log_directive(Some("")), "warn,honkhonk=info");
        assert_eq!(log_directive(Some("   ")), "warn,honkhonk=info");
    }
}
```

Add `pub mod logging;` to `src/lib.rs` (after `pub mod app;`, keeping the list alphabetical):

```rust
pub mod app;
pub mod audio;
pub mod logging;
pub mod settings;
pub mod shortcuts;
pub mod state;
pub mod tray;
pub mod ui;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib logging`
Expected: FAIL to compile — `cannot find function `log_directive` in this scope`.

- [ ] **Step 4: Implement `log_directive` and `init()`**

Add to `src/logging.rs`, above the test module:

```rust
use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;

/// Default verbosity when `HONKHONK_LOG` is unset: our crate at `info`,
/// dependencies quiet at `warn`.
const DEFAULT_DIRECTIVE: &str = "warn,honkhonk=info";

/// Resolves the `EnvFilter` directive from a `HONKHONK_LOG` value: the env
/// string when set and non-blank, otherwise the default. Pure, for testing.
pub fn log_directive(env: Option<&str>) -> String {
    match env {
        Some(s) if !s.trim().is_empty() => s.to_string(),
        _ => DEFAULT_DIRECTIVE.to_string(),
    }
}

/// Installs the global `tracing` subscriber: compact `LEVEL target: message`
/// to stderr, ANSI only on a TTY, no timestamps. Verbosity from `HONKHONK_LOG`
/// (default `warn,honkhonk=info`). Call once, first thing in `main()`.
pub fn init() {
    let directive = log_directive(std::env::var("HONKHONK_LOG").ok().as_deref());
    // `parse_lossy` never panics: invalid directives are dropped with a warning.
    let filter = EnvFilter::builder().parse_lossy(directive);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .without_time()
        .init();
}
```

- [ ] **Step 5: Call `init()` first in `main()`**

In `src/main.rs`, make `honkhonk::logging::init();` the **first statement** of `main()`, before the `AppConfig::load()` block:

```rust
fn main() -> iced::Result {
    honkhonk::logging::init();

    let config = match honkhonk::state::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: failed to load config, using defaults: {e}");
            honkhonk::state::AppConfig::default()
        }
    };
    // ... rest unchanged (this eprintln is migrated in Task 2) ...
```

- [ ] **Step 6: Verify build, tests, deny**

Run: `cargo test --lib logging` → PASS (3 tests).
Run: `cargo build` → compiles.
Run: `cargo deny check 2>&1 | tail -5` → no new license/advisory failures from `tracing`/`tracing-subscriber`.
Run: `cargo clippy --all-targets -- -D warnings` → clean; `cargo fmt --all --check` → clean.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/logging.rs src/lib.rs src/main.rs
git commit -m "feat(infra): tracing-based logging init + HONKHONK_LOG filter (#154)"
```

---

### Task 2: Migrate every `eprintln!` to a level-mapped `tracing` macro

**Files (locate each site by grepping its message — line numbers shift post-#151):**
- Modify: `src/main.rs`, `src/app/mod.rs`, `src/app/playback.rs`, `src/audio/engine.rs`, `src/audio/streams.rs`, `src/audio/registry.rs`, `src/shortcuts/config_ui.rs`, `src/shortcuts/portal.rs`

**Interfaces:**
- Consumes: nothing (uses `tracing::{error,warn,info,debug}!` fully-qualified — no `use` needed, avoids unused-import churn).
- Produces: no public API; behavior is logging-only.

- [ ] **Step 1: Migrate `src/main.rs` (4 remaining sites)**

| before | after |
|---|---|
| `eprintln!("warning: failed to load config, using defaults: {e}");` | `tracing::warn!(error = %e, "failed to load config; using defaults");` |
| `eprintln!("fatal: failed to initialize GTK (required for system tray): {e}");` | `tracing::error!(error = %e, "failed to initialize GTK (required for system tray); exiting");` |
| `eprintln!("warning: failed to scan sound library: {e}");` | `tracing::warn!(error = %e, "failed to scan sound library");` |
| `eprintln!("fatal: failed to initialize system tray: {e}");` | `tracing::error!(error = %e, "failed to initialize system tray; exiting");` |
| `eprintln!("fatal: failed to start audio engine: {e}");` | `tracing::error!(error = %e, "failed to start audio engine; exiting");` |

- [ ] **Step 2: Migrate `src/audio/engine.rs` — the router chatter to `debug`**

`eprintln!("honkhonk router: {event:?}");` → `tracing::debug!(?event, "router event");`

- [ ] **Step 3: Migrate `src/audio/streams.rs` and `src/audio/registry.rs` (warn)**

- `streams.rs`: `eprintln!("honkhonk: failed to bind stream node {}: {e}", global.id);` → `tracing::warn!(node = global.id, error = %e, "failed to bind stream node");`
- `registry.rs`: `eprintln!("honkhonk: failed to create mic passthrough link: {e}");` → `tracing::warn!(error = %e, "failed to create mic passthrough link");`
- `registry.rs`: `eprintln!("honkhonk: failed to create monitor→source link: {e}");` → `tracing::warn!(error = %e, "failed to create monitor->source link");`

- [ ] **Step 4: Migrate `src/shortcuts/config_ui.rs` and `src/shortcuts/portal.rs` (warn)**

Replace each, preserving the message text:

- `"honkhonk: configure_shortcuts command dropped: {e}"` → `tracing::warn!(error = %e, "configure_shortcuts command dropped");`
- `"honkhonk: configure_shortcuts: portal v2 flagged but handle not yet received"` → `tracing::warn!("configure_shortcuts: portal v2 flagged but handle not yet received");`
- `"honkhonk: failed to open KDE shortcuts: {e}"` → `tracing::warn!(error = %e, "failed to open KDE shortcuts");`
- `"honkhonk: failed to open GNOME keyboard settings: {e}"` → `tracing::warn!(error = %e, "failed to open GNOME keyboard settings");`
- `"honkhonk: configure_shortcuts requires portal v2 on Hyprland (not available)"` → `tracing::warn!("configure_shortcuts requires portal v2 on Hyprland (not available)");`
- `"honkhonk: configure_shortcuts requires portal v2 on Sway (not available)"` → `tracing::warn!("configure_shortcuts requires portal v2 on Sway (not available)");`
- the `Unknown(de)` two-line `"honkhonk: no shortcut config path for DE '{de}'; install xdg-desktop-portal v2"` → `tracing::warn!(de = %de, "no shortcut config path for this DE; install xdg-desktop-portal v2");`
- `portal.rs`: `"honkhonk: configure_shortcuts failed: {e}"` → `tracing::warn!(error = %e, "configure_shortcuts failed");`

- [ ] **Step 5: Migrate `src/app/mod.rs`**

Lifecycle / errors:
- `"honkhonk: audio engine ready"` → `tracing::info!("audio engine ready");`
- `"honkhonk: audio error: {e}"` → `tracing::error!(error = %e, "audio error");`
- `"honkhonk: {notice}"` (the `SourceFirstRun` notice) → `tracing::info!(message = %notice, "source first-run notice");`

Save failures — every identical occurrence is replaced the same way (replace-all):
- `"honkhonk: failed to save config: {e}"` → `tracing::warn!(error = %e, "failed to save config");`
- `"honkhonk: config save error: {e}"` → `tracing::warn!(error = %e, "config save error");`
- `"honkhonk: slots save error: {e}"` → `tracing::warn!(error = %e, "slots save error");`
- `"honkhonk: sound meta save error: {e}"` → `tracing::warn!(error = %e, "sound meta save error");`

Multi-arg sites:
- `"honkhonk: slot {} points to missing file {:?}, clearing", idx + 1, path` → `tracing::warn!(slot = idx + 1, ?path, "slot points to missing file; clearing stale slot");`
- `"honkhonk: directory picker error: {e:#}"` → `tracing::warn!(error = ?e, "directory picker error");`
- `"honkhonk: library rescan failed for {:?}: {e}", self.config.sound_directories` → `tracing::warn!(dirs = ?self.config.sound_directories, error = %e, "library rescan failed");`

- [ ] **Step 6: Migrate the decode-error site in `src/app/playback.rs` WITH the file path (the headline)**

In `handle_decoded`'s `Err(e)` branch, replace `eprintln!("honkhonk: decode error: {e}");` with a path-resolving log:

```rust
            Err(e) => {
                let file = self
                    .sounds
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.path.display().to_string())
                    .unwrap_or_else(|| id.clone()); // fall back to id if rescanned away
                tracing::error!(file = %file, error = %e, "decode failed");
                self.clear_playback_state();
            }
```

(If, before #151 is merged, this site is still the synchronous `play_sound_entry` in `app.rs` where `sound` is in scope, use `tracing::error!(file = %sound.path.display(), error = %e, "decode failed");` instead. The post-#151 form above is the target.)

- [ ] **Step 7: Verify no `eprintln!` remains and everything is green**

Run: `grep -rn 'eprintln!' src` → returns **nothing** (all migrated).
Run: `cargo test` → all pass (no behavior change).
Run: `cargo test --features pipewire-test --no-run` → compiles.
Run: `cargo clippy --all-targets -- -D warnings` and `cargo clippy --all-targets --features pipewire-test -- -D warnings` → clean.
Run: `cargo fmt --all --check` → clean.

- [ ] **Step 8: Manual verification (record results in the PR)**

```bash
cargo run 2>err.log          # default: info+ shown, router chatter hidden
HONKHONK_LOG=debug cargo run  # router "router event" debug lines now appear
cargo run 2>err.log; grep -c $'\e' err.log   # piped to file -> 0 ANSI escape bytes
# Fire a known-bad audio file -> the error line includes `file=<path>`
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor: migrate eprintln! to leveled tracing logging (#154)"
```

---

## Self-Review

**Spec coverage:**
- `tracing` + `tracing-subscriber` (env-filter) dependency → Task 1 Step 1. ✓
- `logging::init()` first in `main()` → Task 1 Steps 4–5. ✓
- `HONKHONK_LOG`, default `warn,honkhonk=info` → `log_directive` + `init()`, tested. ✓
- stderr / compact / ANSI-on-TTY / no timestamps → `init()` (`with_writer(stderr)`, `with_ansi(is_terminal())`, `without_time()`). ✓
- Level map (error/warn/info/debug) across all sites → Task 2 Steps 1–6. ✓
- Decode failure logs the file path → Task 2 Step 6. ✓
- Router `SourceDisconnected` chatter → debug → Task 2 Step 2. ✓
- No `eprintln!` remains; tests stay green → Task 2 Step 7. ✓
- GUI surfacing out of scope (#156) → not in plan. ✓

**Placeholder scan:** none. Replace-all rules for identical save-error lines are exact transformations, not vague gaps; the decode site shows full code for both the post-#151 and pre-#151 forms.

**Type consistency:** `log_directive(Option<&str>) -> String` and `init()` are used identically in Task 1 and `main.rs`. The `tracing::{error,warn,info,debug}!` macros are fully-qualified everywhere (no import drift). `DEFAULT_DIRECTIVE` value `"warn,honkhonk=info"` matches the tests and the spec.
