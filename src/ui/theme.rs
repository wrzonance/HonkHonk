use iced::{Background, Border, Color};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn is_dark(self) -> bool {
        matches!(self, Theme::Dark)
    }
}

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
    pub const SM: Radius = Radius {
        top_left: 8.0,
        top_right: 8.0,
        bottom_left: 8.0,
        bottom_right: 8.0,
    };
    pub const MD: Radius = Radius {
        top_left: 12.0,
        top_right: 12.0,
        bottom_left: 12.0,
        bottom_right: 12.0,
    };
    pub const TILE: Radius = Radius {
        top_left: 20.0,
        top_right: 20.0,
        bottom_left: 20.0,
        bottom_right: 20.0,
    };
    pub const PILL: Radius = Radius {
        top_left: 999.0,
        top_right: 999.0,
        bottom_left: 999.0,
        bottom_right: 999.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Amber,
    Orange,
    Yellow,
    Lime,
    Cyan,
    Blue,
    Pink,
    Red,
    Purple,
    Gray,
}

const TONES: [Tone; 10] = [
    Tone::Amber,
    Tone::Orange,
    Tone::Yellow,
    Tone::Lime,
    Tone::Cyan,
    Tone::Blue,
    Tone::Pink,
    Tone::Red,
    Tone::Purple,
    Tone::Gray,
];

impl Tone {
    pub fn from_index(idx: usize) -> Self {
        TONES[idx % TONES.len()]
    }

    fn hsl(self) -> (f32, f32, f32) {
        match self {
            Tone::Amber => (38.0, 95.0, 55.0),
            Tone::Orange => (22.0, 90.0, 56.0),
            Tone::Yellow => (50.0, 95.0, 55.0),
            Tone::Lime => (95.0, 65.0, 50.0),
            Tone::Cyan => (190.0, 75.0, 50.0),
            Tone::Blue => (220.0, 70.0, 56.0),
            Tone::Pink => (340.0, 80.0, 60.0),
            Tone::Red => (0.0, 75.0, 55.0),
            Tone::Purple => (270.0, 60.0, 60.0),
            Tone::Gray => (220.0, 8.0, 55.0),
        }
    }

    pub fn tile_tint(self, dark: bool) -> Color {
        let (h, s, _) = self.hsl();
        if dark {
            hsl_to_color(h, s.min(40.0) / 100.0, 0.13)
        } else {
            hsl_to_color(h, s.min(60.0) / 100.0, 0.93)
        }
    }

    pub fn highlight(self, dark: bool) -> Color {
        let (h, s, l) = self.hsl();
        if dark {
            hsl_to_color(h, s / 100.0, (l - 5.0).max(0.0) / 100.0)
        } else {
            hsl_to_color(h, s / 100.0, l / 100.0)
        }
    }
}

pub trait Hh {
    fn bg(self) -> Color;
    fn panel(self) -> Color;
    fn ink(self) -> Color;
    fn ink_dim(self) -> Color;
    fn ink_faint(self) -> Color;
    fn hairline(self) -> Color;
    fn hairline2(self) -> Color;
    fn good(self) -> Color;
    fn accent(self) -> Color;
}

impl Hh for Theme {
    fn bg(self) -> Color {
        match self {
            Theme::Light => hex(0xf4efe4),
            Theme::Dark => hex(0x171410),
        }
    }
    fn panel(self) -> Color {
        match self {
            Theme::Light => hex(0xfffaf0),
            Theme::Dark => hex(0x1f1c16),
        }
    }
    fn ink(self) -> Color {
        match self {
            Theme::Light => hex(0x1a1208),
            Theme::Dark => hex(0xfbf3df),
        }
    }
    fn ink_dim(self) -> Color {
        match self {
            Theme::Light => hex(0x6a553a),
            Theme::Dark => hex(0xa39377),
        }
    }
    fn ink_faint(self) -> Color {
        match self {
            Theme::Light => hex(0xa8957a),
            Theme::Dark => hex(0x6a5b46),
        }
    }
    fn hairline(self) -> Color {
        match self {
            Theme::Light => Color::from_rgba(0.0, 0.0, 0.0, 0.06),
            Theme::Dark => Color::from_rgba(1.0, 1.0, 1.0, 0.06),
        }
    }
    fn hairline2(self) -> Color {
        match self {
            Theme::Light => Color::from_rgba(0.0, 0.0, 0.0, 0.12),
            Theme::Dark => Color::from_rgba(1.0, 1.0, 1.0, 0.12),
        }
    }
    fn good(self) -> Color {
        match self {
            Theme::Light => hex(0x16a34a),
            Theme::Dark => hex(0x4ade80),
        }
    }
    fn accent(self) -> Color {
        hex(0xfbbf24)
    }
}

pub fn bg_color(c: Color) -> Background {
    Background::Color(c)
}

pub fn tile_border(color: Color, width: f32) -> Border {
    Border {
        color,
        width,
        radius: radius::TILE,
    }
}

fn hex(rgb: u32) -> Color {
    let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
    let b = (rgb & 0xff) as f32 / 255.0;
    Color::from_rgb(r, g, b)
}

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
