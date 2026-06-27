use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use honkhonk::ui::side_panel::{
    BurstEmitter, BurstLine, FeatherClass, PanelFlourish, PanelRect, PanelTransition,
    panel_burst_emitter,
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

fn floating_panel() -> PanelRect {
    PanelRect {
        x: 300.0,
        y: 180.0,
        w: 420.0,
        h: 320.0,
        center: Point::new(510.0, 340.0),
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
    let emitter = panel_burst_emitter(floating_panel(), (1280.0, 800.0));
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
    let particles = flourish.particles();
    assert!(!particles.is_empty());
    let min_y = particles
        .iter()
        .map(|p| p.position.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = particles
        .iter()
        .map(|p| p.position.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(min_y < 80.0, "feathers should start near the top edge");
    assert!(max_y > 720.0, "feathers should start near the bottom edge");
    assert!(
        max_y - min_y > 700.0,
        "feathers should span most of the edge"
    );
}

#[test]
fn burst_seeds_all_feather_classes() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    let classes = flourish
        .particles()
        .iter()
        .map(|p| p.class)
        .collect::<BTreeSet<_>>();

    assert!(classes.contains(&FeatherClass::Dust));
    assert!(classes.contains(&FeatherClass::Chunk));
    assert!(classes.contains(&FeatherClass::Feather));
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
