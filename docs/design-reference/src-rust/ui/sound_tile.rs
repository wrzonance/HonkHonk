// src/ui/sound_tile.rs — Custom canvas widget for a single sound tile.
//
// Implements the "Confetti" sticker tile from the mockup as an Iced
// `canvas::Program`. This is the right tool because (a) the radial-gradient
// gloss on each sticker isn't a built-in Iced primitive, and (b) the
// deterministic per-sound rotation is awkward to compose with `container`.
//
// Drop this at src/ui/sound_tile.rs. The tile uses our Theme + Tone tokens
// from ui::theme. Click handling is layered on top by the parent via
// `mouse_area`; this widget only paints.
//
// Performance: the gloss + glyph are recomputed every frame. For a 5x4 grid
// that's 20 tiles * a couple dozen path ops each — well within budget at
// 60Hz on integrated graphics. If you go bigger, cache the rendered tile
// to an `image::Handle` keyed on (sound_id, theme, hover, playing).

use iced::{
    advanced::graphics::geometry::frame::Backend,
    mouse,
    widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, Text},
    Color, Pixels, Point, Rectangle, Renderer, Size, Vector,
};

use crate::ui::theme::{Hh, Theme, Tone};

// ──────────────────────────────────────────────────────────────────────────
// Sound model — minimal shape this widget needs from your app state.
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SoundTileData {
    pub id: String,
    pub name: String,
    pub category: String,
    pub tone: Tone,
    pub duration_secs: f32,
    pub hotkey: Option<String>,
    pub favorite: bool,
    pub seed: u32,
    /// Glyph kind — see `Glyph` below. Falls back to a dot if unknown.
    pub glyph: Glyph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Goose,
    AngryGoose,
    Boom,
    Note,
    Arrow,
    ScreamFace,
    Star,
    Dot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TileState {
    #[default]
    Idle,
    Hovered,
    Playing,
}

// ──────────────────────────────────────────────────────────────────────────
// The widget
// ──────────────────────────────────────────────────────────────────────────

pub struct SoundTile<'a> {
    pub sound: &'a SoundTileData,
    pub theme: Theme,
    pub state: TileState,
    pub cache: &'a Cache,
}

impl<'a> SoundTile<'a> {
    pub fn new(sound: &'a SoundTileData, theme: Theme, state: TileState, cache: &'a Cache) -> Self {
        Self { sound, theme, state, cache }
    }

    /// Deterministic ±3° rotation, in radians.
    fn rotation_rad(&self) -> f32 {
        let deg = ((self.sound.seed as f32 * 37.0) % 7.0) - 3.0;
        let amplified = match self.state {
            TileState::Idle => deg,
            TileState::Hovered => deg * 1.4,
            TileState::Playing => deg * 0.6,
        };
        amplified.to_radians()
    }
}

// ──────────────────────────────────────────────────────────────────────────
// canvas::Program impl. Message = () because clicking is handled by the
// parent's mouse_area; this widget is paint-only.
// ──────────────────────────────────────────────────────────────────────────

impl<'a, Message> canvas::Program<Message, Renderer> for SoundTile<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            self.paint(frame, bounds.size());
        });
        vec![geom]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::Pointer
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Painting
// ──────────────────────────────────────────────────────────────────────────

impl SoundTile<'_> {
    fn paint(&self, frame: &mut Frame, size: Size) {
        let theme = self.theme;
        let tone = self.sound.tone;

        // ── 1. Background tint ──────────────────────────────────────────
        let bg = tone.tile_tint(theme.is_dark());
        let radius = 20.0;
        let rect = Path::new(|p| {
            // approximate rounded rect — Iced's geometry frame doesn't have
            // a native rounded_rect, so we trace one with arcs.
            rounded_rect(p, Point::ORIGIN, size, radius);
        });
        frame.fill(&rect, bg);

        // Subtle inner highlight (paper-like).
        if !theme.is_dark() {
            let hi = Path::new(|p| {
                rounded_rect(p, Point::new(0.0, 0.0), Size::new(size.width, 20.0), radius);
            });
            frame.fill(&hi, Color::from_rgba(1.0, 1.0, 1.0, 0.4));
        }

        // ── 2. Hover/playing border ─────────────────────────────────────
        match self.state {
            TileState::Hovered => {
                let stroke = Stroke::default().with_color(theme.ink()).with_width(2.0);
                frame.stroke(&rect, stroke);
            }
            TileState::Playing => {
                // Goose-yellow ring with soft outer glow.
                let stroke = Stroke::default().with_color(theme.accent()).with_width(2.5);
                frame.stroke(&rect, stroke);
                // Cheap glow: stroke a slightly larger rect at low alpha.
                let glow_path = Path::new(|p| {
                    rounded_rect(p, Point::new(-3.0, -3.0), Size::new(size.width + 6.0, size.height + 6.0), radius + 3.0);
                });
                frame.stroke(&glow_path, Stroke::default()
                    .with_color(Color { a: 0.25, ..theme.accent() })
                    .with_width(6.0));
            }
            TileState::Idle => {
                let stroke = Stroke::default().with_color(theme.hairline()).with_width(1.0);
                frame.stroke(&rect, stroke);
            }
        }

        // ── 3. Category label, top-left ─────────────────────────────────
        frame.fill_text(Text {
            content: self.sound.category.to_uppercase(),
            position: Point::new(14.0, 14.0),
            color: theme.ink_dim(),
            size: Pixels(11.0),
            ..Text::default()
        });

        // ── 4. Favorite star, top-right ─────────────────────────────────
        if self.sound.favorite {
            draw_star(frame, Point::new(size.width - 18.0, 18.0), 6.0, theme.accent());
        }

        // ── 5. Sticker, centered ────────────────────────────────────────
        let center = Point::new(size.width / 2.0, size.height * 0.52);
        let sticker_radius = (size.width.min(size.height) * 0.22).clamp(28.0, 42.0);
        let rot = self.rotation_rad() * 1.5; // sticker rotates 1.5× the tile
        self.draw_sticker(frame, center, sticker_radius, rot);

        // ── 6. Name, lower portion ──────────────────────────────────────
        frame.fill_text(Text {
            content: self.sound.name.clone(),
            position: Point::new(size.width / 2.0, size.height - 38.0),
            color: theme.ink(),
            size: Pixels(15.0),
            horizontal_alignment: iced::alignment::Horizontal::Center,
            ..Text::default()
        });

        // ── 7. Hotkey badge or duration, bottom ─────────────────────────
        let bottom_label = self.sound.hotkey.clone().unwrap_or_else(|| {
            format!("{:.0}:{:02}", self.sound.duration_secs as u32 / 60, self.sound.duration_secs as u32 % 60)
        });
        let badge_w = (bottom_label.len() as f32) * 7.5 + 14.0;
        let badge_x = size.width / 2.0 - badge_w / 2.0;
        let badge_y = size.height - 22.0;
        let badge_rect = Path::new(|p| {
            rounded_rect(p, Point::new(badge_x, badge_y), Size::new(badge_w, 16.0), 4.0);
        });
        frame.fill(&badge_rect, Color { a: 0.07, ..theme.ink() });
        frame.fill_text(Text {
            content: bottom_label,
            position: Point::new(size.width / 2.0, badge_y + 8.0),
            color: theme.ink(),
            size: Pixels(11.0),
            horizontal_alignment: iced::alignment::Horizontal::Center,
            vertical_alignment: iced::alignment::Vertical::Center,
            ..Text::default()
        });
    }

    fn draw_sticker(&self, frame: &mut Frame, center: Point, radius: f32, rotation_rad: f32) {
        let dark = self.theme.is_dark();
        let fill = self.sound.tone.sticker(dark);
        let highlight = self.sound.tone.highlight(dark);

        // Save → translate → rotate → draw → restore.
        frame.with_save(|frame| {
            frame.translate(Vector::new(center.x, center.y));
            frame.rotate(rotation_rad);

            // Sticker disc.
            let disc = Path::circle(Point::ORIGIN, radius);
            frame.fill(&disc, fill);

            // Radial gloss — emulate by stacking a smaller, brighter disc
            // shifted up-left at low alpha.
            let gloss = Path::circle(Point::new(-radius * 0.25, -radius * 0.3), radius * 0.6);
            frame.fill(&gloss, Color { a: 0.55, ..highlight });

            // Hairline border for crispness on light backgrounds.
            let stroke = Stroke::default()
                .with_color(Color { a: 0.18, ..self.theme.ink() })
                .with_width(1.0);
            frame.stroke(&disc, stroke);

            // Glyph.
            self.draw_glyph(frame, radius);
        });
    }

    fn draw_glyph(&self, frame: &mut Frame, r: f32) {
        let dark = self.theme.is_dark();
        let fg = if dark { Color::from_rgb(0.95, 0.95, 0.95) } else { Color::from_rgb(0.10, 0.07, 0.03) };

        match self.sound.glyph {
            Glyph::Goose       => draw_goose(frame, r * 0.85, fg, false),
            Glyph::AngryGoose  => draw_goose(frame, r * 0.85, fg, true),
            Glyph::Boom        => draw_boom(frame, r * 0.7, fg),
            Glyph::Note        => draw_note(frame, r * 0.6, fg),
            Glyph::Arrow       => draw_arrow(frame, r * 0.7, fg),
            Glyph::ScreamFace  => draw_scream(frame, r * 0.6, fg),
            Glyph::Star        => draw_star(frame, Point::ORIGIN, r * 0.7, fg),
            Glyph::Dot         => {
                let dot = Path::circle(Point::ORIGIN, r * 0.25);
                frame.fill(&dot, fg);
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Glyph primitives — keep simple, scale by argument, draw centered at origin.
// ──────────────────────────────────────────────────────────────────────────

fn draw_goose(frame: &mut Frame, scale: f32, color: Color, angry: bool) {
    let body = Path::new(|p| {
        // Body — leaning forward
        p.move_to(Point::new(-0.6 * scale, 0.3 * scale));
        p.quadratic_curve_to(Point::new(-0.7 * scale, -0.4 * scale), Point::new(0.0,  -0.4 * scale));
        p.quadratic_curve_to(Point::new(0.6 * scale,  -0.3 * scale), Point::new(0.5 * scale, 0.4 * scale));
        p.line_to(Point::new(-0.5 * scale, 0.4 * scale));
        p.close();
    });
    frame.fill(&body, color);

    let neck = Path::new(|p| {
        p.move_to(Point::new(-0.55 * scale, -0.3 * scale));
        p.quadratic_curve_to(Point::new(-0.85 * scale, -0.7 * scale), Point::new(-0.7 * scale, -0.95 * scale));
        p.line_to(Point::new(-0.55 * scale, -0.95 * scale));
        p.quadratic_curve_to(Point::new(-0.65 * scale, -0.6 * scale), Point::new(-0.4 * scale, -0.4 * scale));
        p.close();
    });
    frame.fill(&neck, color);

    // Beak
    let beak = Path::new(|p| {
        p.move_to(Point::new(-0.7 * scale, -0.85 * scale));
        p.line_to(Point::new(-0.95 * scale, -0.78 * scale));
        p.line_to(Point::new(-0.7 * scale, -0.7 * scale));
        p.close();
    });
    frame.fill(&beak, if angry { Color::from_rgb(0.85, 0.2, 0.1) } else { Color::from_rgb(0.95, 0.62, 0.05) });

    // Eye
    let eye = Path::circle(Point::new(-0.6 * scale, -0.85 * scale), 0.05 * scale);
    frame.fill(&eye, Color::from_rgb(1.0, 1.0, 1.0));
    let pupil = Path::circle(Point::new(-0.6 * scale, -0.85 * scale), 0.025 * scale);
    frame.fill(&pupil, Color::BLACK);

    // Angry eyebrow
    if angry {
        let brow = Path::new(|p| {
            p.move_to(Point::new(-0.7 * scale, -0.97 * scale));
            p.line_to(Point::new(-0.5 * scale, -0.9 * scale));
        });
        frame.stroke(&brow, Stroke::default().with_color(Color::from_rgb(0.85, 0.2, 0.1)).with_width(scale * 0.08));
    }
}

fn draw_boom(frame: &mut Frame, r: f32, color: Color) {
    // 3 concentric rings
    for i in 1..=3 {
        let radius = r * (i as f32 / 3.0);
        let ring = Path::circle(Point::ORIGIN, radius);
        frame.stroke(&ring, Stroke::default().with_color(color).with_width(r * 0.12));
    }
}

fn draw_note(frame: &mut Frame, r: f32, color: Color) {
    let head = Path::new(|p| {
        // Slightly tilted ellipse — approximate via two arcs.
        p.move_to(Point::new(-r * 0.6, r * 0.3));
        p.quadratic_curve_to(Point::new(-r * 0.7, -r * 0.3), Point::new(0.0, -r * 0.3));
        p.quadratic_curve_to(Point::new(r * 0.6, -r * 0.3), Point::new(r * 0.4, r * 0.3));
        p.quadratic_curve_to(Point::new(0.0, r * 0.6), Point::new(-r * 0.6, r * 0.3));
        p.close();
    });
    frame.fill(&head, color);

    // Stem
    let stem = Path::new(|p| {
        p.move_to(Point::new(r * 0.4, 0.0));
        p.line_to(Point::new(r * 0.4, -r * 1.2));
        p.line_to(Point::new(r * 0.5, -r * 1.2));
        p.line_to(Point::new(r * 0.5, 0.0));
        p.close();
    });
    frame.fill(&stem, color);
}

fn draw_arrow(frame: &mut Frame, r: f32, color: Color) {
    let arr = Path::new(|p| {
        p.move_to(Point::new(-r, 0.0));
        p.line_to(Point::new(r * 0.5, 0.0));
        p.line_to(Point::new(r * 0.2, -r * 0.5));
        p.move_to(Point::new(r * 0.5, 0.0));
        p.line_to(Point::new(r * 0.2, r * 0.5));
    });
    frame.stroke(&arr, Stroke::default().with_color(color).with_width(r * 0.18).with_line_cap(canvas::LineCap::Round));
}

fn draw_scream(frame: &mut Frame, r: f32, color: Color) {
    let face = Path::circle(Point::ORIGIN, r);
    frame.stroke(&face, Stroke::default().with_color(color).with_width(r * 0.1));
    // Eyes
    let l = Path::circle(Point::new(-r * 0.35, -r * 0.15), r * 0.1);
    let rr = Path::circle(Point::new(r * 0.35, -r * 0.15), r * 0.1);
    frame.fill(&l, color);
    frame.fill(&rr, color);
    // Open mouth (oval)
    let mouth = Path::new(|p| {
        p.move_to(Point::new(0.0, r * 0.2));
        p.quadratic_curve_to(Point::new(r * 0.25, r * 0.5), Point::new(0.0, r * 0.6));
        p.quadratic_curve_to(Point::new(-r * 0.25, r * 0.5), Point::new(0.0, r * 0.2));
        p.close();
    });
    frame.fill(&mouth, color);
}

fn draw_star(frame: &mut Frame, c: Point, r: f32, color: Color) {
    let star = Path::new(|p| {
        for i in 0..10 {
            let angle = std::f32::consts::PI / 5.0 * i as f32 - std::f32::consts::FRAC_PI_2;
            let radius = if i % 2 == 0 { r } else { r * 0.45 };
            let x = c.x + angle.cos() * radius;
            let y = c.y + angle.sin() * radius;
            if i == 0 { p.move_to(Point::new(x, y)); } else { p.line_to(Point::new(x, y)); }
        }
        p.close();
    });
    frame.fill(&star, color);
}

// ──────────────────────────────────────────────────────────────────────────
// Geometry helper — Iced's path builder doesn't have rounded_rect.
// ──────────────────────────────────────────────────────────────────────────

fn rounded_rect(p: &mut canvas::path::Builder, origin: Point, size: Size, r: f32) {
    let r = r.min(size.width / 2.0).min(size.height / 2.0);
    let x = origin.x;
    let y = origin.y;
    let w = size.width;
    let h = size.height;
    p.move_to(Point::new(x + r, y));
    p.line_to(Point::new(x + w - r, y));
    p.quadratic_curve_to(Point::new(x + w, y), Point::new(x + w, y + r));
    p.line_to(Point::new(x + w, y + h - r));
    p.quadratic_curve_to(Point::new(x + w, y + h), Point::new(x + w - r, y + h));
    p.line_to(Point::new(x + r, y + h));
    p.quadratic_curve_to(Point::new(x, y + h), Point::new(x, y + h - r));
    p.line_to(Point::new(x, y + r));
    p.quadratic_curve_to(Point::new(x, y), Point::new(x + r, y));
    p.close();
}
