# Design — Issue #13 sub-MVP: canvas sticker tiles (static)

**Date:** 2026-05-17
**Branch:** `feat/issue-13-sticker-tiles`
**Closes:** part of #13 (static visual); animations + state rings tracked in #92

## Goal

Replace the current basic-button tile rendering in `src/ui/sound_grid.rs` with a `canvas::Program`-based sticker tile: radial-gradient gloss disc, deterministic ±3° rotation, 8 hand-drawn glyph primitives, favorite-star indicator, and an 8-color Tone palette. Tile visuals derive deterministically from the sound id so the same sound looks identical across restarts. Hover and playing-state animations are deferred to a follow-up PR (#92) to stay under the CLAUDE.md 500-LOC ceiling.

## Scope

### In

| File | Purpose | LOC est. |
|---|---|---|
| `src/ui/sound_tile.rs` (new) | `SoundTile` canvas::Program, `SoundTileData` struct, `Glyph` enum (8 variants), 8 glyph paint helpers, sticker disc + favorite star + label rendering, hash-derived tone/glyph/seed utilities | ~340 |
| `src/ui/theme.rs` | Append `Tone` enum (Pink/Mint/Lemon/Sky/Lilac/Coral/Peach/Sage); `sticker_fill(Tone)`, `sticker_gloss(Tone)`, `ink()` accessors | ~40 |
| `src/ui/sound_grid.rs` | Replace existing per-slot tile widget with `canvas(SoundTile::new(...))` wrapped in existing `mouse_area` click handler; build `SoundTileData` at view time from slot state | ~20 (net delta) |
| `src/ui/mod.rs` | `mod sound_tile;` + `pub use sound_tile::{SoundTile, SoundTileData, Glyph};` | ~2 |
| Unit tests (in `sound_tile.rs`) | Determinism of derive_* + distribution sanity + rotation-math bounds + Tone palette coverage | ~60 |

**Total: ~462 LOC.** Under CLAUDE.md 500 LOC ceiling.

`docs/design-reference/src-rust/ui/sound_tile.rs` (420 LOC) is the source of truth for the visual implementation. PR1 adapts the static portions only — animation paths and state-driven rings are skipped here and ship in #92.

### Out (explicit — tracked in #92)

- Hover rotation amplification (idle ±3° → hover ±8°)
- Hover ink ring around sticker disc
- Playing state accent ring (Tone-derived)
- Playing state outer glow / drop shadow
- Animation easing / timing (~150ms ease-out)
- Right-click context menu changes (already exists in `sound_grid.rs::context_menu_overlay`)
- User-configurable per-sound `tone` / `glyph` override (future `state.rs` change)
- Hotkey badge rendering on tile face (covered by `slot_manager.rs`)
- Visual snapshot / pixel-diff tests

## Architecture

### `src/ui/sound_tile.rs`

```rust
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size};
use crate::ui::theme::{Theme, Tone};

#[derive(Debug, Clone)]
pub struct SoundTileData {
    pub id: String,
    pub name: String,
    pub category: String,
    pub tone: Tone,                  // derived from id hash via `derive_tone`
    pub duration_secs: f32,
    pub hotkey: Option<String>,
    pub favorite: bool,
    pub seed: u32,                   // derived from id hash via `derive_seed`
    pub glyph: Glyph,                // derived from id hash via `derive_glyph`
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph { Goose, AngryGoose, Boom, Note, Arrow, ScreamFace, Star, Dot }

pub struct SoundTile<'a> {
    data: &'a SoundTileData,
    theme: &'a Theme,
    cache: &'a Cache,
}

impl<'a> SoundTile<'a> {
    pub fn new(data: &'a SoundTileData, theme: &'a Theme, cache: &'a Cache) -> Self {
        Self { data, theme, cache }
    }
}

impl<Message, Theme, Renderer> canvas::Program<Message, Theme, Renderer> for SoundTile<'_>
where
    Renderer: iced::advanced::graphics::geometry::Renderer,
{
    type State = ();
    fn draw(
        &self, _state: &(), renderer: &Renderer, _theme: &Theme,
        bounds: Rectangle, _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            paint_sticker(frame, self.data, self.theme);
            paint_glyph(frame, self.data, self.theme);
            if self.data.favorite {
                paint_favorite_star(frame, self.theme);
            }
            paint_label(frame, self.data, self.theme);
        });
        vec![geom]
    }
}

// Paint primitives ------------------------------------------------------

fn paint_sticker(frame: &mut Frame, data: &SoundTileData, theme: &Theme) {
    let center = Point::new(frame.width() / 2.0, frame.height() / 2.0);
    let radius = (frame.width().min(frame.height()) / 2.0) * 0.78;
    let angle_deg = rotation_for_seed(data.seed);

    frame.translate(iced::Vector::new(center.x, center.y));
    frame.rotate(angle_deg.to_radians());

    // Radial gradient fill: sticker_fill at center → sticker_gloss at edge
    let disc = Path::circle(Point::ORIGIN, radius);
    let fill = iced::widget::canvas::Fill {
        style: iced::widget::canvas::Style::Solid(theme.sticker_fill(data.tone)),
        ..Default::default()
    };
    frame.fill(&disc, fill);
    // Gloss highlight (lighter inner disc, offset upward)
    let gloss_disc = Path::circle(Point::new(0.0, -radius * 0.25), radius * 0.55);
    frame.fill(&gloss_disc, theme.sticker_gloss(data.tone));
    // Outline
    frame.stroke(&disc, Stroke::default().with_color(theme.ink()).with_width(2.5));

    // Reset transform; subsequent paints use untranslated coords.
    frame.rotate(-(angle_deg.to_radians()));
    frame.translate(iced::Vector::new(-center.x, -center.y));
}

fn paint_glyph(frame: &mut Frame, data: &SoundTileData, theme: &Theme) {
    match data.glyph {
        Glyph::Goose      => paint_goose(frame, theme),
        Glyph::AngryGoose => paint_angry_goose(frame, theme),
        Glyph::Boom       => paint_boom(frame, theme),
        Glyph::Note       => paint_note(frame, theme),
        Glyph::Arrow      => paint_arrow(frame, theme),
        Glyph::ScreamFace => paint_scream_face(frame, theme),
        Glyph::Star       => paint_star(frame, theme),
        Glyph::Dot        => paint_dot(frame, theme),
    }
}

// 8 glyph helpers — each builds Path of strokes/fills using theme.ink().
// Adapted from docs/design-reference/src-rust/ui/sound_tile.rs.
fn paint_goose(frame: &mut Frame, theme: &Theme) { /* ~25 LOC */ }
// ... 7 more

fn paint_favorite_star(frame: &mut Frame, theme: &Theme) {
    // Small star path in top-right corner, filled with accent color.
}

fn paint_label(frame: &mut Frame, data: &SoundTileData, theme: &Theme) {
    // Text under the sticker — sound name, truncated.
}

// Hash-derived assignment ----------------------------------------------

pub fn derive_tone(sound_id: &str) -> Tone {
    const TONES: [Tone; 8] = [Tone::Pink, Tone::Mint, Tone::Lemon, Tone::Sky,
                              Tone::Lilac, Tone::Coral, Tone::Peach, Tone::Sage];
    TONES[hash(sound_id) % 8]
}

pub fn derive_glyph(sound_id: &str) -> Glyph {
    const GLYPHS: [Glyph; 8] = [Glyph::Goose, Glyph::AngryGoose, Glyph::Boom,
                                Glyph::Note, Glyph::Arrow, Glyph::ScreamFace,
                                Glyph::Star, Glyph::Dot];
    GLYPHS[hash(sound_id).wrapping_add(31) % 8]   // offset so tone+glyph aren't always paired
}

pub fn derive_seed(sound_id: &str) -> u32 {
    hash(sound_id) as u32
}

fn rotation_for_seed(seed: u32) -> f32 {
    // Map u32 → angle in degrees within [-3.0, +3.0].
    let frac = (seed as f64) / (u32::MAX as f64);   // 0.0..=1.0
    (frac * 6.0 - 3.0) as f32
}

fn hash(s: &str) -> usize {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish() as usize
}
```

### `src/ui/theme.rs` extension

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone { Pink, Mint, Lemon, Sky, Lilac, Coral, Peach, Sage }

impl Theme {
    pub fn sticker_fill(&self, tone: Tone) -> Color {
        match tone {
            Tone::Pink   => Color::from_rgb8(0xff, 0xc1, 0xd6),
            Tone::Mint   => Color::from_rgb8(0xc1, 0xf0, 0xd6),
            Tone::Lemon  => Color::from_rgb8(0xff, 0xf0, 0xa8),
            Tone::Sky    => Color::from_rgb8(0xa8, 0xd6, 0xff),
            Tone::Lilac  => Color::from_rgb8(0xd6, 0xc1, 0xff),
            Tone::Coral  => Color::from_rgb8(0xff, 0xa8, 0x96),
            Tone::Peach  => Color::from_rgb8(0xff, 0xd6, 0xa8),
            Tone::Sage   => Color::from_rgb8(0xb8, 0xd0, 0xa8),
        }
    }
    pub fn sticker_gloss(&self, tone: Tone) -> Color {
        // Lighter variant: blend toward white at 35% strength.
        let base = self.sticker_fill(tone);
        Color::from_rgba(
            base.r + (1.0 - base.r) * 0.35,
            base.g + (1.0 - base.g) * 0.35,
            base.b + (1.0 - base.b) * 0.35,
            0.85,
        )
    }
    pub fn ink(&self) -> Color { Color::from_rgb8(0x1a, 0x1a, 0x2e) }
}
```

### `src/ui/sound_grid.rs` integration

Inside the existing per-slot view code:

```rust
let tile_data = SoundTileData {
    id: sound.id.clone(),
    name: sound.name.clone(),
    category: sound.category.clone(),
    tone: derive_tone(&sound.id),
    duration_secs: sound.duration_secs,
    hotkey: slot.hotkey.clone(),
    favorite: sound.favorite,
    seed: derive_seed(&sound.id),
    glyph: derive_glyph(&sound.id),
};

mouse_area(
    canvas(SoundTile::new(&tile_data, theme, &tile_cache))
        .width(Length::Fill)
        .height(Length::Fixed(140.0))
)
.on_press(Message::PlaySlot(slot_idx))
.on_right_press(Message::OpenContextMenu(slot_idx))
```

A per-grid `canvas::Cache` is held on the parent grid struct (re-used across tiles); cache is invalidated on theme switch (existing theme-persistence machinery in #69 already triggers re-render).

Empty / unassigned slots continue to render via the existing pre-#13 path — `SoundTile` is only used for slots with an attached sound. This keeps slot-manager and import flows untouched.

### `src/ui/mod.rs`

```rust
mod sound_tile;
pub use sound_tile::{SoundTile, SoundTileData, Glyph};
```

## Testing

### Unit (regular `cargo test`)

| Test | Asserts |
|---|---|
| `derive_tone_deterministic` | Same `sound_id` returns same `Tone` across 100 calls |
| `derive_glyph_deterministic` | Same for `Glyph` |
| `derive_seed_deterministic` | Same for `u32` |
| `derive_tone_distribution` | 1000 sequential ids cover all 8 `Tone` variants ≥1 time |
| `derive_glyph_distribution` | 1000 sequential ids cover all 8 `Glyph` variants ≥1 time |
| `rotation_for_seed_bounds` | `rotation_for_seed(0)` and `rotation_for_seed(u32::MAX)` both within `[-3.0, 3.0]` |
| `rotation_for_seed_midpoint` | `rotation_for_seed(u32::MAX / 2) ≈ 0.0` (within 0.01) |
| `tone_palette_complete` | Every `Tone` variant returns a non-default `Color` from `sticker_fill` |
| `gloss_is_lighter_than_fill` | `sticker_gloss(tone).r >= sticker_fill(tone).r` for all 8 tones |

### Manual smoke (post-merge)

1. `cargo run` — grid shows sticker tiles with varied tones + glyphs
2. Each tile has visible slight rotation; rotations differ across tiles (non-uniform grid)
3. Restart app — same sound has same tone + glyph + rotation (determinism)
4. Mark a sound as favorite — star appears top-right of its tile
5. Click a tile → sound plays (no regression in click handling)
6. Empty slots render with pre-#13 visual (no `SoundTile` used)
7. 5×4 grid renders smoothly on integrated graphics (no frame drops)
8. Toggle dark/light theme — sticker colors unchanged (tones live in palette, not theme-dependent)
9. `HONKHONK_RENDERER=software cargo run` — tiny-skia software renderer renders gradients + paths correctly

### Out of test scope

- Pixel-diff / screenshot snapshot tests (Iced canvas GPU rendering — no in-place framework)
- Hover state (#92)
- Playing state ring + glow (#92)
- Stress 100+ tile grid (current grid is fixed 5×4)

## Error handling + edge cases

- **Empty `sound_id`**: hash deterministic → Tone::Pink + Glyph::Goose + seed 0 (mid-range rotation). Tile renders normally.
- **Unicode `sound_id`**: `DefaultHasher` byte-stable within a single process arch; cross-platform determinism not required (sound ids are local to user's library).
- **Software renderer** (`HONKHONK_RENDERER=software`): tiny-skia supports radial gradient fills and path strokes — reference file documents this. Manual smoke covers.
- **Tile resize / window resize**: `canvas::Cache::draw` invalidates on size change via Iced internals.
- **Sound with empty `name`**: label paint renders empty string; no panic.
- **Unused `category` / `duration_secs` / `hotkey` fields**: kept on `SoundTileData` for forward-compat with future visual treatment (per-category palette swap, duration ring, hotkey badge) without API churn.
- **Theme switch mid-session**: existing theme-persistence (#69) triggers re-render via Iced subscription; canvas cache invalidates on theme change because `Theme` ref changes.

## TDD ordering (writing-plans will expand)

1. RED: `derive_tone_deterministic` test — fails (fn doesn't exist).
2. GREEN: implement `derive_tone` + `hash`.
3. RED: remaining `derive_*` determinism + distribution tests.
4. GREEN: implement `derive_glyph`, `derive_seed`.
5. RED: `rotation_for_seed_bounds` + `rotation_for_seed_midpoint`.
6. GREEN: implement `rotation_for_seed`.
7. RED: `tone_palette_complete` + `gloss_is_lighter_than_fill`.
8. GREEN: extend `theme.rs` with `Tone` enum + accessors.
9. RED: skeleton `SoundTile` canvas::Program returning empty Geometry → `cargo build` fails until impl complete.
10. GREEN: implement `paint_sticker`, then each glyph helper one at a time, then `paint_favorite_star`, then `paint_label`.
11. Integrate into `sound_grid.rs` — manual smoke verifies tiles render.
12. REFACTOR: extract repeated transform setup if patterns emerge across paint helpers.

## References

- Issue #13: https://github.com/wrzonance/HonkHonk/issues/13
- Follow-up issue #92 (hover + playing animations): https://github.com/wrzonance/HonkHonk/issues/92
- Design reference: `docs/design-reference/src-rust/ui/sound_tile.rs` (420 LOC, "ready to use")
- Visual spec: `docs/design-reference/honkhonk-direction-c.jsx`
- Iced canvas docs: https://docs.rs/iced/0.13/iced/widget/canvas/
- Tiny-skia software renderer support: tested via `HONKHONK_RENDERER=software`
- CLAUDE.md 500 LOC / sub-MVP rule
