# Structured Leveled Logging — Design (#154)

**Status:** Approved 2026-06-24.

**Prerequisite:** Lands after #151 (async decode) merges. By then `app.rs` is the
`app/` module (`app/mod.rs` + `app/playback.rs`) and the decode-error site is
`app/playback.rs::handle_decoded`. Rebase the logging branch onto the post-#151
`main` before implementing, and target that structure.

## Goal

Replace HonkHonk's ~42 ad-hoc `eprintln!` calls with structured, **leveled**
logging (error / warn / info / debug) that carries real context — most
importantly the **file path** on a decode failure — and make verbosity
controllable from the environment.

## Motivation

Today the shell output is a flat `eprintln!` stream. Two concrete failures:

- **No levels.** Routine router chatter and genuine errors print identically:
  ```
  honkhonk router: SourceDisconnected { identity: AppIdentity { app_name: None, ... } }
  honkhonk: decode error: missing codec parameters (sample rate or channels)
  ```
  There is no way to quieten the chatter or raise verbosity for debugging.
- **No context.** "decode error: …" never says **which file** failed — useless
  for finding one bad file among hundreds.

## Dependency

`tracing` + `tracing-subscriber` (features: `env-filter`, `fmt`, `ansi`). Both
**MIT**, the tokio-ecosystem standard, zero system deps. Structured fields are
the reason to prefer this over `log` + `env_logger` — they attach the filename
cleanly and set up the future GUI surfacing (#156) and background pipeline (#158).
`cargo deny` must stay green; commit `Cargo.lock`; justify the crate in the PR
(stdlib has no logging facade).

## Architecture

- **`src/logging.rs`** (new lib module), exported as `honkhonk::logging`, with
  `pub fn init()`.
- `init()` builds an `EnvFilter` from the verbosity directive and installs a
  `tracing_subscriber::fmt` subscriber writing to **stderr**.
- Called **first thing in `main()`**, before config load / `pipewire::init()` /
  `gtk::init()`, so even early failures are logged through it.
- A pure helper `log_directive(env: Option<&str>) -> String` returns the env
  value when set, else the default directive. This is the unit-testable seam;
  `init()` reads `HONKHONK_LOG` and passes it through.

## Verbosity

`HONKHONK_LOG` env var (consistent with the existing `HONKHONK_RENDERER`). When
unset, default to **`warn,honkhonk=info`** — our crate at `info`, dependencies
quiet at `warn`. Users override with `HONKHONK_LOG=debug`, `honkhonk=trace`, etc.

## Format

Compact `LEVEL target: message`, written to **stderr**, with **ANSI color only
when `std::io::stderr().is_terminal()`** (clean when piped or redirected), and
**no timestamps** (desktop app, not a server log). Structured fields render
inline (e.g. `file=… error=…`).

## Level mapping

Policy applied across all `eprintln!` sites:

| Level | Used for | Examples |
|-------|----------|----------|
| `error` | failures, with structured context | **decode failure (with file path)**, audio engine error, fatal init (then exit) |
| `warn` | recoverable degradation | config / slots / sound-meta save failures, library scan failure, mic/monitor link-creation failures, shortcut-config failures, directory-picker error |
| `info` | lifecycle milestones | audio engine ready, first-run source notice |
| `debug` | routine chatter | the router event log (`engine.rs` — `SourceDisconnected` etc.) |

Fatal init failures in `main.rs` (GTK / tray / audio engine) log at `error` and
then keep the existing process exit; the message states it is fatal.

Genuinely test-only / placeholder prints (e.g. the `audio/router.rs` "SKIP:
pipewire-test integration not yet implemented" stub) are **excluded** — the plan
verifies each of the 42 sites individually.

### Decode-error path context (the headline fix)

In `app/playback.rs::handle_decoded`, the `Err` branch has the sound `id`, not
the path. Resolve the path from the library and log it:

```rust
let file = self
    .sounds
    .iter()
    .find(|s| s.id == id)
    .map(|s| s.path.display().to_string())
    .unwrap_or_else(|| id.clone()); // fall back to id if rescanned away
tracing::error!(file = %file, error = %e, "decode failed");
```

## Testing

- **Unit:** `log_directive` — `None → "warn,honkhonk=info"`; `Some("debug") → "debug"`.
  (The subscriber install + output is third-party I/O — not unit-tested, per the
  testing philosophy: test our glue, not `tracing` internals.)
- **Manual verification:** run with no env var (info+ visible, router chatter
  hidden); `HONKHONK_LOG=debug` (router chatter appears); pipe stderr to a file
  (no ANSI escapes); trigger a decode failure and confirm the **file path** is in
  the message.
- **Regression guard:** confirm **no `eprintln!` remains** in `src/`
  (`grep -rn 'eprintln!' src` returns nothing) and all existing tests stay green.

## Out of scope

GUI surfacing of errors/notices (#156), per-operation spans / structured tracing
of the audio pipeline, and file/rotating log sinks. This issue is **console
logging only**.

## File organization

`src/logging.rs` (< 120 lines). `main.rs` gains a single
`honkhonk::logging::init();` as its first statement. The site migrations are 1–2
line swaps across the 8 affected files (`main.rs`, `app/mod.rs`,
`app/playback.rs`, `audio/engine.rs`, `audio/streams.rs`, `audio/registry.rs`,
`shortcuts/config_ui.rs`, `shortcuts/portal.rs`).
