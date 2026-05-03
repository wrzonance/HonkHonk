# HonkHonk

Wayland-native Linux soundboard. Rust + Tauri v2 + Svelte + PipeWire.

## Architecture

Read ARCHITECTURE.md for the full spec. Key points:

- Tauri v2 app: Rust backend + Svelte frontend
- PipeWire audio via pipewire-rs (persistent virtual sink, no per-sound node churn)
- Global shortcuts via ashpd (xdg-desktop-portal GlobalShortcuts v2)
- System tray via ksni (StatusNotifierItem — works across KDE, GNOME, Hyprland)
- Audio decoding via symphonia (pure Rust)

## Build

```bash
cargo tauri dev    # Development (hot-reload frontend + Rust rebuild)
cargo tauri build  # Production binary
```

### Build Dependencies

```bash
# Arch / Manjaro
sudo pacman -S rust nodejs npm pkg-config pipewire webkit2gtk-4.1 base-devel

# Fedora
sudo dnf install rust cargo nodejs npm pkg-config pipewire-devel webkit2gtk4.1-devel gcc

# Ubuntu / Debian
sudo apt install rustc cargo nodejs npm pkg-config libpipewire-0.3-dev libwebkit2gtk-4.1-dev build-essential
```

## Project Structure

- `src-tauri/` — Rust backend (PipeWire, shortcuts, tray, IPC commands)
- `src/` — Svelte frontend (sound grid, settings, config panels)
- `static/` — Icons, assets
- `packaging/` — Flatpak, AUR, DEB, RPM, Nix, AppImage build configs

## Conventions

- Rust: strict clippy lints (see below), no unsafe unless absolutely necessary
- Frontend: Svelte 5 runes, TypeScript strict
- No X11 code. Wayland-native only.
- No PulseAudio direct calls. PipeWire only (pulse compat layer handles legacy).
- Commit format: `type(scope): description` (feat, fix, refactor, docs, test, chore)
- Functions: max 50 lines. Files: max 400 lines. Split when approaching limits.

## Error Handling — Context Chains

Every error must carry full context from origin to surface. Never lose the "why" chain.

**Two crates, different roles:**
- `thiserror` — typed error enums at module boundaries (AudioError, PortalError, ConfigError)
- `anyhow` — `.context("what was happening")` at every call site within command handlers / top-level glue

**Rules:**
- Every module (`audio/`, `shortcuts/`, `state/`) exports its own error enum via `thiserror`
- No `String` errors crossing module boundaries. No `.unwrap()` in non-test code. No `panic!()`.
- Use `.context()` or `.with_context(|| format!(...))` at every `?` propagation where the caller adds meaning
- Tauri command handlers catch errors with `anyhow` and surface the full chain to the frontend for debugging

**Example of correct error propagation:**
```rust
// audio/engine.rs
pub fn init() -> Result<AudioEngine, AudioError> {
    let core = connect_pipewire()
        .context("connecting to PipeWire server")?;
    let sink = create_virtual_sink(&core, "HonkHonk Mix")
        .context("creating virtual sink")?;
    let source = create_virtual_source(&core, "HonkHonk Mic")
        .context("creating virtual source")?;
    // ...
}

// When create_virtual_sink fails, the error chain reads:
//   Error: audio engine initialization failed
//   Caused by:
//     0: creating virtual sink
//     1: PipeWire node creation failed
//     2: factory 'support.null-audio-sink' not found
```

**Anti-patterns (will be rejected in review):**
```rust
// BAD: swallows context
let result = do_thing().map_err(|_| "failed")?;

// BAD: unwrap in non-test code
let core = connect_pipewire().unwrap();

// BAD: String error across module boundary
fn play_sound(path: &str) -> Result<(), String> { ... }

// BAD: generic error with no chain
Err(anyhow!("something went wrong"))
```

## Complexity Controls

### Rust Lints (clippy.toml)

```toml
cognitive-complexity-threshold = 10
too-many-arguments-threshold = 5
too-many-lines-threshold = 50
type-complexity-threshold = 200
```

### Cargo Tooling

| Tool | Purpose | When |
|------|---------|------|
| `cargo clippy -- -D warnings` | Lint + complexity | Every CI run |
| `cargo deny check` | Dependency audit (licenses, advisories, duplicates) | Every CI run |
| `cargo machete` | Detect unused dependencies | Weekly / pre-release |
| `cargo tarpaulin` | Code coverage (80% target) | Every CI run |
| `cargo bloat --release` | Binary size breakdown | Pre-release |
| `cargo udeps` | Unused transitive deps | Weekly / pre-release |

### Frontend Linting

```json
// .eslintrc — strict complexity rules
{
  "rules": {
    "complexity": ["error", 10],
    "max-lines-per-function": ["error", 50],
    "max-depth": ["error", 3],
    "max-params": ["error", 4]
  }
}
```

## Module Boundaries

Each Rust module is a self-contained unit with a typed error, a public API, and no leaking internals.

```
src-tauri/src/
├── audio/
│   ├── mod.rs          # pub use, re-exports
│   ├── error.rs        # AudioError enum (thiserror)
│   ├── engine.rs       # PipeWire lifecycle (init, shutdown)
│   ├── decoder.rs      # symphonia file → PCM samples
│   ├── mixer.rs        # Mix mic + playback into virtual sink
│   └── playback.rs     # Play sound to sink + monitor output
├── shortcuts/
│   ├── mod.rs
│   ├── error.rs        # PortalError enum
│   └── portal.rs       # ashpd GlobalShortcuts session
├── tray/
│   ├── mod.rs
│   └── icon.rs         # ksni StatusNotifierItem
├── state/
│   ├── mod.rs
│   ├── error.rs        # ConfigError enum
│   ├── config.rs       # App settings (serde JSON)
│   ├── library.rs      # Sound file index + metadata
│   └── slots.rs        # Hotkey slot ↔ sound mapping
└── commands.rs          # Tauri IPC handlers (thin glue only)
```

**`commands.rs` is glue only.** It calls module APIs and translates errors. No business logic lives here.

## Architecture Decision Records (ADRs)

When a non-obvious decision is made, record it. Future agents and contributors need to know WHY, not just WHAT.

```
docs/adr/
  001-tauri-over-electron.md
  002-pipewire-only-no-pulseaudio.md
  003-fixed-slot-hotkey-model.md
  004-persistent-sink-no-per-sound-nodes.md
  ...
```

**ADR format:**
```markdown
# ADR-NNN: Title

## Status: Accepted | Superseded by ADR-XXX | Deprecated

## Context
What situation prompted this decision?

## Decision
What did we choose?

## Consequences
What trade-offs does this create?
```

Write an ADR when: choosing between two viable approaches, rejecting a popular approach, making a decision that will surprise future readers.

## Scope Creep Prevention

- **Milestones are gates.** Phase 2 does not start until Phase 1 is merged, tagged, and smoke-tested on 3 DEs.
- **Feature requests go to a backlog issue.** They don't get planned until the current phase ships.
- **"Not now" is a valid answer.** If a feature doesn't serve the current phase's sub-MVP, it waits.
- **Every plan states what is OUT of scope** — explicitly. "This PR does NOT add hotkey support" prevents drift.
- **Dependency additions require justification.** New crate/npm package = comment in PR explaining why existing deps or stdlib can't do it.

## Multi-DE Rules

All desktop integration MUST go through xdg-desktop-portal D-Bus APIs:
- Shortcuts: `org.freedesktop.portal.GlobalShortcuts` (via ashpd crate)
- File dialogs: `org.freedesktop.portal.FileChooser` (via ashpd or Tauri)
- Notifications: `org.freedesktop.portal.Notification`
- Background/Autostart: `org.freedesktop.portal.Background`

Never use KDE-specific, GNOME-specific, or compositor-specific APIs directly. The portal abstraction is what makes one binary work on KDE, GNOME, Hyprland, Sway, etc.

System tray uses StatusNotifierItem (SNI) protocol — the cross-DE standard for Wayland. No XEmbed.

## Packaging Rules

- Runtime deps are NOT bundled (except in AppImage/Flatpak). Packages declare deps and let the package manager resolve.
- `packaging/` contains build configs for each format. All must build from the same source tree.
- Tagged releases trigger CI that builds: Flatpak, AUR PKGBUILD, .deb, .rpm, AppImage, Nix flake.
- Test each package format in a clean container before release.

### XDG Compliance

- Config: `$XDG_CONFIG_HOME/honkhonk/` (default `~/.config/honkhonk/`)
- Data: `$XDG_DATA_HOME/honkhonk/` (default `~/.local/share/honkhonk/`)
- No writing to `~/`, `~/.honkhonk`, or any non-XDG path
- .desktop file installed to `$XDG_DATA_HOME/applications/`
- Icons installed to `$XDG_DATA_HOME/icons/hicolor/`

## Development Workflow

### PR Chunking — Sub-MVP Discipline

Every PR must have a single, demonstrable outcome. Not "progress toward Phase 1" — a concrete thing that works when the PR merges.

**Rules:**
- **500 LOC max per PR** (excluding generated files, lockfiles, test fixtures). If a PR exceeds this, split it. No exceptions.
- **Each PR = one sub-MVP.** A sub-MVP is the smallest unit of work that a human reviewer can understand, test, and verify in isolation. Examples:
  - "Tauri v2 + Svelte skeleton builds and shows empty window" — sub-MVP
  - "PipeWire virtual sink creates and appears in `wpctl status`" — sub-MVP
  - "Sound grid renders file list from directory" — sub-MVP
  - "Phase 1 MVP" — NOT a sub-MVP. Too big. Break it down.
- **Each PR has a test plan** in the description. What commands to run, what to visually verify.
- **Each PR must pass CI independently.** No "this breaks but the next PR fixes it."
- **PR title format:** `feat(audio): create persistent PipeWire virtual sink` — scope in parens, imperative description.

### Branch Strategy

```
main                    ← always builds, always works
 └── feat/pipewire-sink ← one sub-MVP per branch
 └── feat/sound-grid    ← branched from main, not from other feature branches
 └── fix/tray-icon-kde  ← fixes also get branches
```

No long-lived feature branches. Branch from main, PR back to main, delete branch.

### TDD — Mandatory for All Code

Every plan and every PR follows Red-Green-Refactor. No implementation code without a failing test first.

**Rust backend:**
1. Write a failing test (`cargo test` → red)
2. Write minimal code to pass (`cargo test` → green)
3. Refactor if needed, tests stay green
4. Commit

**Svelte frontend:**
1. Write component test or integration test (vitest/playwright → red)
2. Implement component (→ green)
3. Refactor, tests stay green
4. Commit

**What gets tested:**
- Audio engine: virtual sink creation, playback routing, mic passthrough, cleanup on shutdown
- Config/state: serialization, slot assignment, library indexing
- Shortcuts: portal registration, slot activation signals
- Frontend: component rendering, IPC command wiring, user interactions
- Integration: full play-sound-to-virtual-mic pipeline (requires PipeWire in CI)

**What does NOT get tested (waste of time):**
- Tauri boilerplate / auto-generated code
- CSS styling
- Third-party library internals

### CI/CD Pipeline

Every push to a PR branch triggers:

```yaml
# .github/workflows/ci.yml
jobs:
  lint:
    - cargo clippy -- -D warnings
    - cargo fmt -- --check
    - npm run check          # Svelte/TS type check
    - npm run lint           # eslint

  test:
    - cargo test
    - npm run test           # vitest

  build:
    - cargo tauri build      # must succeed
    - binary size check      # alert if > 50MB

  loc-check:
    - diff --stat main...HEAD | check LOC delta <= 500
```

Every tagged release (`v*`) triggers the full packaging matrix (see ARCHITECTURE.md).

### Agent Orchestration for Development

This project uses multi-agent workflows. Model selection by task:

| Task | Model | Why |
|------|-------|-----|
| Plan writing, architecture decisions | **Opus** | Deep reasoning, cross-cutting analysis |
| Code implementation | **Sonnet** | Fast, accurate code generation |
| Code review | **Opus** | Catches subtle bugs, architectural drift |
| Test writing | **Sonnet** | Mechanical, pattern-based |
| CI/CD, packaging configs | **Sonnet** | Templated, well-documented formats |
| Debugging / root cause analysis | **Opus** | Requires tracing across layers |

**Parallel agent patterns:**
- Plan writing: architect agent + security reviewer + feasibility checker in parallel
- Implementation: one agent per sub-MVP (isolated worktrees), review agent after each
- Pre-merge: lint agent + test agent + build agent in parallel

**Plan writing rules:**
- Every plan MUST be written before implementation begins
- Plans use TDD structure: test first, then implementation, then verification
- Plans are reviewed by a separate agent before execution
- Plans specify exact files, exact test names, exact commands

## Testing

- `cargo test` — Rust unit tests (audio engine, config, slot logic)
- `cargo clippy -- -D warnings` — lint must pass with zero warnings
- `npm run check` — Svelte/TypeScript type checking
- `npm run test` — vitest for frontend component/integration tests
- Package smoke tests run in CI containers (Arch, Fedora, Ubuntu)
- PipeWire tests require a running PipeWire instance (use `pipewire -c pipewire.conf` in CI or skip with `#[cfg(test)]` mocks)
- **Coverage target: 80%+** for Rust backend. Frontend coverage is best-effort (UI-heavy code is hard to unit test meaningfully).
