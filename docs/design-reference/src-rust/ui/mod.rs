// src/ui/mod.rs — module index for the HonkHonk UI layer.
//
// Drop this alongside theme.rs and sound_tile.rs at src/ui/mod.rs and re-export
// the bits the rest of the app cares about.

pub mod theme;
pub mod sound_tile;

pub use theme::{Theme, Tone, Hh, space, radius, hairline_border, accent_border, bg_color};
pub use sound_tile::{SoundTile, SoundTileData, TileState, Glyph};
