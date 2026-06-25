//! Renderer-neutral canvas overlay for [`PanelFlourish`](super::PanelFlourish).

use iced::widget::{Space, canvas};
use iced::{Color, Element, Length, Point, Vector};

use crate::app::Message;

use super::{FeatherParticle, PanelFlourish};

pub fn view_panel_flourish(flourish: &PanelFlourish) -> Element<'_, Message> {
    if !flourish.is_animating() {
        return Space::new().width(Length::Fill).height(Length::Fill).into();
    }
    canvas::Canvas::new(FeatherProgram {
        particles: flourish.particles(),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

struct FeatherProgram<'a> {
    particles: &'a [FeatherParticle],
}

impl<Message> canvas::Program<Message> for FeatherProgram<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let colors = feather_colors(theme);
        for particle in self.particles {
            draw_feather(&mut frame, *particle, colors);
        }
        vec![frame.into_geometry()]
    }
}

fn draw_feather(frame: &mut canvas::Frame, particle: FeatherParticle, colors: FeatherColors) {
    use iced::widget::canvas::{Path, Stroke};

    if particle.alpha <= 0.0 {
        return;
    }
    let dir = unit_from_angle(particle.rotation);
    let spine = scale(dir, particle.size);
    let start = translate(particle.position, scale(spine, -0.45));
    let end = translate(particle.position, scale(spine, 0.55));
    let ink = Color {
        a: particle.alpha * 0.9,
        ..colors.ink
    };
    let shadow = Color {
        a: particle.alpha * 0.5,
        ..colors.shadow
    };
    frame.stroke(
        &Path::line(start, end),
        Stroke::default().with_color(ink).with_width(1.4),
    );
    draw_barbs(frame, particle, dir, shadow);
}

#[derive(Clone, Copy)]
struct FeatherColors {
    ink: Color,
    shadow: Color,
}

fn feather_colors(theme: &iced::Theme) -> FeatherColors {
    if matches!(theme, iced::Theme::Light) {
        return FeatherColors {
            ink: Color::from_rgb(0.20, 0.22, 0.24),
            shadow: Color::from_rgb(0.55, 0.58, 0.62),
        };
    }
    FeatherColors {
        ink: Color::WHITE,
        shadow: Color::from_rgb(0.72, 0.76, 0.8),
    }
}

fn draw_barbs(frame: &mut canvas::Frame, particle: FeatherParticle, dir: Vector, color: Color) {
    use iced::widget::canvas::{Path, Stroke};

    let normal = Vector::new(-dir.y, dir.x);
    for i in 0..3 {
        let along = -0.25 + i as f32 * 0.22;
        let base = translate(particle.position, scale(dir, particle.size * along));
        let barb_len = particle.size * (0.32 - i as f32 * 0.05);
        for side in [-1.0_f32, 1.0] {
            let outward = scale(normal, side * barb_len);
            let backward = scale(dir, -particle.size * 0.12);
            let tip = translate(base, add(outward, backward));
            frame.stroke(
                &Path::line(base, tip),
                Stroke::default().with_color(color).with_width(1.0),
            );
        }
    }
}

fn unit_from_angle(angle: f32) -> Vector {
    Vector::new(angle.cos(), angle.sin())
}

fn translate(point: Point, vector: Vector) -> Point {
    Point::new(point.x + vector.x, point.y + vector.y)
}

fn add(a: Vector, b: Vector) -> Vector {
    Vector::new(a.x + b.x, a.y + b.y)
}

fn scale(vector: Vector, scalar: f32) -> Vector {
    Vector::new(vector.x * scalar, vector.y * scalar)
}
