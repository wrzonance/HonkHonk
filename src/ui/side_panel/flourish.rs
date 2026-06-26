//! Reusable panel open/close flourish: panel bounds in, short feather burst out.
//! The app owns a [`PanelFlourish`] and ticks it only while active; future panels
//! opt in by calling [`PanelFlourish::emit`] with their own full-open bounds.

use std::time::{Duration, Instant};

use iced::{Point, Vector};

use super::PanelRect;

pub const BURST_DURATION: Duration = Duration::from_millis(3000);
const PARTICLES: usize = 18;
const EDGE_EPS: f32 = 2.0;
const GRAVITY: f32 = 80.0;
const CURSOR_RADIUS: f32 = 56.0;
const CURSOR_FORCE: f32 = 260.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTransition {
    Open,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BurstEmitter {
    Edge(BurstLine),
    Center(Point),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurstLine {
    pub start: Point,
    pub end: Point,
    pub direction: Vector,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatherParticle {
    pub position: Point,
    pub velocity: Vector,
    pub alpha: f32,
    pub size: f32,
    pub rotation: f32,
}

#[derive(Debug, Clone, Default)]
pub struct PanelFlourish {
    particles: Vec<FeatherParticle>,
    started: Option<Instant>,
    last_tick: Option<Instant>,
}

pub fn panel_burst_emitter(panel: PanelRect, window: (f32, f32)) -> BurstEmitter {
    let win_w = window.0.max(0.0);
    let win_h = window.1.max(0.0);
    let touches_left = panel.x <= EDGE_EPS;
    let touches_top = panel.y <= EDGE_EPS;
    let touches_right = win_w > 0.0 && panel.x + panel.w >= win_w - EDGE_EPS;
    let touches_bottom = win_h > 0.0 && panel.y + panel.h >= win_h - EDGE_EPS;

    if touches_right && panel.x > EDGE_EPS {
        return edge_line(
            Point::new(panel.x, panel.y),
            Point::new(panel.x, panel.y + panel.h),
            -1.0,
            0.0,
        );
    }
    if touches_left && panel.x + panel.w < win_w - EDGE_EPS {
        return edge_line(
            Point::new(panel.x + panel.w, panel.y),
            Point::new(panel.x + panel.w, panel.y + panel.h),
            1.0,
            0.0,
        );
    }
    if touches_bottom && panel.y > EDGE_EPS {
        return edge_line(
            Point::new(panel.x, panel.y),
            Point::new(panel.x + panel.w, panel.y),
            0.0,
            -1.0,
        );
    }
    if touches_top && panel.y + panel.h < win_h - EDGE_EPS {
        return edge_line(
            Point::new(panel.x, panel.y + panel.h),
            Point::new(panel.x + panel.w, panel.y + panel.h),
            0.0,
            1.0,
        );
    }

    BurstEmitter::Center(panel.center)
}

impl PanelFlourish {
    pub fn emit(
        &mut self,
        panel: PanelRect,
        window: (f32, f32),
        transition: PanelTransition,
        now: Instant,
    ) {
        let emitter = panel_burst_emitter(panel, window);
        self.particles = seed_particles(emitter, transition);
        self.started = Some(now);
        self.last_tick = Some(now);
    }

    pub fn tick(&mut self, now: Instant, cursor: Option<Point>) -> bool {
        let Some(started) = self.started else {
            return false;
        };
        if now.saturating_duration_since(started) >= BURST_DURATION {
            self.clear();
            return false;
        }

        let last = self.last_tick.unwrap_or(started);
        let dt = now.saturating_duration_since(last).as_secs_f32();
        self.last_tick = Some(now);
        let alpha = fade_alpha(started, now);
        for p in &mut self.particles {
            tick_particle(p, dt, cursor);
            p.alpha = alpha;
        }
        true
    }

    pub fn is_animating(&self) -> bool {
        !self.particles.is_empty()
    }

    pub fn particles(&self) -> &[FeatherParticle] {
        &self.particles
    }

    pub fn clear(&mut self) {
        self.particles.clear();
        self.started = None;
        self.last_tick = None;
    }
}

fn edge_line(start: Point, end: Point, x: f32, y: f32) -> BurstEmitter {
    BurstEmitter::Edge(BurstLine {
        start,
        end,
        direction: Vector::new(x, y),
    })
}

fn seed_particles(emitter: BurstEmitter, transition: PanelTransition) -> Vec<FeatherParticle> {
    (0..PARTICLES)
        .map(|i| seed_particle(emitter, transition, i))
        .collect()
}

fn seed_particle(emitter: BurstEmitter, transition: PanelTransition, i: usize) -> FeatherParticle {
    let dir = particle_direction(emitter, i);
    let perp = Vector::new(-dir.y, dir.x);
    let scatter = ((i % 7) as f32 - 3.0) / 3.0;
    let speed = 62.0 + (i % 5) as f32 * 9.0;
    let drift = -18.0 + (i % 4) as f32 * 9.0;
    let velocity = add(
        scale(dir, speed),
        add(scale(perp, scatter * 28.0), Vector::new(0.0, drift)),
    );
    let offset = add(
        scale(dir, 5.0 + (i % 3) as f32 * 2.0),
        scale(perp, scatter * 6.0),
    );
    let rotation_bias = match transition {
        PanelTransition::Open => 0.0,
        PanelTransition::Close => 0.35,
    };

    FeatherParticle {
        position: translate(emitter_point(emitter), offset),
        velocity,
        alpha: 1.0,
        size: 10.0 + (i % 4) as f32 * 3.5,
        rotation: rotation_bias + scatter * 0.45,
    }
}

fn particle_direction(emitter: BurstEmitter, i: usize) -> Vector {
    match emitter {
        BurstEmitter::Edge(line) => normalize(line.direction),
        BurstEmitter::Center(_) => {
            let angle = i as f32 * 2.399_963_1;
            Vector::new(angle.cos(), angle.sin())
        }
    }
}

fn emitter_point(emitter: BurstEmitter) -> Point {
    match emitter {
        BurstEmitter::Edge(line) => Point::new(
            (line.start.x + line.end.x) / 2.0,
            (line.start.y + line.end.y) / 2.0,
        ),
        BurstEmitter::Center(point) => point,
    }
}

fn tick_particle(particle: &mut FeatherParticle, dt: f32, cursor: Option<Point>) {
    if let Some(cursor) = cursor {
        particle.velocity = add(particle.velocity, cursor_bump(*particle, cursor, dt));
    }
    particle.velocity.y += GRAVITY * dt;
    particle.position = translate(particle.position, scale(particle.velocity, dt));
    particle.rotation += dt * 0.6;
}

fn cursor_bump(particle: FeatherParticle, cursor: Point, dt: f32) -> Vector {
    let away = Vector::new(
        particle.position.x - cursor.x,
        particle.position.y - cursor.y,
    );
    let dist = length(away);
    if dist <= f32::EPSILON || dist >= CURSOR_RADIUS {
        return Vector::new(0.0, 0.0);
    }
    scale(
        normalize(away),
        (1.0 - dist / CURSOR_RADIUS) * CURSOR_FORCE * dt,
    )
}

fn fade_alpha(started: Instant, now: Instant) -> f32 {
    let elapsed = now.saturating_duration_since(started).as_secs_f32();
    let duration = BURST_DURATION.as_secs_f32();
    (1.0 - elapsed / duration).clamp(0.0, 1.0)
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

fn normalize(vector: Vector) -> Vector {
    let len = length(vector);
    if len <= f32::EPSILON {
        return Vector::new(0.0, 0.0);
    }
    scale(vector, 1.0 / len)
}

fn length(vector: Vector) -> f32 {
    (vector.x * vector.x + vector.y * vector.y).sqrt()
}
