# Persistent canvas::Cache on Now-Playing Waveform — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove the correct persistent-`canvas::Cache` pattern (cache held in persistent state, cleared only on content change — never rebuilt in `view()`) on the low-risk now-playing waveform surface, per ADR-009, to de-risk it for the tile grid (#13/#92).

**Architecture:** A `NowPlaying` cache-owner struct lives in `src/ui/now_playing.rs` and owns the persistent `canvas::Cache` plus a render key (the playing sound id + the progress bucket). It exposes `sync(playing, progress)` which clears the cache **only** when the key changes, and a `canvas::Program` impl whose `draw` reuses the cache for the static waveform bars and paints the moving playhead as a cheap per-frame overlay. `HonkHonk` holds one `now_playing: NowPlaying` field and, in `update`/`view` glue, calls `sync(..)` — no cache-lifecycle business logic enters `app.rs`.

**Tech Stack:** Rust, Iced 0.14 (`iced::widget::canvas`, requires the `canvas` cargo feature), Elm/MVU.

## Global Constraints

- **File size: 400 lines max; functions ≤50 lines.** `src/ui/now_playing.rs` must stay under 400 lines after changes (split a helper module if needed). `src/app.rs` is over budget — add only a minimal field + a single delegating call; **no cache logic in app.rs**.
- **clippy.toml:** cognitive-complexity 10, too-many-arguments 5, too-many-lines 50, type-complexity 200. `cargo clippy --all-targets -- -D warnings` MUST pass clean. `cargo fmt`.
- **Errors:** `thiserror` typed enums per module boundary; `anyhow` `.context(...)` at top-level glue. No `String` errors across module boundaries. **No `.unwrap()` / `panic!()` in non-test code.** (This feature is pure rendering — no fallible IO is expected; do not introduce any.)
- **TDD mandatory:** failing test first. 80% coverage target (`cargo tarpaulin`). **Do NOT test Iced view rendering or `canvas::Program::draw` output / third-party internals** — test the cache-lifecycle decision logic (when the key changes, when `sync` clears) at the module boundary.
- **Iced canvas feature:** `iced::widget::canvas` is feature-gated and NOT in the default feature set; `Cargo.toml` currently has `iced = { features = ["tokio", "tiny-skia"] }`. Add `"canvas"`.
- **Dependency / lockfile rule:** Enabling the `canvas` feature pulls no new crate (it toggles a feature on the already-present `iced_widget`). If `Cargo.lock` nonetheless changes, regenerate `packaging/flatpak/cargo-sources.json` (CI freshness gate, #121) in the same PR.
- **Branch:** all commits go to `feat/issue-131`. Never commit to main.
- **Commit/PR trailer:** `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.

---

## File Structure

- `Cargo.toml` — add `"canvas"` to the `iced` feature list.
- `src/ui/now_playing.rs` — replace the manual `container`-width progress bar with a canvas-backed waveform+progress. Houses the `NowPlaying` cache-owner struct (the cache-lifecycle logic and the `canvas::Program` impl). Must stay < 400 lines.
- `src/ui/waveform.rs` *(new)* — pure, deterministic waveform-sample generation from a sound id, and the `RenderKey` type + bucketing logic. Extracting this keeps `now_playing.rs` under budget and isolates the unit-testable logic from Iced types. Pure Rust, no Iced canvas types.
- `src/ui/mod.rs` — add `pub mod waveform;`.
- `src/app.rs` — add one field `now_playing: NowPlaying` to `HonkHonk` (init in `new` + `new_for_test`), call `self.now_playing.sync(self.playing.as_deref(), self.progress)` once in the existing glue (a single line in `view_main` before building the element, or in `update`), and pass `&self.now_playing` to `view_now_playing`. No other logic.

---

## Task 1: Enable the `canvas` Iced feature

**Files:**
- Modify: `Cargo.toml` (the `iced = { version = "0.14", features = [...] }` line)

**Interfaces:**
- Consumes: nothing.
- Produces: `iced::widget::canvas` becomes usable in later tasks.

- [ ] **Step 1: Add the feature**

In `Cargo.toml`, change the iced dependency line from:

```toml
iced = { version = "0.14", features = ["tokio", "tiny-skia"] }
```

to:

```toml
iced = { version = "0.14", features = ["tokio", "tiny-skia", "canvas"] }
```

- [ ] **Step 2: Verify it builds and lockfile impact**

Run: `cargo build 2>&1 | tail -5 && git status --short Cargo.lock`
Expected: build succeeds. Note whether `Cargo.lock` is listed as modified.

- [ ] **Step 3: Regenerate cargo-sources.json only if Cargo.lock changed**

If `git status --short Cargo.lock` printed a line (lock changed), regenerate the Flatpak sources so the #121 freshness gate passes. Inspect how the repo generates it first:

Run: `ls packaging/flatpak/ && grep -rn "cargo-sources\|flatpak-cargo-generator" .github/workflows/ packaging/ 2>/dev/null | head`

Then regenerate using the method the repo documents (typically a vendored `flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json`). If the lock did NOT change, skip this step.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock packaging/flatpak/cargo-sources.json 2>/dev/null; git add Cargo.toml
git commit -m "build: enable iced canvas feature for now-playing waveform

Canvas is feature-gated in Iced 0.14 and absent from the default set;
the persistent-cache waveform (#131) needs iced::widget::canvas.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Deterministic waveform samples + render key (pure logic)

**Files:**
- Create: `src/ui/waveform.rs`
- Modify: `src/ui/mod.rs` (add `pub mod waveform;` in the module list, alphabetical: after `volume`? keep existing order — append `pub mod waveform;` after `pub mod volume;`)
- Test: inline `#[cfg(test)] mod tests` in `src/ui/waveform.rs`

**Interfaces:**
- Produces:
  - `pub const WAVEFORM_BARS: usize = 48;`
  - `pub fn samples(id: &str) -> [f32; WAVEFORM_BARS];` — deterministic bar heights in `0.15..=1.0` derived from the id hash; identical id ⇒ identical array; differs across ids.
  - `pub const PROGRESS_BUCKETS: u16 = 240;`
  - `pub fn progress_bucket(progress: f32) -> u16;` — quantizes `progress` (clamped 0.0..=1.0) into `0..=PROGRESS_BUCKETS`.
  - `#[derive(Clone, PartialEq, Eq, Debug)] pub struct RenderKey { pub id: Option<String>, pub bucket: u16 }`
  - `pub fn render_key(playing: Option<&str>, progress: f32) -> RenderKey;`

- [ ] **Step 1: Write the failing tests**

Create `src/ui/waveform.rs`:

```rust
//! Pure, deterministic waveform-sample generation and the render-key used to
//! decide when the now-playing `canvas::Cache` must be invalidated (#131).
//!
//! No Iced types live here so the cache-lifecycle decision stays unit-testable
//! without a renderer (ADR-009: prove the persistent-cache pattern first).

use std::hash::{Hash, Hasher};

/// Number of vertical bars in the now-playing waveform.
pub const WAVEFORM_BARS: usize = 48;

/// Quantization resolution for progress. The cache is keyed on the bucket, not
/// the raw float, so identical-looking frames reuse the cached geometry instead
/// of re-tessellating every sub-pixel `progress` tick.
pub const PROGRESS_BUCKETS: u16 = 240;

/// Deterministic bar heights for a sound id, each in `0.15..=1.0`.
pub fn samples(id: &str) -> [f32; WAVEFORM_BARS] {
    std::array::from_fn(|i| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut h);
        (i as u64).hash(&mut h);
        // Map the hash into 0.15..=1.0 so no bar fully disappears.
        let frac = (h.finish() % 1000) as f32 / 1000.0;
        0.15 + frac * 0.85
    })
}

/// Quantizes progress into `0..=PROGRESS_BUCKETS`.
pub fn progress_bucket(progress: f32) -> u16 {
    let p = progress.clamp(0.0, 1.0);
    (p * PROGRESS_BUCKETS as f32).round() as u16
}

/// What the cached waveform depends on. When this changes between frames the
/// cache must be cleared; when it is unchanged the cache is reused verbatim.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RenderKey {
    pub id: Option<String>,
    pub bucket: u16,
}

pub fn render_key(playing: Option<&str>, progress: f32) -> RenderKey {
    RenderKey {
        id: playing.map(str::to_owned),
        bucket: progress_bucket(progress),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_deterministic_for_same_id() {
        assert_eq!(samples("abc123"), samples("abc123"));
    }

    #[test]
    fn samples_differ_across_ids() {
        assert_ne!(samples("abc123"), samples("def456"));
    }

    #[test]
    fn samples_stay_in_visible_range() {
        for v in samples("any-id") {
            assert!((0.15..=1.0).contains(&v), "bar {v} out of range");
        }
    }

    #[test]
    fn progress_bucket_is_monotonic_and_bounded() {
        assert_eq!(progress_bucket(-1.0), 0);
        assert_eq!(progress_bucket(0.0), 0);
        assert_eq!(progress_bucket(1.0), PROGRESS_BUCKETS);
        assert_eq!(progress_bucket(2.0), PROGRESS_BUCKETS);
        assert!(progress_bucket(0.25) <= progress_bucket(0.5));
    }

    #[test]
    fn tiny_progress_changes_share_a_bucket() {
        // Sub-bucket jitter must NOT change the key (else the cache thrashes).
        assert_eq!(progress_bucket(0.5000), progress_bucket(0.5001));
    }

    #[test]
    fn render_key_changes_with_sound_and_bucket() {
        let a = render_key(Some("s1"), 0.0);
        let b = render_key(Some("s2"), 0.0);
        let c = render_key(Some("s1"), 1.0);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, render_key(Some("s1"), 0.0));
    }

    #[test]
    fn render_key_none_when_idle() {
        assert_eq!(render_key(None, 0.7).id, None);
    }
}
```

Add to `src/ui/mod.rs` after the existing `pub mod volume;` line:

```rust
pub mod waveform;
```

- [ ] **Step 2: Run tests to verify they pass (logic is written inline above)**

Run: `cargo test --lib ui::waveform 2>&1 | tail -20`
Expected: all `ui::waveform::tests::*` PASS.

> Note: because this task's implementation and tests are written together (the logic is short and pure), the RED step is the compile-failure of the `mod waveform;` reference before the file exists. If you prefer strict RED→GREEN, comment out the function bodies first, watch the tests fail, then restore. Either way, end with all tests green.

- [ ] **Step 3: Lint**

Run: `cargo clippy --lib -- -D warnings 2>&1 | tail -10 && cargo fmt`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/ui/waveform.rs src/ui/mod.rs
git commit -m "feat(ui): deterministic waveform samples + cache render-key

Pure, Iced-free logic for the now-playing waveform: per-id bar heights
and the RenderKey (sound id + quantized progress bucket) that decides
when the canvas::Cache is invalidated. Bucketing prevents per-frame
cache thrash from sub-pixel progress jitter. (#131)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `NowPlaying` cache-owner with the sync/clear lifecycle

**Files:**
- Modify: `src/ui/now_playing.rs` (add the `NowPlaying` struct, its `sync` method, and a `tracking-test`-friendly accessor)
- Test: inline `#[cfg(test)] mod tests` in `src/ui/now_playing.rs`

**Interfaces:**
- Consumes (from Task 2): `crate::ui::waveform::{RenderKey, render_key}`.
- Produces:
  - `pub struct NowPlaying { cache: iced::widget::canvas::Cache, key: Option<crate::ui::waveform::RenderKey> }`
  - `impl Default for NowPlaying`
  - `pub fn sync(&mut self, playing: Option<&str>, progress: f32) -> bool;` — recomputes the key; if it differs from the stored key, clears the cache, stores the new key, returns `true` (cleared); otherwise returns `false` (reused). **This is the cache-lifecycle decision the whole issue is about.**
  - `#[cfg(test)] pub(crate) fn current_key(&self) -> Option<&RenderKey>` — read access for tests only.

- [ ] **Step 1: Write the failing test**

Add to the top of `src/ui/now_playing.rs` imports:

```rust
use iced::widget::canvas;
use crate::ui::waveform::{render_key, RenderKey};
```

Add the struct + impl (place above `view_now_playing`):

```rust
/// Owns the persistent waveform `canvas::Cache` and the key describing what is
/// currently cached. Held in app state across frames; the cache is cleared
/// **only** when [`NowPlaying::sync`] observes a key change — never rebuilt in
/// `view()`. This is the persistent-cache pattern ADR-009 requires proving
/// before the tile grid (#13/#92).
#[derive(Default)]
pub struct NowPlaying {
    cache: canvas::Cache,
    key: Option<RenderKey>,
}

impl NowPlaying {
    /// Reconciles the cache with the current playback state. Returns `true` when
    /// the cached geometry was invalidated (content changed), `false` when the
    /// existing cache is reused. Call once per update/view glue — the only place
    /// the cache is ever cleared.
    pub fn sync(&mut self, playing: Option<&str>, progress: f32) -> bool {
        let next = render_key(playing, progress);
        if self.key.as_ref() == Some(&next) {
            return false;
        }
        self.cache.clear();
        self.key = Some(next);
        true
    }

    #[cfg(test)]
    pub(crate) fn current_key(&self) -> Option<&RenderKey> {
        self.key.as_ref()
    }
}
```

Add tests inside the `#[cfg(test)] mod tests` block (create the block if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sync_clears_and_stores_key() {
        let mut np = NowPlaying::default();
        assert!(np.current_key().is_none());
        let cleared = np.sync(Some("s1"), 0.0);
        assert!(cleared, "first sync must populate the cache key");
        assert!(np.current_key().is_some());
    }

    #[test]
    fn same_state_reuses_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.5);
        let cleared = np.sync(Some("s1"), 0.5);
        assert!(!cleared, "identical state must reuse the cache, not clear it");
    }

    #[test]
    fn sub_bucket_progress_jitter_reuses_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.5000);
        // A tiny progress tick within the same bucket must NOT thrash the cache.
        let cleared = np.sync(Some("s1"), 0.5001);
        assert!(!cleared, "sub-bucket jitter must not invalidate the cache");
    }

    #[test]
    fn changing_sound_clears_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.5);
        assert!(np.sync(Some("s2"), 0.5), "new sound must invalidate cache");
    }

    #[test]
    fn crossing_a_progress_bucket_clears_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.0);
        assert!(np.sync(Some("s1"), 1.0), "large progress jump must invalidate");
    }

    #[test]
    fn stopping_playback_clears_cache() {
        let mut np = NowPlaying::default();
        np.sync(Some("s1"), 0.5);
        assert!(np.sync(None, 0.0), "stopping must invalidate the cache");
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib ui::now_playing 2>&1 | tail -20`
Expected: all `ui::now_playing::tests::*` PASS. (The `view_now_playing` fn is untouched in this task and still compiles.)

- [ ] **Step 3: Lint + fmt**

Run: `cargo clippy --lib -- -D warnings 2>&1 | tail -10 && cargo fmt`
Expected: clean. (If `canvas` import is unused-warned because `view_now_playing` doesn't use it yet, that's resolved in Task 4 — if clippy fails on unused import here, add `#[allow(unused_imports)]` is NOT acceptable; instead reorder so Task 4 lands in the same commit OR temporarily reference `canvas::Cache` only via the struct field, which already uses it. The struct field uses `canvas::Cache`, so the import IS used — no allow needed.)

- [ ] **Step 4: Commit**

```bash
git add src/ui/now_playing.rs
git commit -m "feat(ui): NowPlaying owns the persistent waveform canvas::Cache

The cache lives in this module's own state and is cleared ONLY when
sync() observes a render-key change (different sound or a crossed
progress bucket) — never rebuilt in view(). This is the persistent-cache
lifecycle ADR-009 mandates proving on a low-risk surface before #13/#92.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Canvas `Program` — draw waveform from cache + playhead overlay

**Files:**
- Modify: `src/ui/now_playing.rs` (add the `canvas::Program` impl; replace `view_progress_bar` usage with the canvas widget; remove the now-dead `view_progress_bar` fn)
- Test: none new (rendering output is explicitly out of scope per Global Constraints; the lifecycle is already covered by Task 3). Keep the existing `view_builds_*` smoke coverage path green.

**Interfaces:**
- Consumes (Task 2 + 3): `waveform::{samples, WAVEFORM_BARS}`, `NowPlaying`.
- Produces: `view_now_playing` now takes `&'a NowPlaying` and renders the waveform via `canvas::Canvas`. The waveform bars are drawn through `cache.draw(..)` (cached); the playhead/progress fill is drawn per-frame as a cheap overlay that does NOT touch the cached bar geometry.

- [ ] **Step 1: Add the `Program` impl**

Add to `src/ui/now_playing.rs` (below the `NowPlaying` impl):

```rust
/// Canvas program for the now-playing waveform. `draw` paints the static bars
/// through the persistent cache (reused frame-to-frame) and overlays the moving
/// playhead separately, so the expensive bar tessellation happens once per
/// sound — not once per frame (the ADR-009 anti-pattern PR #96 hit).
struct WaveformProgram<'a> {
    cache: &'a canvas::Cache,
    samples: [f32; crate::ui::waveform::WAVEFORM_BARS],
    progress: f32,
    bar: iced::Color,
    bar_dim: iced::Color,
    accent: iced::Color,
}

impl<Message> canvas::Program<Message> for WaveformProgram<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::widget::canvas::{Path, Stroke};
        use iced::{Point, Size};

        let n = self.samples.len() as f32;
        let gap = 2.0;
        let bar_w = ((bounds.width - gap * (n - 1.0)) / n).max(1.0);

        // Static bars: drawn once and reused. `Cache::draw` only re-runs this
        // closure when the cache was cleared (NowPlaying::sync on a key change).
        let played_to = (self.progress.clamp(0.0, 1.0) * bounds.width).round();
        let bars = self.cache.draw(renderer, bounds.size(), |frame| {
            for (i, &h) in self.samples.iter().enumerate() {
                let x = i as f32 * (bar_w + gap);
                let bh = (h * bounds.height).max(1.0);
                let y = (bounds.height - bh) / 2.0;
                let color = if x <= played_to { self.bar } else { self.bar_dim };
                frame.fill_rectangle(Point::new(x, y), Size::new(bar_w, bh), color);
            }
        });

        // Playhead overlay: a thin accent line at the progress position, drawn
        // fresh each frame WITHOUT invalidating the cached bars above.
        let mut overlay = canvas::Frame::new(renderer, bounds.size());
        let line = Path::line(
            Point::new(played_to, 0.0),
            Point::new(played_to, bounds.height),
        );
        overlay.stroke(&line, Stroke::default().with_color(self.accent).with_width(2.0));

        vec![bars, overlay.into_geometry()]
    }
}
```

> Design note: bars left of the playhead use the brighter `bar` color, right use `bar_dim`. Because color depends on `played_to`, crossing a progress **bucket** is exactly when the visual changes — which is why Task 2 keys the cache on the bucket. Within a bucket the bars are pixel-identical, so the cache is correctly reused; the thin playhead line moves via the cheap overlay.

- [ ] **Step 2: Rewrite `view_now_playing` to use the canvas, drop the manual progress bar**

Replace the `view_now_playing` signature and body so it takes `&'a NowPlaying` and swaps `view_progress_bar(progress, t)` for the canvas widget. Replace the existing function with:

```rust
pub fn view_now_playing<'a>(
    now_playing: &'a NowPlaying,
    playing: Option<&'a str>,
    sounds: &'a [SoundEntry],
    progress: f32,
    vol: f32,
) -> Element<'a, Message> {
    let t = Theme::Dark;

    let sound = match playing.and_then(|id| sounds.iter().find(|s| s.id == id)) {
        Some(s) => s,
        None => return Space::new().into(),
    };

    let content = row![
        view_placeholder(t),
        view_sound_info(sound, t),
        view_waveform(now_playing, &sound.id, progress, t),
        space::horizontal(),
        volume::view_volume(vol),
    ]
    .spacing(theme::space::LG)
    .align_y(iced::Alignment::Center);

    container(content)
        .width(Length::Fill)
        .padding([theme::space::MD, theme::space::XL])
        .style(move |_theme| container::Style {
            background: Some(theme::bg_color(t.panel())),
            border: Border {
                color: t.hairline(),
                width: 1.0,
                radius: iced::border::Radius::default(),
            },
            ..Default::default()
        })
        .into()
}

/// Builds the canvas widget backed by the persistent cache. The waveform samples
/// are derived per-sound; the cache itself is owned by `now_playing` and reused
/// across frames (cleared only by `NowPlaying::sync`).
fn view_waveform<'a>(
    now_playing: &'a NowPlaying,
    id: &str,
    progress: f32,
    t: Theme,
) -> Element<'a, Message> {
    use crate::ui::waveform;
    let program = WaveformProgram {
        cache: &now_playing.cache,
        samples: waveform::samples(id),
        progress,
        bar: t.accent(),
        bar_dim: t.ink_faint(),
        accent: t.ink(),
    };
    canvas(program)
        .width(320.0)
        .height(theme::component::ARTWORK_SQ)
        .into()
}
```

Delete the now-unused `view_progress_bar` function entirely. Add `use iced::widget::canvas;` (already added in Task 3) and `use crate::ui::theme::Hh;` if `t.accent()`/`t.ink_faint()`/`t.ink()` are not already in scope (check existing imports — `Hh` is used elsewhere in the file already via `t.panel()`, so it is in scope).

> The `cache` field on `NowPlaying` is private to the module; `view_waveform` lives in the same module so it can borrow `&now_playing.cache` directly. This is why the program impl and the view both live in `now_playing.rs`.

- [ ] **Step 3: Build**

Run: `cargo build 2>&1 | tail -20`
Expected: compiles. If `canvas(program)` errors on trait bounds, confirm `WaveformProgram` implements `canvas::Program<Message>` for the concrete `Message` (the impl is generic over `Message` so it covers `crate::app::Message`).

- [ ] **Step 4: Confirm now_playing.rs is under the 400-line cap**

Run: `wc -l src/ui/now_playing.rs`
Expected: < 400. If over, extract `WaveformProgram` + `view_waveform` into a sibling `src/ui/now_playing_canvas.rs` (re-exported), keeping each file focused. (Likely still under; check.)

- [ ] **Step 5: Lint + fmt**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -20 && cargo fmt --check`
Expected: clean. Watch for `too-many-lines` on `draw` (≤50) and `too-many-arguments` (≤5) on `WaveformProgram` — it's a struct, fine; on `view_now_playing` it has 5 args (at the limit, OK) and `view_waveform` has 4 (OK).

- [ ] **Step 6: Commit**

```bash
git add src/ui/now_playing.rs
git commit -m "feat(ui): render now-playing waveform via cached canvas

Static bars draw through the persistent cache (one tessellation per
sound); the playhead is a per-frame overlay that never invalidates the
cached bars. Replaces the manual container-width progress bar. (#131)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Wire `NowPlaying` into `HonkHonk` (minimal app.rs glue)

**Files:**
- Modify: `src/app.rs` — add field, init in two constructors, one `sync` call, update the `view_now_playing` call args.
- Modify: `benches/support/mod.rs` only if it references `view_now_playing` (it does not — it uses `view_grid`; skip).

**Interfaces:**
- Consumes: `crate::ui::now_playing::NowPlaying`, `NowPlaying::sync`.
- Produces: app holds `now_playing: NowPlaying`; renders the cached waveform.

- [ ] **Step 1: Add the field to the struct**

In `src/app.rs`, in `pub struct HonkHonk { ... }` (around line 159, after `editor_draft_volume`), add:

```rust
    /// Persistent now-playing waveform cache owner (#131). App holds it but all
    /// cache-lifecycle logic lives in `ui::now_playing::NowPlaying`.
    now_playing: crate::ui::now_playing::NowPlaying,
```

- [ ] **Step 2: Initialize it in both constructors**

In `HonkHonk::new(..)` Self literal (after `editor_draft_volume: 1.0,`) add:

```rust
            now_playing: crate::ui::now_playing::NowPlaying::default(),
```

In `HonkHonk::new_for_test()` Self literal (after `editor_draft_volume: 1.0,`) add the same line:

```rust
            now_playing: crate::ui::now_playing::NowPlaying::default(),
```

- [ ] **Step 3: Sync the cache + pass it to the view**

In `view_main` (around line 1214), replace the `view_now_playing` call. Note `view_main` takes `&self`, so the `sync` (which needs `&mut`) must happen in `update`, not in `view`. Add the sync to `update`'s tail instead.

First, in `view_main`, change the call to pass the cache and reorder args to match Task 4's new signature:

```rust
        let now_playing = now_playing::view_now_playing(
            &self.now_playing,
            self.playing.as_deref(),
            &self.sounds,
            self.progress,
            self.config.volume,
        );
```

Then add the single sync call in `update`. Find the end of `pub fn update(&mut self, message: Message) -> Task<Message>` (line 454) — just before its final `return`/tail `Task`. The cleanest seam: sync right after the message match, before returning. Locate the `update` function's final returned `Task` and insert immediately before it:

```rust
        // Keep the now-playing waveform cache in step with playback state.
        // Single delegating call — all lifecycle logic lives in NowPlaying.
        self.now_playing.sync(self.playing.as_deref(), self.progress);
```

> If `update` has many early `return`s, instead wrap: capture the task into a `let task = match message { .. };`, then `self.now_playing.sync(self.playing.as_deref(), self.progress); task`. Inspect `update`'s structure first (`sed -n '454,520p' src/app.rs` and its tail) and choose the seam that runs on every update without duplicating the call. The sync must run after `self.playing`/`self.progress` are mutated by the message handler.

- [ ] **Step 4: Build + full test suite**

Run: `cargo build 2>&1 | tail -10 && cargo test 2>&1 | tail -25`
Expected: build clean; all tests pass (including the existing `view_builds_in_all_overlay_states` smoke test, which now exercises the canvas widget construction path).

- [ ] **Step 5: Lint + fmt + app.rs line budget sanity**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -20 && cargo fmt --check && wc -l src/app.rs`
Expected: clean. `app.rs` grew by ~6 lines only (field + 2 inits + 1 sync + arg reorder) — acceptable per the "minimal field that delegates" allowance; no logic added.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat(ui): hold persistent now-playing cache in app state

HonkHonk owns a NowPlaying field and makes one delegating sync() call in
update() so the waveform cache tracks playback. No cache logic in app.rs
(it is over the 400-line budget) — all of it lives in ui::now_playing.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: Verify cache reuse on both renderers (acceptance evidence)

**Files:**
- Test: add a focused test in `src/ui/now_playing.rs` proving the **sync-driven reuse contract** (the observable proxy for "no per-frame re-tessellation"), plus a manual two-renderer smoke check documented for the PR.

**Interfaces:**
- Consumes: `NowPlaying::sync`.
- Produces: a regression test pinning the reuse behavior, and PR evidence.

- [ ] **Step 1: Write the reuse-contract regression test**

Add to `src/ui/now_playing.rs` tests:

```rust
    #[test]
    fn steady_playback_frames_reuse_cache() {
        // Simulate 60 consecutive frames of the SAME sound at the SAME progress
        // bucket (the common idle-render case Iced triggers on every Message).
        // Exactly one clear (the first); the rest reuse — proving the cache is
        // NOT rebuilt per frame (the ADR-009 anti-pattern).
        let mut np = NowPlaying::default();
        let mut clears = 0;
        for _ in 0..60 {
            if np.sync(Some("s1"), 0.5) {
                clears += 1;
            }
        }
        assert_eq!(clears, 1, "only the first frame may clear; rest reuse");
    }

    #[test]
    fn smooth_progress_clears_once_per_bucket_not_per_frame() {
        // Advance progress smoothly across one bucket worth of frames; the cache
        // clears at most once per bucket boundary, never every frame.
        use crate::ui::waveform::PROGRESS_BUCKETS;
        let mut np = NowPlaying::default();
        let step = 1.0 / (PROGRESS_BUCKETS as f32 * 4.0); // 4 frames per bucket
        let mut clears = 0;
        let mut p = 0.0;
        for _ in 0..16 {
            if np.sync(Some("s1"), p) {
                clears += 1;
            }
            p += step;
        }
        // 16 frames spanning ~4 buckets ⇒ far fewer clears than frames.
        assert!(clears <= 5, "got {clears} clears in 16 frames — cache thrashing");
    }
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --lib ui::now_playing 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 3: Manual two-renderer smoke check (record output for PR)**

Build and launch under each renderer; confirm the now-playing waveform renders and the playhead animates without stutter. The app needs a sound playing — use whatever the dev setup provides, or just confirm the bar appears for a playing sound.

Run (wgpu default):
```bash
cargo build --release 2>&1 | tail -3
```
Then document in the PR that the waveform renders on wgpu (default) and on software:
```bash
HONKHONK_RENDERER=software cargo run 2>&1 | head -5   # confirm no canvas-clip panic/flicker on tiny-skia
```

> If a GPU/display is unavailable in this environment, record that the automated reuse-contract tests (Steps 1–2) stand in for the per-frame-reuse acceptance and that the bench harness (`benches/grid_render.rs`) already proves canvas layout+draw works headless on tiny-skia. Note the limitation honestly in the PR rather than claiming a manual run that did not happen.

- [ ] **Step 4: Full verification gate**

Run:
```bash
cargo fmt --check && \
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5 && \
cargo test 2>&1 | tail -15
```
Expected: all clean / all pass.

- [ ] **Step 5: Commit**

```bash
git add src/ui/now_playing.rs
git commit -m "test(ui): pin now-playing cache reuse across steady frames

Regression tests prove the cache clears once (not per frame) for steady
playback and at most once per progress bucket while advancing — the
observable contract behind 'no per-frame re-tessellation' (#131).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Document the reusable pattern for #13 / #92

**Files:**
- Modify: `docs/adr/009-canvas-sticker-tiles-rejected.md` — append a short "Persistent-cache pattern proven (#131)" note pointing at `now_playing.rs` as the reference implementation. (This is the ADR's own decision step 4; updating it closes the loop. Keep it brief — a paragraph + the key rule.)

**Interfaces:**
- Consumes: the shipped `NowPlaying` pattern.
- Produces: documented guidance the #13/#92 implementer follows.

- [ ] **Step 1: Append the note to ADR-009**

Add at the end of `docs/adr/009-canvas-sticker-tiles-rejected.md`:

```markdown

## Update: persistent-cache pattern proven (#131)

Decision step 4 ("prove a persistent `Cache` pattern at small scale") is done.
`src/ui/now_playing.rs` is the reference. The rule, for #13/#92:

1. The `canvas::Cache` lives in **persistent state** (a struct field), never
   `Cache::new()` inside `view()`. Here `NowPlaying` owns it; the app holds one
   `NowPlaying` field.
2. A **render key** captures everything the cached geometry depends on (here:
   sound id + a *quantized* progress bucket). `NowPlaying::sync` clears the cache
   **only** when the key changes — called once from `update()`, never `view()`.
3. Quantize continuous inputs (progress) into buckets so sub-pixel ticks do not
   thrash the cache.
4. Draw cheap, frequently-moving elements (the playhead) as a **separate
   per-frame overlay** that does not invalidate the cached geometry.

For the tile grid (#13): the grid's `Cache` belongs in app state keyed on the
filtered-sound set + density, cleared on filter/density change — not rebuilt per
`view()`. The text-overflow and tiny-skia scroll-clip lessons above still stand
and are out of scope for #131.
```

- [ ] **Step 2: Sanity check the doc**

Run: `tail -25 docs/adr/009-canvas-sticker-tiles-rejected.md`
Expected: the note is present and well-formed.

- [ ] **Step 3: Commit**

```bash
git add docs/adr/009-canvas-sticker-tiles-rejected.md
git commit -m "docs(adr-009): record proven persistent-cache pattern for #13/#92

Closes ADR-009 decision step 4 — now_playing.rs is the reference impl
for the persistent canvas::Cache lifecycle the tile grid must follow.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Final Verification (before PR)

- [ ] `cargo build` clean
- [ ] `cargo build --release` clean
- [ ] `cargo test` all pass
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] `wc -l src/ui/now_playing.rs` < 400
- [ ] `git diff main...HEAD --stat` — app.rs grew only minimally (no logic)
- [ ] If `Cargo.lock` changed in Task 1, `packaging/flatpak/cargo-sources.json` was regenerated in the same commit
- [ ] Coverage: `cargo tarpaulin` — waveform + NowPlaying logic covered (≥80% on new code)

Then: `superpowers:finishing-a-development-branch` → option 2 (Push + PR). PR body includes `Closes #131`, a `## Design decisions` section, and `## Testing` checkboxes.
