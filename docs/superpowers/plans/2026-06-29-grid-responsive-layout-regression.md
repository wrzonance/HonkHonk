# Grid Responsive Layout Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the main sound grid so it uses evenly sized cells at all practical window widths, does not collapse into narrow tall columns, and remains clear of side-panel overlays.

**Architecture:** Keep the existing Iced 0.14 Elm/MVU view structure and the stable root `Stack` used to preserve widget state. Add a small pure grid sizing policy that converts available grid width plus the user density preference into an effective column count. Use Iced `responsive` at the grid boundary to feed actual layout width into that policy. Keep committed tests on HonkHonk-owned layout invariants, not Iced internals.

**Tech Stack:** Rust 2024, Iced 0.14, existing canvas tile renderer, existing unit tests and grid render bench support.

---

## Diagnosis Summary

Current branch: `main` at `1278248` (`[codex] smooth panel feather motion (#184)`). Open draft PRs `#185` and `#186` are not merged into this branch.

Recent PRs inspected:

- `#176` added the side-panel flourish layer.
- `#178` added tile rotation clearance and changed tiles to fit inside the slot they receive.
- `#181` added macro recording UI in the header.
- `#182` and `#184` changed flourish particle behavior only.

Findings:

- The root `Stack` is not the main cause. Iced lays out a `Stack` from child 0, and HonkHonk still keeps the base app view at child 0 before overlay layers.
- The side-panel closed handle is a full-size layer, but it does not constrain the base grid layout. At 1200x900 it lays over only a 28x96 handle on the right.
- `src/ui/sound_grid.rs` still builds rows with a fixed preferred column count from `Density::columns()` (`Regular` is 5).
- `src/ui/tile_layout.rs` now reserves rotation clearance. `SOUND_TILE_H` is 140, but `tile_slot_height()` is 211 for the current constants.
- `sound_tile::view` fills its assigned cell, then `fitted_tile_size()` shrinks the drawn tile to whatever width the parent row gives it. This is correct for rotation containment, but it makes narrow cells visibly severe.
- Existing tests pass, but they do not cover responsive full-view layout:
  - `cargo test incomplete_rows_reserve_all_missing_tile_slots`: passed.
  - `cargo test tile_slot_height_reserves_hover_rotation_clearance`: passed.
  - `cargo test view_builds_in_all_overlay_states`: passed.

Throwaway Iced layout probe, removed after measurement:

```text
window=1200 count=20 grid=1128 row=1128 first_tile=212.8
window=800  count=20 grid=728  row=728  first_tile=132.8
window=500  count=20 grid=428  row=428  first_tile=72.8
window=360  count=20 grid=288  row=288  first_tile=44.8
```

Conclusion:

The grid is even at a wide viewport, but it keeps using 5 columns when the available width is too small. After `#178`, those narrow cells become tall 211px slots with tiny fitted drawings, which reads as a broken vertical stack/overlap. The fix should make the effective column count responsive to actual grid width while preserving the user's density setting as the maximum/preferred column count.

---

## Invariants

- Effective columns are always `1..=preferred_columns`.
- Effective columns decrease when available width cannot support the minimum usable tile cell width.
- Cell widths stay approximately equal within each row.
- If the window is narrower than one usable tile, the grid falls back to one column instead of generating sub-tile columns.
- Incomplete final rows reserve missing slots so existing rows do not stretch last-row tiles.
- Tile slot height remains the rotation-safe height from `tile_layout::tile_slot_height()`.
- The root app `Stack` remains stable: base view first, side-panel/flourish/context/editor overlays after it.
- The grid scrollable keeps its stateful position in the same widget tree slot.
- Permanent tests pin HonkHonk-owned sizing policy and row contracts, not Iced private layout internals.

---

## Task 1: Add A Pure Responsive Column Policy

Files:

- Modify `src/ui/tile_layout.rs`

Add constants and a pure function:

```rust
pub const MIN_TILE_CELL_W: f32 = 180.0;

pub fn responsive_columns(available_width: f32, preferred_columns: usize, spacing: f32) -> usize {
    let preferred = preferred_columns.max(1);
    let usable_width = available_width.max(0.0);
    let columns_that_fit = ((usable_width + spacing) / (MIN_TILE_CELL_W + spacing))
        .floor()
        .max(1.0) as usize;

    preferred.min(columns_that_fit)
}
```

Notes:

- `180.0` is intentionally below the healthy 1200px regular-density cell width (`212.8`) and above the measured broken widths (`44.8`, `72.8`, `132.8`).
- Keep this pure so tests can cover the important behavior without testing Iced rendering.
- If visual review shows `180.0` is too strict, tune it in this constant only.

Tests:

- Add tests in `tile_layout.rs` for the measured widths:
  - `1128px`, preferred 5, spacing 16 -> 5 columns.
  - `728px`, preferred 5, spacing 16 -> 3 columns.
  - `428px`, preferred 5, spacing 16 -> 2 columns.
  - `288px`, preferred 5, spacing 16 -> 1 column.
  - preferred 1 always stays 1.

Validation:

```bash
cargo test responsive_columns
```

---

## Task 2: Make `view_grid` Responsive At The Grid Boundary

Files:

- Modify `src/ui/sound_grid.rs`

Refactor `view_grid` into a small responsive wrapper plus the existing fixed-column builder:

```rust
pub fn view_grid<'a>(
    sounds: Vec<&'a SoundEntry>,
    playing: Option<&'a str>,
    grid: GridCtx<'a>,
) -> Element<'a, Message> {
    iced::widget::responsive(move |size| {
        let columns = tile_layout::responsive_columns(
            size.width,
            grid.columns,
            theme::space::LG,
        );

        view_grid_columns(&sounds, playing, grid, columns)
    })
    .width(Length::Fill)
    .height(Length::Shrink)
    .into()
}
```

Then move the current body of `view_grid` into `view_grid_columns(...)`, replacing its local `let columns = grid.columns.max(1);` with the passed effective column count.

Important details:

- Use `.height(Length::Shrink)` on `responsive`; the grid lives inside a vertical `Scrollable`, and the responsive widget should report the grid content height, not claim infinite fill height.
- Keep the existing filler-slot logic, but use the effective column count.
- Do not replace the grid with a new layout framework in this pass. Iced has `Grid::fluid`, but the current row builder already owns the last-row filler contract and has tests. A responsive wrapper is the smaller fix.

Tests:

- Keep `incomplete_rows_reserve_all_missing_tile_slots`.
- Add or update a pure row-plan test only if the refactor introduces a new helper. Do not assert Iced node internals in committed tests.

Validation:

```bash
cargo test incomplete_rows_reserve_all_missing_tile_slots
cargo test responsive_columns
```

---

## Task 3: Harden The Main View Width Contracts

Files:

- Modify `src/app/mod.rs`

Add explicit fill sizing where the main view currently relies on child size hints:

```rust
let grid_scroll = scrollable(...)
    .width(Length::Fill)
    .height(Length::Fill);

let content = iced::widget::Column::with_children(items)
    .spacing(theme::space::MD)
    .width(Length::Fill)
    .height(Length::Fill);
```

Rationale:

- The throwaway probe showed current wide layout is healthy, but these explicit contracts make the app resilient to future child composition changes.
- This does not change the stable root `Stack` ordering or overlay layering.
- Keep the edit minimal because `src/app/mod.rs` is a known oversized file and should not receive broader refactors here.

Validation:

```bash
cargo test view_builds_in_all_overlay_states
```

---

## Task 4: Run A Local Layout Spike And Revert It

Files:

- Temporarily modify `src/app/mod.rs`, then revert before commit.

Repeat the throwaway layout probe used for diagnosis after Tasks 1-3. The probe should build `app.view()` with 20 sounds and inspect node bounds at these widths:

```text
360, 500, 800, 1200
```

Expected post-fix shape:

```text
window=360  effective_columns=1 first_tile ~= 288
window=500  effective_columns=2 first_tile ~= 206
window=800  effective_columns=3 first_tile ~= 232
window=1200 effective_columns=5 first_tile ~= 212.8
```

Rules:

- This spike is diagnostic only.
- Remove the probe before final verification.
- Do not commit tests that assert Iced private node paths.

---

## Task 5: Manual Visual Verification

Run the app in software mode first so the result is independent of GPU state:

```bash
HONKHONK_RENDERER=software cargo run
```

Check:

- Main grid at narrow, medium, and wide window widths.
- 1, 4, 5, and 20 visible sounds if possible by search/category filtering.
- Effects panel closed and open.
- Side-panel flourish enabled and disabled.
- Regular, Compact, and Comfy density settings.
- Grid scroll position survives opening/closing context menu and side panel.

If a local display is unavailable, record that limitation and rely on the layout spike plus unit tests until someone can run the visual pass.

---

## Task 6: Final Verification

Run:

```bash
cargo fmt --check
cargo test responsive_columns
cargo test incomplete_rows_reserve_all_missing_tile_slots
cargo test view_builds_in_all_overlay_states
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
git status --short --branch
```

Optional but useful because this is a grid rendering regression:

```bash
cargo bench --bench grid_render -- --sample-size 10
```

Expected:

- All targeted and full tests pass.
- Clippy passes with `-D warnings`.
- No throwaway probe remains in the diff.
- `src/app/mod.rs` has only the explicit width/height contract edits.
- `src/ui/sound_grid.rs` remains under the 400-line project limit.
- `src/ui/tile_layout.rs` remains under the 400-line project limit.

---

## Risk Notes

- Changing column count with window width changes grid content height. That is expected. The outer scrollable remains the same widget, so scroll state should be preserved across ordinary view rebuilds.
- The `responsive` widget rebuilds its child during layout. The child tree contains stateless row/mouse/canvas tile widgets, so this is low risk. The search input, scrollable, and overlays stay outside the responsive boundary.
- If visual review finds density should mean an exact column count instead of a maximum, add a separate setting later. For this regression, density should be treated as preferred maximum columns because unusably narrow cells are worse than fewer columns.
