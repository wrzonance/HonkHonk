use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use crate::ui::theme::{Hh, Theme, Tone};

pub(super) fn draw_sticker_glow(frame: &mut canvas::Frame, tone: Tone, theme: Theme, radius: f32) {
    let shadow = Path::circle(Point::new(0.0, 3.0), radius + 5.0);
    frame.fill(
        &shadow,
        Color {
            a: if theme.is_dark() { 0.24 } else { 0.14 },
            ..Color::BLACK
        },
    );

    for (grow, width, alpha) in [(5.0, 7.0, 0.18), (9.0, 5.0, 0.11)] {
        let glow = Path::circle(Point::ORIGIN, radius + grow);
        frame.stroke(
            &glow,
            Stroke::default()
                .with_color(Color {
                    a: alpha,
                    ..tone.highlight(theme.is_dark())
                })
                .with_width(width),
        );
    }
}

pub(super) fn draw_sticker_disc(frame: &mut canvas::Frame, tone: Tone, theme: Theme, radius: f32) {
    let dark = theme.is_dark();
    let disc = Path::circle(Point::ORIGIN, radius);
    frame.fill(&disc, tone.sticker(dark));
    for (scale, alpha) in [(0.78, 0.22), (0.58, 0.22), (0.36, 0.18)] {
        let gloss = Path::circle(Point::new(-radius * 0.18, -radius * 0.22), radius * scale);
        frame.fill(
            &gloss,
            Color {
                a: alpha,
                ..tone.highlight(dark)
            },
        );
    }
    frame.stroke(
        &disc,
        Stroke::default()
            .with_color(Color {
                a: 0.18,
                ..theme.ink()
            })
            .with_width(1.0),
    );
}

pub(super) fn draw_playing_ring(frame: &mut canvas::Frame, tone: Tone, theme: Theme, radius: f32) {
    let ring = Path::circle(Point::ORIGIN, radius + 3.0);
    frame.stroke(
        &ring,
        Stroke::default()
            .with_color(tone.highlight(theme.is_dark()))
            .with_width(2.6),
    );
}

pub(super) fn draw_hover_ring(
    frame: &mut canvas::Frame,
    theme: Theme,
    radius: f32,
    hover_progress: f32,
) {
    let ring = Path::circle(Point::ORIGIN, radius + 6.0);
    frame.stroke(
        &ring,
        Stroke::default()
            .with_color(Color {
                a: 0.72 * hover_progress,
                ..theme.ink()
            })
            .with_width(1.0 + hover_progress * 1.2),
    );
}
