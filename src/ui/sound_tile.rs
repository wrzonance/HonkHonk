use iced::mouse;
use iced::widget::canvas::{self, Path, Stroke, Text};
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Size, Vector};
use iced::{Event, window};

use crate::app::Message;
use crate::ui::theme::{self, Hh, Theme, Tone};
use crate::ui::tile_layout;

mod animation;
mod effects;

pub const PLACEHOLDER_GRAPHIC: &str = "\u{1f50a}";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const ROTATION_STEPS_PER_DEGREE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq)]
pub struct SoundTileData {
    pub id: String,
    pub name: String,
    pub category: String,
    pub duration: String,
    pub hotkey: Option<String>,
    pub favorite: bool,
    pub tone: Tone,
    pub seed: u64,
}

impl SoundTileData {
    pub fn rotation_degrees(&self) -> f32 {
        rotation_degrees(self.seed)
    }

    pub fn placeholder_graphic(&self) -> &'static str {
        PLACEHOLDER_GRAPHIC
    }
}

pub fn seed_from_sound_id(id: &str) -> u64 {
    if let Some(seed) = id
        .get(..16)
        .and_then(|hex| u64::from_str_radix(hex, 16).ok())
        .filter(|seed| *seed != 0)
    {
        return seed;
    }

    let seed = id.bytes().fold(FNV_OFFSET_BASIS, |acc, byte| {
        (acc ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    });

    if seed == 0 { FNV_OFFSET_BASIS } else { seed }
}

pub fn rotation_degrees(seed: u64) -> f32 {
    let bucket_count =
        (tile_layout::IDLE_MAX_ROTATION_DEGREES * 2.0 * ROTATION_STEPS_PER_DEGREE as f32) as u64
            + 1;
    let bucket = (seed % bucket_count) as f32;
    bucket / ROTATION_STEPS_PER_DEGREE as f32 - tile_layout::IDLE_MAX_ROTATION_DEGREES
}

pub fn tone_from_seed(seed: u64) -> Tone {
    Tone::from_index(seed as usize)
}

pub struct SoundTile {
    data: SoundTileData,
    theme: Theme,
    is_playing: bool,
}

impl SoundTile {
    pub fn new(data: SoundTileData, theme: Theme, is_playing: bool) -> Self {
        Self {
            data,
            theme,
            is_playing,
        }
    }
}

impl<Message> canvas::Program<Message> for SoundTile {
    type State = animation::HoverAnimation;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let now = match event {
            Event::Window(window::Event::RedrawRequested(now)) => *now,
            _ => std::time::Instant::now(),
        };
        let retargeted = state.retarget_if_changed(cursor.is_over(bounds), now);
        let progressed = state.tick(now);

        if retargeted || state.is_animating_at(now) || progressed {
            Some(canvas::Action::request_redraw())
        } else {
            None
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        self.paint(&mut frame, bounds.size(), state.progress());
        vec![frame.into_geometry()]
    }
}

pub fn view<'a>(data: SoundTileData, theme: Theme, is_playing: bool) -> Element<'a, Message> {
    canvas::Canvas::new(SoundTile::new(data, theme, is_playing))
        .width(Length::Fill)
        .height(tile_layout::tile_slot_height())
        .into()
}

impl SoundTile {
    fn paint(&self, frame: &mut canvas::Frame, size: Size, hover_progress: f32) {
        let tile_size = tile_layout::fitted_tile_size(size);
        let center = Point::new(size.width / 2.0, size.height / 2.0);
        let rotation = animation::hover_rotation_degrees(self.data.seed, hover_progress);

        frame.with_save(|frame| {
            frame.translate(Vector::new(center.x, center.y));
            frame.rotate(rotation.to_radians());
            frame.translate(Vector::new(-tile_size.width / 2.0, -tile_size.height / 2.0));
            self.paint_rotated(frame, tile_size, hover_progress);
        });
    }

    fn paint_rotated(&self, frame: &mut canvas::Frame, size: Size, hover_progress: f32) {
        let tile = Path::rounded_rectangle(Point::ORIGIN, size, theme::radius::TILE);
        frame.fill(&tile, self.data.tone.tile_tint(self.theme.is_dark()));
        frame.stroke(&tile, self.tile_stroke());
        self.paint_top_row(frame, size);
        self.paint_sticker(frame, size, hover_progress);
        self.paint_name(frame, size);
        self.paint_footer(frame, size);
    }

    fn tile_stroke(&self) -> Stroke<'static> {
        let color = if self.is_playing {
            self.theme.accent()
        } else {
            self.theme.hairline()
        };
        Stroke::default()
            .with_color(color)
            .with_width(if self.is_playing { 2.5 } else { 1.0 })
    }

    fn paint_top_row(&self, frame: &mut canvas::Frame, size: Size) {
        frame.fill_text(Text {
            content: self.data.category.to_uppercase(),
            position: Point::new(16.0, 16.0),
            max_width: (size.width - 54.0).max(0.0),
            color: self.theme.ink_dim(),
            size: Pixels(theme::font::LABEL),
            ..Text::default()
        });

        if self.data.favorite {
            draw_star(
                frame,
                Point::new(size.width - 18.0, 18.0),
                7.0,
                self.theme.accent(),
            );
        }
    }

    fn paint_sticker(&self, frame: &mut canvas::Frame, size: Size, hover_progress: f32) {
        let center = Point::new(size.width / 2.0, size.height * 0.45);
        let radius = (size.width.min(size.height) * 0.24).clamp(28.0, 40.0);
        let rotation = animation::hover_rotation_degrees(self.data.seed, hover_progress);
        frame.with_save(|frame| {
            frame.translate(Vector::new(center.x, center.y));
            frame.rotate((rotation * 1.5).to_radians());
            if self.is_playing {
                effects::draw_sticker_glow(frame, self.data.tone, self.theme, radius);
            }
            effects::draw_sticker_disc(frame, self.data.tone, self.theme, radius);
            if self.is_playing {
                effects::draw_playing_ring(frame, self.data.tone, self.theme, radius);
            }
            if hover_progress > 0.0 {
                effects::draw_hover_ring(frame, self.theme, radius, hover_progress);
            }
            frame.fill_text(Text {
                content: self.data.placeholder_graphic().to_owned(),
                position: Point::ORIGIN,
                color: Color::WHITE,
                size: Pixels(radius * 0.85),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Text::default()
            });
        });
    }

    fn paint_name(&self, frame: &mut canvas::Frame, size: Size) {
        frame.fill_text(Text {
            content: self.data.name.clone(),
            position: Point::new(size.width / 2.0, size.height - 42.0),
            max_width: size.width - 28.0,
            color: self.theme.ink(),
            size: Pixels(theme::font::BODY + 2.0),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..Text::default()
        });
    }

    fn paint_footer(&self, frame: &mut canvas::Frame, size: Size) {
        frame.fill_text(Text {
            content: self.data.duration.clone(),
            position: Point::new(16.0, size.height - 18.0),
            color: self.theme.ink_faint(),
            size: Pixels(theme::font::LABEL),
            align_y: iced::alignment::Vertical::Center,
            ..Text::default()
        });

        if let Some(hotkey) = &self.data.hotkey {
            draw_hotkey_badge(
                frame,
                hotkey,
                Point::new(size.width - 16.0, size.height - 18.0),
                self.theme,
            );
        } else {
            draw_play_chip(
                frame,
                Point::new(size.width - 24.0, size.height - 20.0),
                self,
            );
        }
    }
}

fn draw_hotkey_badge(frame: &mut canvas::Frame, hotkey: &str, center: Point, theme: Theme) {
    let width = hotkey.chars().count() as f32 * 7.0 + 14.0;
    let size = Size::new(width, 18.0);
    let origin = Point::new(center.x - width, center.y - size.height / 2.0);
    let badge = Path::rounded_rectangle(origin, size, 4.0.into());
    frame.fill(
        &badge,
        Color {
            a: 0.10,
            ..theme.ink()
        },
    );
    frame.fill_text(Text {
        content: hotkey.to_owned(),
        position: Point::new(origin.x + width / 2.0, center.y),
        color: theme.ink(),
        size: Pixels(theme::font::LABEL),
        font: iced::Font::MONOSPACE,
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..Text::default()
    });
}

fn draw_play_chip(frame: &mut canvas::Frame, center: Point, tile: &SoundTile) {
    let circle = Path::circle(center, 12.0);
    frame.fill(&circle, tile.data.tone.highlight(tile.theme.is_dark()));
    let icon = if tile.is_playing {
        "\u{23f8}"
    } else {
        "\u{25b6}"
    };
    frame.fill_text(Text {
        content: icon.to_owned(),
        position: center,
        color: Color::WHITE,
        size: Pixels(10.0),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..Text::default()
    });
}

fn draw_star(frame: &mut canvas::Frame, center: Point, radius: f32, color: Color) {
    let star = Path::new(|path| {
        for i in 0..10 {
            let angle = std::f32::consts::PI / 5.0 * i as f32;
            let angle = angle - std::f32::consts::FRAC_PI_2;
            let r = if i % 2 == 0 { radius } else { radius * 0.45 };
            let point = Point::new(center.x + angle.cos() * r, center.y + angle.sin() * r);
            if i == 0 {
                path.move_to(point);
            } else {
                path.line_to(point);
            }
        }
        path.close();
    });
    frame.fill(&star, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_degrees_is_deterministic_and_bounded() {
        for seed in [0, 1, 2, 99, 123_456_789, u64::MAX] {
            let first = rotation_degrees(seed);
            let second = rotation_degrees(seed);

            assert_eq!(first, second);
            assert!(
                (-tile_layout::IDLE_MAX_ROTATION_DEGREES..=tile_layout::IDLE_MAX_ROTATION_DEGREES)
                    .contains(&first),
                "rotation {first} exceeded the tile layout clearance"
            );
        }
    }

    #[test]
    fn rotation_degrees_uses_tile_layout_clearance_bound() {
        assert_eq!(rotation_degrees(0), -tile_layout::IDLE_MAX_ROTATION_DEGREES);
        assert_eq!(
            rotation_degrees(6_000),
            tile_layout::IDLE_MAX_ROTATION_DEGREES
        );
    }

    #[test]
    fn seed_from_sound_id_uses_stable_fallback_for_non_hex_ids() {
        let airhorn = seed_from_sound_id("airhorn");
        let honk = seed_from_sound_id("honk");

        assert_ne!(airhorn, 0);
        assert_ne!(honk, 0);
        assert_eq!(airhorn, seed_from_sound_id("airhorn"));
        assert_ne!(airhorn, honk);
    }

    #[test]
    fn placeholder_graphic_is_uniform_for_every_tile() {
        let first = SoundTileData {
            id: "a".into(),
            name: "Airhorn".into(),
            category: "Memes".into(),
            duration: "0:01".into(),
            hotkey: None,
            favorite: false,
            tone: Tone::Amber,
            seed: 1,
        };
        let second = SoundTileData {
            id: "b".into(),
            name: "Goose".into(),
            category: "Honk".into(),
            duration: "0:02".into(),
            hotkey: Some("F1".into()),
            favorite: true,
            tone: Tone::Blue,
            seed: 2,
        };

        assert_eq!(first.placeholder_graphic(), "\u{1f50a}");
        assert_eq!(first.placeholder_graphic(), second.placeholder_graphic());
    }
}
