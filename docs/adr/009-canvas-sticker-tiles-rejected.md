# ADR-009: Canvas Sticker Tiles Rejected — Walk Before Run on UI

## Status: Accepted

## Context

Issue #13 set out to give the sound grid a distinctive visual identity. PR #96 (`feat/issue-13`) shipped a first attempt: each tile replaced its rectangular `button` content with an Iced `canvas::Program` painting a radial-gloss sticker disc, a hash-derived ±3° rotation, one of eight hand-drawn glyphs (Goose, AngryGoose, Boom, Note, Arrow, ScreamFace, Star, Dot), a category label, the sound name, and a hotkey/duration badge. Backed by a new `StickerTone` palette in `theme.rs` and `derive_tone`/`derive_glyph`/`derive_seed` helpers in `sound_tile.rs`.

Three problems surfaced once the PR was exercised at realistic scale:

1. **Per-frame cache invalidation.** `tile_view()` in `sound_grid.rs` constructed `SoundTile { cache: Cache::new() }` on every call to `view()`. Iced calls `view()` after every `Message`, so the canvas cache was thrown away each frame. With 200 tiles in the grid, every frame re-walked roughly 30 path operations per tile (~6000 path ops/frame) — enough to make scrolling visibly stutter on wgpu and lock the CPU on tiny-skia.
2. **Software renderer composition bug.** Under `HONKHONK_RENDERER=software` (tiny-skia), the canvas-in-scrollable combination mis-clipped: the list appeared to scroll behind an invisible rectangle and flickered. Canvas widgets do not get the same dirty-rect / scroll-clip treatment from the compositor that built-in widgets get, and the per-frame cache invalidation amplified the artifact.
3. **No text overflow handling.** `frame.fill_text(Text { ... })` does not wrap, ellipsize, or honor a max width. Long sound names clipped at the canvas edge.

Independently of the technical defects, the aesthetic was rejected on its own merits — the glyphs read as coarse clip-art at the tile's 28–42px disc radius. The PR tried to deliver visual infrastructure, palette, glyph library, hash derivation, and the final styling in one ~889-line change, with no rendering-pipeline groundwork beneath it.

## Decision

Reject PR #96 wholesale. Do not salvage `sound_tile.rs`, `StickerTone`, `sticker_ink()`, or the `derive_*` helpers. Close the PR unmerged. Delete the `feat/issue-13` branch and its worktree.

Pivot future UI work to **infrastructure first, aesthetics last**. Concretely, before any further canvas-based visual work on tiles:

1. Write internal notes on Iced's rendering model — when `view()` runs, how `canvas::Cache` is meant to be held, what scroll-clipping does and does not do for canvas children, and where tiny-skia and wgpu diverge.
2. Formalize the theme framework — promote the current ad-hoc `Hh` trait + `Tone` + `space`/`font`/`radius` modules into one documented `Theme` API surface, with no new visuals.
3. Establish a bench harness for tile grid render at 50, 200, 500 items on both renderers, to anchor any future change against a baseline.
4. Prove a persistent `Cache` pattern at small scale (e.g. now-playing waveform), not on the tile grid.
5. Address text overflow with widget-tree primitives (ellipsis or hover-marquee), not by reaching back into canvas.

Aesthetic exploration resumes only after those are in hand, and only in small sub-MVPs.

## Consequences

- Issue #13 stays open. Its scope narrows to "tile visual identity" once the prerequisites land. The hover-state, playing-ring, and animation work originally deferred to #92 remains deferred and is now blocked on the same prerequisites.
- The `feat/issue-13` branch and PR #96 are discarded. The spec doc at `docs/superpowers/specs/2026-05-17-issue-13-sticker-tiles-static-design.md` was on that branch and dies with it; this ADR is the record.
- Future agents must not re-attempt a canvas sticker tile for the grid without first addressing the cache lifecycle, scroll-clip behavior, and text overflow lessons recorded here. A revival attempt that ignores those lessons is in scope for revert.
- The cost of this revert is one rejected PR. The cost of merging it would have been a perf regression on the most-used surface (the grid) and a follow-on stream of patches chasing each defect in turn. Walking back is cheaper than half-running.
