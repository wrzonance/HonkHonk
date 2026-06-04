# Persistent Virtual Source (conf.d + first-run fallback) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the HonkHonk virtual microphone (`honkhonk-mic`) persist across app restarts and reboots so Discord/OBS never lose their input selection.

**Architecture:** Ship a PipeWire `pipewire.conf.d` drop-in via packages (system-wide) that declares a lingering null-audio-sink source. At app startup, query the PipeWire registry: if `honkhonk-mic` already exists (conf.d case) skip programmatic creation; if absent (dev/unpackaged first run) create it programmatically with `object.linger = true`, write a per-user conf.d file as the persistence bridge, and surface a first-run notice to the UI. Only the **source** persists — the internal mixing sink stays app-lifetime and is destroyed on shutdown along with all links.

**Tech Stack:** Rust, pipewire-rs, `directories` crate (XDG paths), serde (existing), `cargo deb` packaging, Flatpak/AUR/AppImage configs.

---

## File Structure

- `src/audio/confd.rs` — **NEW**. Pure module: conf.d file content constant, XDG path resolution, write-if-absent logic. No PipeWire. Fully unit-testable. Owns `ConfdError` variants surfaced via `AudioError`.
- `src/audio/error.rs` — **MODIFY**. Add `AudioError` variants for conf.d write failures (typed, no String-across-boundary).
- `src/audio/engine.rs` — **MODIFY**. Add `source_already_exists` registry pre-check; decision function `should_create_source`; change source `object.linger` to `"true"`; only create source when absent + write user conf.d + emit first-run event; emit new `AudioEvent::SourcePersisted`/`SourceFirstRun`. Shutdown: do NOT take/destroy source (already the case — source Node lives in `run_engine` scope; document that links + sink are session-lifetime, source is not destroyed by us when lingering).
- `src/audio/mod.rs` — **MODIFY**. Re-export new module items if needed.
- `src/app.rs` — **MODIFY**. Handle the new `AudioEvent` first-run variant (log + set a one-shot notice flag; minimal — no new panel).
- `packaging/pipewire/50-honkhonk.conf` — **NEW**. The canonical conf.d drop-in shipped by all packages.
- `Cargo.toml` — **MODIFY**. `[package.metadata.deb]` add the conf.d asset (installed under `/usr/share/pipewire/pipewire.conf.d/`) + `maintainer-scripts` dir for postrm. The drop-in is a vendor file under `/usr/share`, so it is intentionally NOT marked a dpkg conffile — see the deb-policy note below and ADR-004.
- `packaging/deb/postrm` — **NEW**. Post-remove scriptlet restarting PipeWire.
- `packaging/aur/honkhonk-bin/PKGBUILD` + `.SRCINFO` — **MODIFY**. Install conf.d from the extracted .deb (or vendored copy).
- `packaging/flatpak/io.github.thewrz.HonkHonk.yml` — **MODIFY/DECISION**. Flatpak is sandboxed; document why conf.d is NOT installed there (no host PipeWire conf access) — first-run fallback covers it.
- `packaging/appimage/HonkHonk.AppDir/AppRun` — **DECISION**. AppImage is not a system package; first-run fallback covers it (document, no conf.d install).
- `tests/packaging/deb_validate.sh` — **MODIFY**. Add checks that conf.d asset + postrm are declared.
- `docs/adr/004-persistent-source-conf-d.md` — **NEW**. ADR-004.

---

## Canonical conf.d content (single source of truth)

This EXACT block is used in both `packaging/pipewire/50-honkhonk.conf` and the Rust `CONFD_CONTENTS` constant. Tests assert byte-equality.

```
# Installed by HonkHonk — persistent virtual microphone.
# Declares the "honkhonk-mic" virtual source so it exists across reboots and
# whether or not the HonkHonk app is running. An idle null-audio-sink uses
# zero CPU. Remove this file (and restart PipeWire) to drop the device.
context.objects = [
    {
        factory = adapter
        args = {
            factory.name     = support.null-audio-sink
            node.name        = "honkhonk-mic"
            node.description = "HonkHonk Mic"
            media.class      = Audio/Source/Virtual
            audio.position   = [ FL FR ]
            object.linger     = true
        }
    }
]
```

> NOTE: The issue body shows the same block with looser whitespace. We pin one
> canonical formatting and assert it in tests. The PipeWire SPA-JSON parser is
> whitespace-insensitive, so formatting is a free choice; we choose aligned `=`.

---

## Task 1: conf.d content + XDG path module (pure, no PipeWire)

**Files:**
- Create: `src/audio/confd.rs`
- Modify: `src/audio/mod.rs`
- Modify: `src/audio/error.rs`

- [ ] **Step 1: Add error variants to `src/audio/error.rs`**

Append inside the `AudioError` enum (before the closing brace):

```rust
    #[error("failed to resolve XDG config directory for PipeWire conf.d")]
    ConfdNoConfigDir,

    #[error("failed to create PipeWire conf.d directory at {path}")]
    ConfdDirCreate {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write PipeWire conf.d file at {path}")]
    ConfdWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },
```

- [ ] **Step 2: Write the failing tests in `src/audio/confd.rs`**

Create `src/audio/confd.rs` with ONLY the tests + signatures first (so it compiles to a failing state). Full file:

```rust
//! Persistent virtual-source `pipewire.conf.d` drop-in (issue #49).
//!
//! Pure, PipeWire-free logic: the canonical conf.d file contents, XDG path
//! resolution for the per-user drop-in, and a write-if-absent helper used by
//! the first-run fallback when no packaged conf.d is present.
//!
//! Packaged installs ship the same contents to
//! `/usr/share/pipewire/pipewire.conf.d/50-honkhonk.conf` (package-manager
//! territory). This module only ever writes the per-user copy under
//! `$XDG_CONFIG_HOME/pipewire/pipewire.conf.d/`.

use std::path::{Path, PathBuf};

use super::error::AudioError;

/// File name for the HonkHonk drop-in. `50-` orders it after PipeWire defaults.
pub const CONFD_FILE_NAME: &str = "50-honkhonk.conf";

/// Canonical drop-in contents. Byte-identical to
/// `packaging/pipewire/50-honkhonk.conf` (asserted in tests).
pub const CONFD_CONTENTS: &str = include_str!("../../packaging/pipewire/50-honkhonk.conf");

/// Resolve the per-user conf.d *directory*:
/// `$XDG_CONFIG_HOME/pipewire/pipewire.conf.d` (default `~/.config/...`).
pub fn user_confd_dir() -> Result<PathBuf, AudioError> {
    let base = directories::BaseDirs::new().ok_or(AudioError::ConfdNoConfigDir)?;
    Ok(base
        .config_dir()
        .join("pipewire")
        .join("pipewire.conf.d"))
}

/// Full path to the per-user drop-in file.
pub fn user_confd_path() -> Result<PathBuf, AudioError> {
    Ok(user_confd_dir()?.join(CONFD_FILE_NAME))
}

/// Write the drop-in to `dir/CONFD_FILE_NAME` unless it already exists.
/// Returns `Ok(true)` if it wrote a new file, `Ok(false)` if one was already
/// present. Creates the directory tree as needed.
pub fn write_user_confd_in(dir: &Path) -> Result<bool, AudioError> {
    let path = dir.join(CONFD_FILE_NAME);
    if path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(dir).map_err(|source| AudioError::ConfdDirCreate {
        path: dir.display().to_string(),
        source,
    })?;
    std::fs::write(&path, CONFD_CONTENTS).map_err(|source| AudioError::ConfdWrite {
        path: path.display().to_string(),
        source,
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confd_contents_declares_honkhonk_mic_source() {
        assert!(CONFD_CONTENTS.contains("node.name        = \"honkhonk-mic\""));
        assert!(CONFD_CONTENTS.contains("media.class      = Audio/Source/Virtual"));
    }

    #[test]
    fn confd_contents_sets_object_linger_true() {
        assert!(CONFD_CONTENTS.contains("object.linger     = true"));
    }

    #[test]
    fn confd_contents_uses_null_audio_sink_factory() {
        assert!(CONFD_CONTENTS.contains("factory.name     = support.null-audio-sink"));
    }

    #[test]
    fn user_confd_dir_ends_with_pipewire_confd() {
        // Uses real XDG; only assert the suffix shape, not the absolute prefix.
        let dir = user_confd_dir().expect("BaseDirs resolvable in test env");
        assert!(dir.ends_with("pipewire/pipewire.conf.d"));
    }

    #[test]
    fn write_user_confd_in_creates_file_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("pipewire/pipewire.conf.d");
        let wrote = write_user_confd_in(&target).unwrap();
        assert!(wrote);
        let written = std::fs::read_to_string(target.join(CONFD_FILE_NAME)).unwrap();
        assert_eq!(written, CONFD_CONTENTS);
    }

    #[test]
    fn write_user_confd_in_is_idempotent_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("pipewire/pipewire.conf.d");
        assert!(write_user_confd_in(&target).unwrap());
        // Second call: file exists → returns false, does not overwrite.
        assert!(!write_user_confd_in(&target).unwrap());
    }
}
```

- [ ] **Step 3: Create `packaging/pipewire/50-honkhonk.conf`** with the canonical content block above (so `include_str!` resolves).

- [ ] **Step 4: Register module in `src/audio/mod.rs`**

Add `mod confd;` (private — engine uses it via `super::confd`). After:

```rust
mod confd;
mod decoder;
mod engine;
```

- [ ] **Step 5: Run tests to verify they fail then pass**

Run: `cargo test -p honkhonk confd`
Expected: compiles, the 6 confd tests PASS (this is a fresh module; if `include_str!` path is wrong it FAILS to compile — fix path). `tempfile` is already a dev-dependency (used in config.rs tests).

- [ ] **Step 6: Commit**

```bash
git add src/audio/confd.rs src/audio/mod.rs src/audio/error.rs packaging/pipewire/50-honkhonk.conf
git commit -m "feat(audio): conf.d content + XDG path module for persistent source"
```

---

## Task 2: Startup decision logic — skip create vs create+write

**Files:**
- Modify: `src/audio/engine.rs`

The decision is a pure function so it is unit-testable without PipeWire. The registry pre-check (does `honkhonk-mic` exist?) feeds a `bool` into it.

- [ ] **Step 1: Write the failing test in `src/audio/engine.rs` tests module**

Add to the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn should_create_source_false_when_node_already_present() {
        assert!(!should_create_source(true));
    }

    #[test]
    fn should_create_source_true_when_node_absent() {
        assert!(should_create_source(false));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p honkhonk should_create_source`
Expected: FAIL — `should_create_source` not found.

- [ ] **Step 3: Implement the decision function**

Add near `create_virtual_source` in `src/audio/engine.rs`:

```rust
/// First-run decision: create the virtual source programmatically only when
/// no `honkhonk-mic` node already exists (i.e. no packaged/user conf.d has
/// declared it). When it already exists we reuse it and never recreate.
fn should_create_source(source_already_exists: bool) -> bool {
    !source_already_exists
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p honkhonk should_create_source`
Expected: PASS (both).

- [ ] **Step 5: Commit**

```bash
git add src/audio/engine.rs
git commit -m "feat(audio): add should_create_source first-run decision fn"
```

---

## Task 3: Registry pre-check for an existing honkhonk-mic node

**Files:**
- Modify: `src/audio/engine.rs`

We synchronously probe for an existing `honkhonk-mic` node once at startup,
before deciding whether to create it. The plumbing (passing the resulting bool
to `should_create_source`) is covered by the Task 2 unit tests.

DESIGN DECISION (documented in ADR-004): the probe shells out to `pw-dump`
(matching the existing subprocess pattern used by `query_default_source_name`)
and parses its JSON output with a pure, unit-tested helper
(`source_present_in_dump`) that scans for a node named `honkhonk-mic`. We chose
a subprocess over a live registry roundtrip so the decision logic stays pure
and fully testable without a PipeWire connection — the JSON-parsing helper is
covered by unit tests, and only the thin `pw-dump` invocation is
PipeWire-dependent. If `pw-dump` is unavailable (e.g. CI without PipeWire), the
probe returns `false` (assume absent) so the engine falls back to programmatic
creation, which itself fails gracefully if PipeWire is absent.

- [ ] **Step 1: Write failing tests for the parser in `src/audio/engine.rs` tests**

```rust
    #[test]
    fn parse_source_present_detects_honkhonk_mic() {
        let dump = r#"
        id 42, type PipeWire:Interface:Node/3
            node.name = "honkhonk-mic"
            media.class = "Audio/Source/Virtual"
        "#;
        assert!(source_present_in_dump(dump));
    }

    #[test]
    fn parse_source_present_false_when_absent() {
        let dump = r#"
        id 7, type PipeWire:Interface:Node/3
            node.name = "alsa_input.pci-0000"
        "#;
        assert!(!source_present_in_dump(dump));
    }

    #[test]
    fn parse_source_present_false_on_empty() {
        assert!(!source_present_in_dump(""));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p honkhonk source_present`
Expected: FAIL — `source_present_in_dump` not found.

- [ ] **Step 3: Implement parser + probe**

Add to `src/audio/engine.rs`:

```rust
/// Pure scan: does a `pw-cli`/`pw-dump` text blob mention a node whose
/// `node.name` is our virtual source? Matches the quoted name token so a
/// substring like `honkhonk-mic-foo` does not false-positive.
fn source_present_in_dump(dump: &str) -> bool {
    let needle = format!("\"{SOURCE_NODE_NAME}\"");
    dump.lines().any(|line| {
        let l = line.trim();
        l.starts_with("node.name") && l.contains(&needle)
    })
}

/// Probe PipeWire (via `pw-dump`) for an existing `honkhonk-mic` node.
/// Returns `false` if the tool is missing or fails — the caller then falls
/// back to programmatic creation, which itself fails gracefully without PW.
fn source_already_exists() -> bool {
    std::process::Command::new("pw-dump")
        .output()
        .ok()
        .map(|o| source_present_in_dump(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or(false)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p honkhonk source_present`
Expected: PASS (3).

- [ ] **Step 5: Commit**

```bash
git add src/audio/engine.rs
git commit -m "feat(audio): probe registry for existing honkhonk-mic node"
```

---

## Task 4: Wire first-run path into run_engine + linger=true + new AudioEvent

**Files:**
- Modify: `src/audio/engine.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add the new AudioEvent variant + test**

In `src/audio/engine.rs`, extend the `AudioEvent` enum:

```rust
    /// Emitted once on a first run that created the source programmatically and
    /// wrote the per-user conf.d. The UI shows a one-time notice telling the
    /// user the "HonkHonk Mic" device now persists and to select it in
    /// Discord/OBS. Carries whether a new conf.d file was actually written.
    SourceFirstRun { confd_written: bool },
```

Add a constructibility test in the engine tests module:

```rust
    #[test]
    fn audio_event_source_first_run_is_constructible() {
        let _ = AudioEvent::SourceFirstRun { confd_written: true };
        let _ = AudioEvent::SourceFirstRun { confd_written: false };
    }
```

- [ ] **Step 2: Change source creation to linger=true**

In `create_virtual_source`, change:

```rust
        "object.linger" => "false",
```
to
```rust
        "object.linger" => "true",
```

(The sink in `create_virtual_sink` stays `"false"` — session-lifetime.)

- [ ] **Step 3: Replace the unconditional source creation in `run_engine`**

Find:

```rust
    let _sink = create_virtual_sink(&core)?;
    let _source = create_virtual_source(&core)?;
```

Replace with:

```rust
    let _sink = create_virtual_sink(&core)?;

    // Persistent virtual source (issue #49): reuse a conf.d-declared device if
    // present; otherwise create it programmatically (lingering) and write the
    // per-user conf.d as the persistence bridge for dev/unpackaged runs.
    let _source = if should_create_source(source_already_exists()) {
        let node = create_virtual_source(&core)?;
        let confd_written = match confd::user_confd_dir() {
            Ok(dir) => confd::write_user_confd_in(&dir).unwrap_or_else(|e| {
                let _ = evt_tx.send(AudioEvent::Error(format!("conf.d write: {e}")));
                false
            }),
            Err(e) => {
                let _ = evt_tx.send(AudioEvent::Error(format!("conf.d path: {e}")));
                false
            }
        };
        let _ = evt_tx.send(AudioEvent::SourceFirstRun { confd_written });
        Some(node)
    } else {
        None
    };
```

Add the import at the top of `engine.rs`:

```rust
use super::confd;
```

Note: `_source` becomes `Option<Node>`; it is held to end-of-scope and is NEVER
explicitly destroyed. On shutdown the lingering node survives the app per
`object.linger = true`; a non-lingering programmatic node would die, but the
conf.d bridge means it is re-created next session regardless. Links and the
sink remain session-lifetime (dropped at end of `run_engine`).

- [ ] **Step 4: Handle SourceFirstRun in `src/app.rs`**

In the `Message::AudioEvent(event)` match arm, add a branch (after `AudioEvent::Error`):

```rust
                    AudioEvent::SourceFirstRun { confd_written } => {
                        eprintln!(
                            "honkhonk: created persistent HonkHonk Mic (conf.d written: {confd_written}). \
Select 'HonkHonk Mic' as your input in Discord/OBS."
                        );
                        self.source_notice = Some(if confd_written {
                            "Created HonkHonk Mic virtual device. It will persist after restart. \
Select 'HonkHonk Mic' as your input in Discord/OBS."
                                .to_string()
                        } else {
                            "HonkHonk Mic created for this session. \
Select 'HonkHonk Mic' as your input in Discord/OBS."
                                .to_string()
                        });
                    }
```

Add a `source_notice: Option<String>` field to the `HonkHonk` struct and
initialize it to `None` in `HonkHonk::new`. (Find the struct definition and the
constructor; add the field consistently.) This is the minimal UI surface — a
banner can consume it later; out of scope to render a panel now.

- [ ] **Step 5: Add an app-level test for the notice transition**

In `src/app.rs` tests, add:

```rust
    #[test]
    fn source_first_run_sets_notice() {
        let mut app = test_app();
        let _ = app.update(Message::AudioEvent(AudioEvent::SourceFirstRun {
            confd_written: true,
        }));
        assert!(app.source_notice.is_some());
        assert!(app.source_notice.as_deref().unwrap().contains("persist"));
    }
```

(Use whatever existing test constructor the file uses — search for how other
app tests build `app`, e.g. a `test_app()` helper or inline `HonkHonk::new`.
Match the established pattern; do not invent a new constructor.)

- [ ] **Step 6: Run tests**

Run: `cargo test -p honkhonk source_first_run && cargo test -p honkhonk audio_event_source_first_run`
Expected: PASS.
Then full suite: `cargo test`
Expected: all PASS.

- [ ] **Step 7: Lint + format**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings`
Expected: zero warnings. If `confd.rs` or engine changes trip cognitive-complexity, extract helpers.

- [ ] **Step 8: Commit**

```bash
git add src/audio/engine.rs src/app.rs
git commit -m "feat(audio): persistent source first-run path (linger + conf.d bridge)"
```

---

## Task 5: Package the conf.d (deb) + postrm scriptlet

**Files:**
- Modify: `Cargo.toml`
- Create: `packaging/deb/postrm`
- Modify: `tests/packaging/deb_validate.sh`

- [ ] **Step 1: Create `packaging/deb/postrm`**

```bash
#!/bin/sh
# HonkHonk deb post-removal: drop the persistent virtual source by restarting
# the user's PipeWire so it stops reading the (now-removed) conf.d drop-in.
# Best-effort: must never fail the package removal.
set -e

if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
    # Restart the *invoking user's* PipeWire if possible. dpkg runs as root,
    # so target the user that ran the package manager via SUDO_USER when set.
    if [ -n "${SUDO_USER:-}" ] && command -v runuser >/dev/null 2>&1; then
        runuser -u "$SUDO_USER" -- \
            systemctl --user restart pipewire.socket pipewire.service 2>/dev/null || true
    else
        systemctl --user restart pipewire.socket pipewire.service 2>/dev/null || true
    fi
fi

exit 0
```

Make it executable: `chmod +x packaging/deb/postrm`.

- [ ] **Step 2: Wire conf.d asset + maintainer-scripts + conf-files into `Cargo.toml`**

In `[package.metadata.deb]`, after the existing `assets = [ ... ]`, add the
conf.d asset line inside the array and the script/conf-files keys after it:

Add to the `assets` array (before the closing `]`):

```toml
    # persistent virtual-source PipeWire drop-in (issue #49)
    ["packaging/pipewire/50-honkhonk.conf", "usr/share/pipewire/pipewire.conf.d/", "644"],
```

Add after the `assets = [...]` block:

```toml
maintainer-scripts = "packaging/deb/"
```

> NOTE: `conf-files` in `[package.metadata.deb]` marks files under `/etc` as
> dpkg conffiles. Our drop-in lives under `/usr/share` (vendor default,
> read-only, not user-edited), so we do NOT mark it a conffile — that is correct
> Debian policy for vendor PipeWire drop-ins. Document this in the ADR.

- [ ] **Step 3: Extend `tests/packaging/deb_validate.sh`**

Add before the Summary section:

```bash
# ── conf.d drop-in (issue #49) ───────────────────────────────────────
CONFD="packaging/pipewire/50-honkhonk.conf"
[ -f "$CONFD" ] \
    && check "conf.d drop-in exists" "ok" \
    || check "conf.d drop-in exists" "missing: $CONFD"

if [ -f "$CONFD" ]; then
    grep -q 'node.name .*"honkhonk-mic"' "$CONFD" \
        && check "conf.d declares honkhonk-mic" "ok" \
        || check "conf.d declares honkhonk-mic" "node.name missing"
    grep -q 'object.linger .* true' "$CONFD" \
        && check "conf.d sets object.linger true" "ok" \
        || check "conf.d sets object.linger true" "missing"
fi

grep -q 'pipewire.conf.d' "$CARGO" \
    && check "Cargo.toml deb installs conf.d" "ok" \
    || check "Cargo.toml deb installs conf.d" "asset entry missing"

grep -q 'maintainer-scripts' "$CARGO" \
    && check "Cargo.toml deb has maintainer-scripts" "ok" \
    || check "Cargo.toml deb has maintainer-scripts" "field missing"

POSTRM="packaging/deb/postrm"
[ -x "$POSTRM" ] \
    && check "deb postrm exists and is executable" "ok" \
    || check "deb postrm exists and is executable" "missing or not +x: $POSTRM"
```

- [ ] **Step 4: Run the validator locally**

Run: `bash tests/packaging/deb_validate.sh`
Expected: all PASS, exit 0. (Requires `imagemagick`/`identify` for the icon
dimension check — if unavailable locally that single check is skipped by the
existing `||`; the new checks do not need it.)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml packaging/deb/postrm tests/packaging/deb_validate.sh
git commit -m "feat(packaging): ship persistent source conf.d in .deb + postrm restart"
```

---

## Task 6: AUR honkhonk-bin installs conf.d; Flatpak/AppImage documented

**Files:**
- Modify: `packaging/aur/honkhonk-bin/PKGBUILD`
- Modify: `packaging/aur/honkhonk-bin/.SRCINFO`
- Modify: `packaging/flatpak/io.github.thewrz.HonkHonk.yml`
- Modify: `packaging/appimage/HonkHonk.AppDir/AppRun`

The AUR `honkhonk-bin` extracts the .deb. Since the .deb now contains the
conf.d (Task 5), install it from the extracted tree.

- [ ] **Step 1: Add conf.d install to PKGBUILD `package()`**

After the icon install line, add:

```bash
    install -Dm644 "$srcdir/usr/share/pipewire/pipewire.conf.d/50-honkhonk.conf" \
        "$pkgdir/usr/share/pipewire/pipewire.conf.d/50-honkhonk.conf"
```

- [ ] **Step 2: Regenerate `.SRCINFO` (or hand-edit)**

`.SRCINFO` does not list per-file installs, so no change is required for the
install step itself. Verify no field needs updating (depends already includes
`pipewire`). If `makepkg --printsrcinfo` is available run it; otherwise leave
`.SRCINFO` unchanged and note in the commit that it is unaffected.

- [ ] **Step 3: Flatpak — document the deliberate omission**

In `packaging/flatpak/io.github.thewrz.HonkHonk.yml`, add a comment in the
`honkhonk` module's `build-commands` (near the metainfo install) explaining the
decision (no functional change):

```yaml
      # NOTE (issue #49): the persistent PipeWire conf.d drop-in is NOT
      # installed in the Flatpak. The sandbox cannot write host PipeWire
      # config, and the app talks to PipeWire via the pulseaudio socket. The
      # app's first-run fallback creates a lingering source instead. See
      # docs/adr/004-persistent-source-conf-d.md.
```

- [ ] **Step 4: AppImage — document the deliberate omission**

In `packaging/appimage/HonkHonk.AppDir/AppRun`, add a comment after the header:

```bash
# NOTE (issue #49): AppImage is not a system package and installs no system
# PipeWire conf.d. The persistent virtual source is provided by the app's
# first-run fallback (lingering node + per-user conf.d). See
# docs/adr/004-persistent-source-conf-d.md.
```

- [ ] **Step 5: Sanity-check shell/YAML**

Run: `bash -n packaging/appimage/HonkHonk.AppDir/AppRun && bash -n packaging/deb/postrm`
Expected: no syntax errors.
Run: `python3 -c "import yaml,sys; yaml.safe_load(open('packaging/flatpak/io.github.thewrz.HonkHonk.yml'))"` (if PyYAML available; else skip).

- [ ] **Step 6: Commit**

```bash
git add packaging/aur packaging/flatpak packaging/appimage
git commit -m "feat(packaging): AUR installs conf.d; document flatpak/appimage fallback"
```

---

## Task 7: ADR-004

**Files:**
- Create: `docs/adr/004-persistent-source-conf-d.md`

- [ ] **Step 1: Write the ADR** following the CLAUDE.md ADR format. Must cover:
  only the source persists (not the internal mixing sink); conf.d as the
  mechanism (vs systemd user service vs WirePlumber rules); first-run fallback
  for dev/unpackaged/Flatpak/AppImage builds; why not both devices persistent;
  why `/usr/share` (vendor) not `/etc` conffile; postrm PipeWire restart on
  uninstall.

```markdown
# ADR-004: Persistent virtual source via PipeWire conf.d

## Status: Accepted

## Context
HonkHonk's virtual microphone (`honkhonk-mic`) previously existed only while
the app ran (`object.linger = false`). On exit/crash, Discord/OBS lost the
selected input and fell back to default — the #1 UX complaint for app-lifetime
virtual-audio tools (EasyEffects, Soundux). Users expect the mic to behave like
a device (VoiceMeeter, NoiseTorch), persisting across app restarts and reboots.

CLAUDE.md referenced an ADR-004 ("persistent sink, no per-sound nodes") that
was never written. That intent meant "persistent within a session". This ADR
supersedes that intent and creates the file: the **source** is now
system-persistent; the internal mixing **sink** remains session-persistent.

## Decision
Ship a PipeWire `pipewire.conf.d` drop-in
(`/usr/share/pipewire/pipewire.conf.d/50-honkhonk.conf`) via every system
package. It declares a lingering `support.null-audio-sink` exposing
`media.class = Audio/Source/Virtual` named `honkhonk-mic`. PipeWire owns the
device's lifecycle; it exists whether or not the app runs and across reboots.

App startup queries the registry for `honkhonk-mic`:
- **Found** (packaged/conf.d case): reuse it, skip programmatic creation.
- **Absent** (dev/unpackaged/Flatpak/AppImage first run): create it
  programmatically with `object.linger = true` (survives app exit until reboot)
  AND write a per-user drop-in to
  `$XDG_CONFIG_HOME/pipewire/pipewire.conf.d/50-honkhonk.conf` as the
  persistence bridge, then surface a one-time UI notice.

Only the source persists. The internal mixing sink (`honkhonk-mix`), mic
passthrough links, and playback streams stay app-lifetime and are torn down on
shutdown.

### Alternatives rejected
- **systemd user service**: heavier, adds a unit to manage, still needs a node
  definition; conf.d achieves persistence with one declarative file and zero
  running process.
- **WirePlumber routing rules**: WirePlumber policy is for *routing*, not for
  *declaring* a static null sink; wrong layer, more fragile across WirePlumber
  versions, and out of scope (#17 handles auto-routing).
- **Both devices persistent**: the internal sink has no external consumers; no
  audio is processed without the app running, so a persistent sink would be
  dead weight and could confuse users browsing device lists.

### Why `/usr/share` not an `/etc` conffile
Vendor PipeWire drop-ins live under `/usr/share/pipewire/pipewire.conf.d/`.
This is read-only vendor config, not user-edited policy, so it is intentionally
NOT marked a dpkg conffile. Users who want to disable it remove the package or
shadow it with a higher-numbered drop-in under `~/.config`.

## Consequences
- Discord/OBS keep their `HonkHonk Mic` selection across app restarts/reboots.
- An idle null-audio-sink uses zero CPU; the device is always visible in audio
  settings after install + a PipeWire restart (or reboot).
- New conf.d files require a PipeWire restart to take effect; reboot does this
  naturally. First install before a reboot needs a manual `systemctl --user
  restart pipewire` — documented for packagers.
- Uninstall must drop the device: the .deb `postrm` restarts the user's
  PipeWire so it stops loading the removed drop-in (best-effort, never fails
  removal).
- Flatpak/AppImage cannot install host conf.d; they rely on the first-run
  fallback (lingering node + per-user conf.d), so behavior is consistent across
  distribution channels.
- The app must tolerate a pre-existing device: it skips creation and only wires
  links, so double-creation and `node.name` collisions cannot occur.
```

- [ ] **Step 2: Commit**

```bash
git add docs/adr/004-persistent-source-conf-d.md
git commit -m "docs(adr): 004 — persistent virtual source via conf.d"
```

---

## Task 8: Full verification gate

- [ ] **Step 1: Full test + lint + build**

Run:
```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
bash tests/packaging/deb_validate.sh
```
Expected: fmt clean, clippy zero warnings, all tests pass, release builds,
validator exits 0.

- [ ] **Step 2: LOC check (CLAUDE.md ≤500 PR)**

Run: `git diff --stat main...HEAD`
Expected: net additions (excluding the ADR doc + plan doc + generated) under
500 LOC. The ADR and plan are docs (not counted toward the code budget per
CLAUDE.md "excluding generated files"); if code LOC approaches the limit,
nothing here should — the change is small. If it balloons, STOP and split.

- [ ] **Step 3:** Proceed to `superpowers:finishing-a-development-branch`, option 2 (Push + PR).

---

## Self-Review notes (author)

- **Spec coverage:** (1) skip-create-when-present → Tasks 2+3+4. (2) create with
  linger=true + write user conf.d when absent → Task 1+4. (3) shutdown destroys
  links + sink only, never the source → Task 4 (source held, never destroyed;
  sink/links already session-scoped). (4) ship conf.d under packaging + wire deb
  → Task 5; AUR → Task 6; flatpak/appimage documented → Task 6. (5) postrm
  PipeWire restart → Task 5. (6) ADR-004 → Task 7. (7) first-run dialog as
  AudioEvent → Task 4. (8) TDD: confd content/path (Task 1), decision fn (Task
  2), dump parser (Task 3), app notice (Task 4). PipeWire-only behavior stays
  out of unit tests.
- **Out of scope (explicit):** internal sink persistence; WirePlumber rules;
  per-app auto-routing (#17); GUI device-management panel. The first-run notice
  is a string field only, not a rendered panel.
- **Placeholders:** none — all code blocks concrete. The only "match existing
  pattern" note is the app test constructor (Task 4 Step 5), which is
  intentional: the file's existing test scaffolding must be reused, not
  reinvented.
```