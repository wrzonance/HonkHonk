// Static sticker-tile canvas widget (issue #13 sub-MVP).
// Paints a sticker disc (radial-gloss + ±3° rotation), glyph, category,
// name, and a hotkey/duration badge. Clicks are handled by parent's
// `mouse_area`. Hover/playing animations are deferred to #92.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, Text};
use iced::{mouse, Color, Pixels, Point, Rectangle, Renderer, Size, Vector};

use crate::ui::theme::{sticker_ink, StickerTone, Theme, STICKER_TONES};

/// Per-tile data built at view time from a `SoundEntry` plus
/// hash-derived `tone`/`glyph`/`seed` fields.
#[derive(Debug, Clone)]
pub struct SoundTileData {
    pub id: String,
    pub name: String,
    pub category: String,
    pub tone: StickerTone,
    pub duration_secs: f32,
    pub hotkey: Option<String>,
    pub favorite: bool,
    pub seed: u32,
    pub glyph: Glyph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

pub const GLYPHS: [Glyph; 8] = [
    Glyph::Goose,
    Glyph::AngryGoose,
    Glyph::Boom,
    Glyph::Note,
    Glyph::Arrow,
    Glyph::ScreamFace,
    Glyph::Star,
    Glyph::Dot,
];

pub struct SoundTile {
    data: SoundTileData,
    theme: Theme,
    cache: Cache,
}

impl SoundTile {
    pub fn new(data: SoundTileData, theme: Theme) -> Self {
        Self {
            data,
            theme,
            cache: Cache::new(),
        }
    }
}

impl<Message> canvas::Program<Message, iced::Theme, Renderer> for SoundTile {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            paint_tile(frame, &self.data, self.theme);
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

// ── Painting ─────────────────────────────────────────────────────────────

fn paint_tile(frame: &mut Frame, data: &SoundTileData, theme: Theme) {
    let size = frame.size();
    paint_category(frame, &data.category, theme);
    if data.favorite {
        paint_favorite_star(frame, size);
    }
    paint_sticker(frame, data, size);
    paint_name(frame, &data.name, theme, size);
    paint_bottom_badge(frame, data, theme, size);
}

fn paint_category(frame: &mut Frame, category: &str, theme: Theme) {
    use crate::ui::theme::Hh;
    frame.fill_text(Text {
        content: category.to_uppercase(),
        position: Point::new(14.0, 14.0),
        color: theme.ink_dim(),
        size: Pixels(11.0),
        ..Text::default()
    });
}

fn paint_favorite_star(frame: &mut Frame, size: Size) {
    let star_color = Color::from_rgb8(0xfb, 0xbf, 0x24);
    draw_star_path(frame, Point::new(size.width - 18.0, 18.0), 6.0, star_color);
}

fn paint_sticker(frame: &mut Frame, data: &SoundTileData, size: Size) {
    let center = Point::new(size.width / 2.0, size.height * 0.42);
    let radius = (size.width.min(size.height) * 0.22).clamp(28.0, 42.0);
    let rot = rotation_for_seed(data.seed).to_radians();

    frame.with_save(|frame| {
        frame.translate(Vector::new(center.x, center.y));
        frame.rotate(rot);

        // Disc fill.
        let disc = Path::circle(Point::ORIGIN, radius);
        frame.fill(&disc, data.tone.fill());

        // Radial gloss — emulated by a smaller, lighter disc offset up-left.
        let gloss = Path::circle(Point::new(-radius * 0.25, -radius * 0.3), radius * 0.6);
        frame.fill(&gloss, data.tone.gloss());

        // Hairline outline.
        let outline = Stroke::default()
            .with_color(Color {
                a: 0.22,
                ..sticker_ink()
            })
            .with_width(1.5);
        frame.stroke(&disc, outline);

        // Glyph painted within the rotated frame so it sticks to the disc.
        paint_glyph(frame, data.glyph, radius);
    });
}

fn paint_name(frame: &mut Frame, name: &str, theme: Theme, size: Size) {
    use crate::ui::theme::Hh;
    frame.fill_text(Text {
        content: name.to_string(),
        position: Point::new(size.width / 2.0, size.height - 38.0),
        color: theme.ink(),
        size: Pixels(15.0),
        align_x: iced::alignment::Horizontal::Center.into(),
        ..Text::default()
    });
}

fn paint_bottom_badge(frame: &mut Frame, data: &SoundTileData, theme: Theme, size: Size) {
    use crate::ui::theme::Hh;
    let label = badge_label(data);
    if label.is_empty() {
        return;
    }
    let badge_y = size.height - 22.0;
    frame.fill_text(Text {
        content: label,
        position: Point::new(size.width / 2.0, badge_y + 8.0),
        color: theme.ink_dim(),
        size: Pixels(11.0),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..Text::default()
    });
}

fn badge_label(data: &SoundTileData) -> String {
    if let Some(hk) = &data.hotkey {
        return hk.clone();
    }
    let secs = data.duration_secs as u32;
    format!("{}:{:02}", secs / 60, secs % 60)
}

// ── Glyph primitives ─────────────────────────────────────────────────────

fn paint_glyph(frame: &mut Frame, glyph: Glyph, r: f32) {
    let fg = sticker_ink();
    match glyph {
        Glyph::Goose => draw_goose(frame, r * 0.85, fg, false),
        Glyph::AngryGoose => draw_goose(frame, r * 0.85, fg, true),
        Glyph::Boom => draw_boom(frame, r * 0.7, fg),
        Glyph::Note => draw_note(frame, r * 0.6, fg),
        Glyph::Arrow => draw_arrow(frame, r * 0.7, fg),
        Glyph::ScreamFace => draw_scream(frame, r * 0.6, fg),
        Glyph::Star => draw_star_path(frame, Point::ORIGIN, r * 0.7, fg),
        Glyph::Dot => {
            let dot = Path::circle(Point::ORIGIN, r * 0.25);
            frame.fill(&dot, fg);
        }
    }
}

fn draw_goose(frame: &mut Frame, s: f32, color: Color, angry: bool) {
    let body = Path::new(|p| {
        p.move_to(Point::new(-0.6 * s, 0.3 * s));
        p.quadratic_curve_to(Point::new(-0.7 * s, -0.4 * s), Point::new(0.0, -0.4 * s));
        p.quadratic_curve_to(Point::new(0.6 * s, -0.3 * s), Point::new(0.5 * s, 0.4 * s));
        p.line_to(Point::new(-0.5 * s, 0.4 * s));
        p.close();
    });
    frame.fill(&body, color);
    let neck = Path::new(|p| {
        p.move_to(Point::new(-0.55 * s, -0.3 * s));
        p.quadratic_curve_to(
            Point::new(-0.85 * s, -0.7 * s),
            Point::new(-0.7 * s, -0.95 * s),
        );
        p.line_to(Point::new(-0.55 * s, -0.95 * s));
        p.quadratic_curve_to(
            Point::new(-0.65 * s, -0.6 * s),
            Point::new(-0.4 * s, -0.4 * s),
        );
        p.close();
    });
    frame.fill(&neck, color);
    let beak_color = if angry {
        Color::from_rgb(0.85, 0.2, 0.1)
    } else {
        Color::from_rgb(0.95, 0.62, 0.05)
    };
    let beak = Path::new(|p| {
        p.move_to(Point::new(-0.7 * s, -0.85 * s));
        p.line_to(Point::new(-0.95 * s, -0.78 * s));
        p.line_to(Point::new(-0.7 * s, -0.7 * s));
        p.close();
    });
    frame.fill(&beak, beak_color);
    let eye = Path::circle(Point::new(-0.6 * s, -0.85 * s), 0.05 * s);
    frame.fill(&eye, Color::WHITE);
    let pupil = Path::circle(Point::new(-0.6 * s, -0.85 * s), 0.025 * s);
    frame.fill(&pupil, Color::BLACK);
    if angry {
        let brow = Path::new(|p| {
            p.move_to(Point::new(-0.7 * s, -0.97 * s));
            p.line_to(Point::new(-0.5 * s, -0.9 * s));
        });
        frame.stroke(
            &brow,
            Stroke::default()
                .with_color(Color::from_rgb(0.85, 0.2, 0.1))
                .with_width(s * 0.08),
        );
    }
}

fn draw_boom(frame: &mut Frame, r: f32, color: Color) {
    for i in 1..=3 {
        let radius = r * (i as f32 / 3.0);
        let ring = Path::circle(Point::ORIGIN, radius);
        frame.stroke(
            &ring,
            Stroke::default().with_color(color).with_width(r * 0.12),
        );
    }
}

fn draw_note(frame: &mut Frame, r: f32, color: Color) {
    let head = Path::new(|p| {
        p.move_to(Point::new(-r * 0.6, r * 0.3));
        p.quadratic_curve_to(Point::new(-r * 0.7, -r * 0.3), Point::new(0.0, -r * 0.3));
        p.quadratic_curve_to(Point::new(r * 0.6, -r * 0.3), Point::new(r * 0.4, r * 0.3));
        p.quadratic_curve_to(Point::new(0.0, r * 0.6), Point::new(-r * 0.6, r * 0.3));
        p.close();
    });
    frame.fill(&head, color);
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
    frame.stroke(
        &arr,
        Stroke::default()
            .with_color(color)
            .with_width(r * 0.18)
            .with_line_cap(canvas::LineCap::Round),
    );
}

fn draw_scream(frame: &mut Frame, r: f32, color: Color) {
    let face = Path::circle(Point::ORIGIN, r);
    frame.stroke(
        &face,
        Stroke::default().with_color(color).with_width(r * 0.1),
    );
    let l = Path::circle(Point::new(-r * 0.35, -r * 0.15), r * 0.1);
    let rr = Path::circle(Point::new(r * 0.35, -r * 0.15), r * 0.1);
    frame.fill(&l, color);
    frame.fill(&rr, color);
    let mouth = Path::new(|p| {
        p.move_to(Point::new(0.0, r * 0.2));
        p.quadratic_curve_to(Point::new(r * 0.25, r * 0.5), Point::new(0.0, r * 0.6));
        p.quadratic_curve_to(Point::new(-r * 0.25, r * 0.5), Point::new(0.0, r * 0.2));
        p.close();
    });
    frame.fill(&mouth, color);
}

fn draw_star_path(frame: &mut Frame, c: Point, r: f32, color: Color) {
    let star = Path::new(|p| {
        for i in 0..10 {
            let angle = std::f32::consts::PI / 5.0 * i as f32 - std::f32::consts::FRAC_PI_2;
            let radius = if i % 2 == 0 { r } else { r * 0.45 };
            let x = c.x + angle.cos() * radius;
            let y = c.y + angle.sin() * radius;
            if i == 0 {
                p.move_to(Point::new(x, y));
            } else {
                p.line_to(Point::new(x, y));
            }
        }
        p.close();
    });
    frame.fill(&star, color);
}

// ── Hash-derived assignment ──────────────────────────────────────────────

pub fn derive_tone(sound_id: &str) -> StickerTone {
    STICKER_TONES[hash(sound_id) as usize % STICKER_TONES.len()]
}

pub fn derive_glyph(sound_id: &str) -> Glyph {
    // Offset so tone and glyph aren't perfectly correlated.
    GLYPHS[(hash(sound_id).wrapping_add(31)) as usize % GLYPHS.len()]
}

pub fn derive_seed(sound_id: &str) -> u32 {
    hash(sound_id) as u32
}

/// Map `seed` → rotation angle in degrees, uniformly in `[-3.0, 3.0]`.
pub fn rotation_for_seed(seed: u32) -> f32 {
    let frac = f64::from(seed) / f64::from(u32::MAX); // 0.0..=1.0
    (frac * 6.0 - 3.0) as f32
}

fn hash(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn id(n: usize) -> String {
        format!("sound-{n:08x}")
    }

    #[test]
    fn derive_is_deterministic() {
        let id = "any-sound-id";
        let (t, g, s) = (derive_tone(id), derive_glyph(id), derive_seed(id));
        for _ in 0..100 {
            assert_eq!(derive_tone(id), t);
            assert_eq!(derive_glyph(id), g);
            assert_eq!(derive_seed(id), s);
        }
    }

    #[test]
    fn derive_tone_distribution() {
        let seen: HashSet<StickerTone> = (0..1000).map(|n| derive_tone(&id(n))).collect();
        assert_eq!(seen.len(), STICKER_TONES.len(), "got {seen:?}");
    }

    #[test]
    fn derive_glyph_distribution() {
        let seen: HashSet<Glyph> = (0..1000).map(|n| derive_glyph(&id(n))).collect();
        assert_eq!(seen.len(), GLYPHS.len(), "got {seen:?}");
    }

    #[test]
    fn rotation_for_seed_bounds() {
        for seed in [0_u32, 1, u32::MAX / 4, u32::MAX / 2, u32::MAX] {
            let r = rotation_for_seed(seed);
            assert!((-3.0..=3.0).contains(&r), "rotation({seed})={r}");
        }
    }

    #[test]
    fn rotation_for_seed_midpoint() {
        let r = rotation_for_seed(u32::MAX / 2);
        assert!(r.abs() < 0.01, "expected near 0.0, got {r}");
    }

    #[test]
    fn rotation_for_seed_endpoints() {
        assert!((rotation_for_seed(0) + 3.0).abs() < 0.001);
        assert!((rotation_for_seed(u32::MAX) - 3.0).abs() < 0.001);
    }
}
