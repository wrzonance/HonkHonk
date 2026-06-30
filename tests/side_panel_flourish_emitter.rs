//! Emitter-geometry tests for the panel-feather flourish: where the burst seeds
//! from given a panel's docked edge (or lack of one). Motion/animation tests
//! live in `side_panel_flourish.rs`; shared fixtures in `flourish_support`.
mod flourish_support;

use std::time::Instant;

use flourish_support::*;

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
fn panels_without_a_spreadable_edge_emit_from_center() {
    // A free-floating panel has no docked edge to spread along.
    assert_eq!(
        panel_burst_emitter(floating_panel(), (1280.0, 800.0)),
        BurstEmitter::Center(Point::new(510.0, 340.0)),
    );
    // A docked panel collapsed to zero height touches an edge but has no span,
    // so it must also fall back to the center rather than a zero-length edge.
    let collapsed = PanelRect {
        x: 880.0,
        y: 400.0,
        w: 400.0,
        h: 0.0,
        center: Point::new(1080.0, 400.0),
    };
    assert_eq!(
        panel_burst_emitter(collapsed, (1280.0, 800.0)),
        BurstEmitter::Center(Point::new(1080.0, 400.0)),
    );
}

#[test]
fn emitted_edge_particles_start_away_from_panel() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);
    let particles = flourish.particles();
    assert!(!particles.is_empty());
    assert!(particles.iter().all(|p| p.velocity.x < 0.0));
}

#[test]
fn edge_particles_span_most_of_the_panel_edge() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);
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
