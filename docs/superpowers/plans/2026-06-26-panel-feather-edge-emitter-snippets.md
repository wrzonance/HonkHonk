# Panel Feather Edge Emitter Plan Snippets

These snippets accompany
`docs/superpowers/plans/2026-06-26-panel-feather-edge-emitter.md`.

## Replacement tests/side_panel_flourish.rs

```rust
use std::time::{Duration, Instant};

use honkhonk::ui::side_panel::{
    BurstEmitter, BurstLine, PanelFlourish, PanelRect, PanelTransition, panel_burst_emitter,
};
use iced::{Point, Vector};

fn right_panel() -> PanelRect {
    PanelRect {
        x: 880.0,
        y: 0.0,
        w: 400.0,
        h: 800.0,
        center: Point::new(1080.0, 400.0),
    }
}

fn left_panel() -> PanelRect {
    PanelRect {
        x: 0.0,
        y: 0.0,
        w: 320.0,
        h: 700.0,
        center: Point::new(160.0, 350.0),
    }
}

fn top_panel() -> PanelRect {
    PanelRect {
        x: 120.0,
        y: 0.0,
        w: 720.0,
        h: 180.0,
        center: Point::new(480.0, 90.0),
    }
}

fn bottom_panel() -> PanelRect {
    PanelRect {
        x: 120.0,
        y: 620.0,
        w: 720.0,
        h: 180.0,
        center: Point::new(480.0, 710.0),
    }
}

fn assert_line(actual: BurstLine, start: Point, end: Point, direction: Vector) {
    assert_eq!(actual.start, start);
    assert_eq!(actual.end, end);
    assert_eq!(actual.direction, direction);
}

#[test]
fn right_docked_panel_emits_from_full_inner_edge_away_from_panel() {
    let emitter = panel_burst_emitter(right_panel(), (1280.0, 800.0));
    let BurstEmitter::Edge(line) = emitter else {
        panic!("right docked panel should emit from an edge line");
    };
    assert_line(
        line,
        Point::new(880.0, 0.0),
        Point::new(880.0, 800.0),
        Vector::new(-1.0, 0.0),
    );
}

#[test]
fn all_docked_panel_edges_emit_from_full_inner_edge() {
    let BurstEmitter::Edge(left) = panel_burst_emitter(left_panel(), (1280.0, 700.0)) else {
        panic!("left docked panel should emit from an edge line");
    };
    let BurstEmitter::Edge(top) = panel_burst_emitter(top_panel(), (960.0, 800.0)) else {
        panic!("top docked panel should emit from an edge line");
    };
    let BurstEmitter::Edge(bottom) = panel_burst_emitter(bottom_panel(), (960.0, 800.0)) else {
        panic!("bottom docked panel should emit from an edge line");
    };
    assert_line(
        left,
        Point::new(320.0, 0.0),
        Point::new(320.0, 700.0),
        Vector::new(1.0, 0.0),
    );
    assert_line(
        top,
        Point::new(120.0, 180.0),
        Point::new(840.0, 180.0),
        Vector::new(0.0, 1.0),
    );
    assert_line(
        bottom,
        Point::new(120.0, 620.0),
        Point::new(840.0, 620.0),
        Vector::new(0.0, -1.0),
    );
}

#[test]
fn floating_panel_emits_from_center() {
    let emitter = panel_burst_emitter(
        PanelRect {
            x: 300.0,
            y: 180.0,
            w: 420.0,
            h: 320.0,
            center: Point::new(510.0, 340.0),
        },
        (1280.0, 800.0),
    );
    assert_eq!(emitter, BurstEmitter::Center(Point::new(510.0, 340.0)));
}

#[test]
fn emitted_edge_particles_start_away_from_panel() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);
    let particles = flourish.particles();
    assert!(!particles.is_empty());
    assert!(particles.iter().all(|p| p.velocity.x < 0.0));
}

#[test]
fn edge_particles_span_most_of_the_panel_edge() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);
    let min_y = flourish
        .particles()
        .iter()
        .map(|p| p.position.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = flourish
        .particles()
        .iter()
        .map(|p| p.position.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(min_y < 80.0, "feathers should start near the top edge");
    assert!(max_y > 720.0, "feathers should start near the bottom edge");
    assert!(max_y - min_y > 700.0, "feathers should span most of the edge");
}

#[test]
fn feathers_float_down_and_fade_out() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Close, now);
    let start_y = flourish.particles()[0].position.y;
    assert!(flourish.tick(now + Duration::from_millis(1500), None));
    let mid = flourish.particles()[0];
    assert!(mid.position.y > start_y);
    assert!(mid.alpha > 0.0 && mid.alpha < 1.0);
    assert!(!flourish.tick(now + Duration::from_millis(3100), None));
    assert!(!flourish.is_animating());
}

#[test]
fn cursor_gently_bumps_nearby_feathers() {
    let now = Instant::now();
    let mut plain = PanelFlourish::default();
    plain.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);
    let mut bumped = plain.clone();
    let first = plain.particles()[0];
    let cursor = Point::new(first.position.x + 8.0, first.position.y);
    plain.tick(now + Duration::from_millis(16), None);
    bumped.tick(now + Duration::from_millis(16), Some(cursor));
    let plain_dx = plain.particles()[0].position.x - first.position.x;
    let bumped_dx = bumped.particles()[0].position.x - first.position.x;
    assert!(bumped_dx < plain_dx, "cursor should push feather away");
}
```

## Emitter type definitions

```rust
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
```

## panel_burst_emitter

```rust
pub fn panel_burst_emitter(panel: PanelRect, window: (f32, f32)) -> BurstEmitter {
    let win_w = window.0.max(0.0);
    let win_h = window.1.max(0.0);
    let touches_left = panel.x <= EDGE_EPS;
    let touches_top = panel.y <= EDGE_EPS;
    let touches_right = win_w > 0.0 && panel.x + panel.w >= win_w - EDGE_EPS;
    let touches_bottom = win_h > 0.0 && panel.y + panel.h >= win_h - EDGE_EPS;
    if touches_right && panel.x > EDGE_EPS {
        return edge_line(Point::new(panel.x, panel.y), Point::new(panel.x, panel.y + panel.h), -1.0, 0.0);
    }
    if touches_left && panel.x + panel.w < win_w - EDGE_EPS {
        return edge_line(Point::new(panel.x + panel.w, panel.y), Point::new(panel.x + panel.w, panel.y + panel.h), 1.0, 0.0);
    }
    if touches_bottom && panel.y > EDGE_EPS {
        return edge_line(Point::new(panel.x, panel.y), Point::new(panel.x + panel.w, panel.y), 0.0, -1.0);
    }
    if touches_top && panel.y + panel.h < win_h - EDGE_EPS {
        return edge_line(Point::new(panel.x, panel.y + panel.h), Point::new(panel.x + panel.w, panel.y + panel.h), 0.0, 1.0);
    }
    BurstEmitter::Center(panel.center)
}
```

## edge_line helper

```rust
fn edge_line(start: Point, end: Point, x: f32, y: f32) -> BurstEmitter {
    BurstEmitter::Edge(BurstLine {
        start,
        end,
        direction: Vector::new(x, y),
    })
}
```

## seed_particles and seed_particle signatures

```rust
fn seed_particles(emitter: BurstEmitter, transition: PanelTransition) -> Vec<FeatherParticle> {
    (0..PARTICLES)
        .map(|i| seed_particle(emitter, transition, i))
        .collect()
}

fn seed_particle(emitter: BurstEmitter, transition: PanelTransition, i: usize) -> FeatherParticle {
    let dir = particle_direction(emitter, i);
```

## particle_direction and midpoint emitter_point

```rust
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
```

## mod.rs re-export

```rust
pub use flourish::{
    BURST_DURATION, BurstEmitter, BurstLine, FeatherParticle, PanelFlourish, PanelTransition,
    panel_burst_emitter,
};
```

## stratified emitter_point

```rust
fn emitter_point(emitter: BurstEmitter, i: usize) -> Point {
    match emitter {
        BurstEmitter::Edge(line) => point_on_line(line, edge_t(i)),
        BurstEmitter::Center(point) => point,
    }
}

fn point_on_line(line: BurstLine, t: f32) -> Point {
    Point::new(
        line.start.x + (line.end.x - line.start.x) * t,
        line.start.y + (line.end.y - line.start.y) * t,
    )
}

fn edge_t(i: usize) -> f32 {
    let slot = 1.0 / PARTICLES as f32;
    let base = (i as f32 + 0.5) * slot;
    let jitter = deterministic_jitter(i) * slot * 0.7;
    (base + jitter).clamp(slot * 0.25, 1.0 - slot * 0.25)
}

fn deterministic_jitter(i: usize) -> f32 {
    const JITTER: [f32; 9] = [-0.42, 0.18, -0.08, 0.36, -0.25, 0.05, 0.44, -0.15, 0.27];
    JITTER[i % JITTER.len()]
}
```
