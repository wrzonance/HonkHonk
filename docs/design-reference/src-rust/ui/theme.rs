// src/ui/theme.rs — HonkHonk "Confetti" theme tokens.
//
// Drop this into your Iced app at src/ui/theme.rs. The Theme enum implements
// the conversions Iced's stylesheet traits expect. All colors are oklch-derived
// from the design mockup, hardcoded as sRGB hex for portability.
//
// Usage:
//   use crate::ui::theme::{Theme, Tone, Hh};
//   let t = Theme::Light;
//   container::Style {
//       background: Some(t.bg().into()),
//       text_color: Some(t.ink()),
//       border: Border { color: t.hairline(), width: 1.0, radius: 14.0.into() },
//       ..Default::default()
//   }
//
// The palette intentionally mirrors honkhonk-direction-c.jsx so the Rust port
// renders pixel-equivalent to the mockup. If you change a hex in one place,
// change it in the other.

use iced::{border::Radius, Background, Border, Color};

// ──────────────────────────────────────────────────────────────────────────
// Theme enum
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub fn is_dark(self) -> bool {
        matches!(self, Theme::Dark)
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Spacing scale — match the mockup's literal numbers so density tweak works.
// ──────────────────────────────────────────────────────────────────────────

pub mod space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

pub mod radius {
    use iced::border::Radius;
    pub const XS: Radius = Radius { top_left: 4.0,  top_right: 4.0,  bottom_left: 4.0,  bottom_right: 4.0 };
    pub const SM: Radius = Radius { top_left: 8.0,  top_right: 8.0,  bottom_left: 8.0,  bottom_right: 8.0 };
    pub const MD: Radius = Radius { top_left: 12.0, top_right: 12.0, bottom_left: 12.0, bottom_right: 12.0 };
    pub const LG: Radius = Radius { top_left: 18.0, top_right: 18.0, bottom_left: 18.0, bottom_right: 18.0 };
    pub const TILE: Radius = Radius { top_left: 20.0, top_right: 20.0, bottom_left: 20.0, bottom_right: 20.0 };
    pub const PILL: Radius = Radius { top_left: 999.0, top_right: 999.0, bottom_left: 999.0, bottom_right: 999.0 };
}

// ──────────────────────────────────────────────────────────────────────────
// Tone — per-sound color identity. Maps to the same `tone` field in mockups.
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone { Amber, Orange, Yellow, Lime, Cyan, Blue, Pink, Red, Purple, Gray }

impl Tone {
    /// (hue°, saturation%, lightness%) — same triplet as C_TONE in JS.
    pub fn hsl(self) -> (f32, f32, f32) {
        match self {
            Tone::Amber  => (38.0,  95.0, 55.0),
            Tone::Orange => (22.0,  90.0, 56.0),
            Tone::Yellow => (50.0,  95.0, 55.0),
            Tone::Lime   => (95.0,  65.0, 50.0),
            Tone::Cyan   => (190.0, 75.0, 50.0),
            Tone::Blue   => (220.0, 70.0, 56.0),
            Tone::Pink   => (340.0, 80.0, 60.0),
            Tone::Red    => (0.0,   75.0, 55.0),
            Tone::Purple => (270.0, 60.0, 60.0),
            Tone::Gray   => (220.0, 8.0,  55.0),
        }
    }

    /// The vivid sticker fill at the chosen lightness, with optional dark-mode shift.
    pub fn sticker(self, dark: bool) -> Color {
        let (h, s, l) = self.hsl();
        hsl_to_color(h, s / 100.0, ((if dark { (l - 5.0).max(40.0) } else { l }) / 100.0).clamp(0.0, 1.0))
    }

    /// Highlight color for the radial gloss.
    pub fn highlight(self, dark: bool) -> Color {
        let (h, s, l) = self.hsl();
        let hl = if dark { (l + 12.0).min(70.0) } else { (l + 22.0).min(85.0) };
        hsl_to_color(h, s / 100.0, hl / 100.0)
    }

    /// Tinted background used behind a tile.
    pub fn tile_tint(self, dark: bool) -> Color {
        let (h, s, _) = self.hsl();
        if dark {
            hsl_to_color(h, s.min(40.0) / 100.0, 0.13)
        } else {
            hsl_to_color(h, s.min(60.0) / 100.0, 0.93)
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Surface palette — call as `theme.bg()`, `theme.ink()`, etc.
// ──────────────────────────────────────────────────────────────────────────

pub trait Hh {
    fn bg(self) -> Color;
    fn panel(self) -> Color;
    fn panel_alt(self) -> Color;
    fn ink(self) -> Color;
    fn ink_dim(self) -> Color;
    fn ink_faint(self) -> Color;
    fn hairline(self) -> Color;
    fn hairline_strong(self) -> Color;
    fn accent(self) -> Color;
    fn accent_deep(self) -> Color;
    fn good(self) -> Color;
    fn warn(self) -> Color;
    fn danger(self) -> Color;
}

impl Hh for Theme {
    fn bg(self) -> Color {
        match self { Theme::Light => hex(0xf4efe4), Theme::Dark => hex(0x171410) }
    }
    fn panel(self) -> Color {
        match self { Theme::Light => hex(0xfffaf0), Theme::Dark => hex(0x1f1c16) }
    }
    fn panel_alt(self) -> Color {
        match self { Theme::Light => hex(0xfaf3e0), Theme::Dark => hex(0x26211a) }
    }
    fn ink(self) -> Color {
        match self { Theme::Light => hex(0x1a1208), Theme::Dark => hex(0xfbf3df) }
    }
    fn ink_dim(self) -> Color {
        match self { Theme::Light => hex(0x6a553a), Theme::Dark => hex(0xa39377) }
    }
    fn ink_faint(self) -> Color {
        match self { Theme::Light => hex(0xa8957a), Theme::Dark => hex(0x6a5b46) }
    }
    fn hairline(self) -> Color {
        match self {
            Theme::Light => Color::from_rgba(0.0, 0.0, 0.0, 0.06),
            Theme::Dark  => Color::from_rgba(1.0, 1.0, 1.0, 0.06),
        }
    }
    fn hairline_strong(self) -> Color {
        match self {
            Theme::Light => Color::from_rgba(0.0, 0.0, 0.0, 0.12),
            Theme::Dark  => Color::from_rgba(1.0, 1.0, 1.0, 0.12),
        }
    }
    fn accent(self) -> Color       { hex(0xf59e0b) }
    fn accent_deep(self) -> Color  { hex(0xb45309) }
    fn good(self) -> Color         { hex(0x16a34a) }
    fn warn(self) -> Color         { hex(0xf59e0b) }
    fn danger(self) -> Color       { hex(0xdc2626) }
}

// ──────────────────────────────────────────────────────────────────────────
// Convenience constructors so call sites read like CSS.
// ──────────────────────────────────────────────────────────────────────────

pub fn hairline_border(t: Theme, radius: Radius) -> Border {
    Border { color: t.hairline(), width: 1.0, radius }
}

pub fn accent_border(t: Theme, radius: Radius) -> Border {
    Border { color: t.accent(), width: 2.0, radius }
}

pub fn bg_color(c: Color) -> Background {
    Background::Color(c)
}

// ──────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────

fn hex(rgb: u32) -> Color {
    let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
    let b = (rgb & 0xff) as f32 / 255.0;
    Color::from_rgb(r, g, b)
}

/// hsl → linear sRGB. h in degrees, s/l in 0..=1.
fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h6 = (h / 60.0).rem_euclid(6.0);
    let x = c * (1.0 - (h6.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    Color::from_rgb(r1 + m, g1 + m, b1 + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amber_sticker_matches_mockup() {
        // hsl(38 95% 55%) ≈ #f59e0b-ish
        let c = Tone::Amber.sticker(false);
        assert!((c.r - 0.96).abs() < 0.05);
        assert!((c.g - 0.62).abs() < 0.08);
    }

    #[test]
    fn dark_sticker_is_dimmer_than_light() {
        let l = Tone::Pink.sticker(false);
        let d = Tone::Pink.sticker(true);
        assert!(l.r + l.g + l.b > d.r + d.g + d.b);
    }
}
