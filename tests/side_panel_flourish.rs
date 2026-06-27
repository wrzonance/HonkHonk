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

fn first_particle_of(
    flourish: &PanelFlourish,
    class: FeatherClass,
) -> honkhonk::ui::side_panel::FeatherParticle {
    *flourish
        .particles()
        .iter()
        .find(|p| p.class == class)
        .expect("burst should include requested feather class")
}

fn tick_for(flourish: &mut PanelFlourish, start: Instant, total: Duration) {
    let total_ms = total.as_millis() as u64;
    let mut elapsed_ms = 16;
    while elapsed_ms < total_ms {
        assert!(flourish.tick(start + Duration::from_millis(elapsed_ms), None));
        elapsed_ms += 16;
    }
    assert!(flourish.tick(start + total, None));
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
fn dust_descends_farther_than_full_feather() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    let dust_start = first_particle_of(&flourish, FeatherClass::Dust).position.y;
    let feather_start = first_particle_of(&flourish, FeatherClass::Feather)
        .position
        .y;

    tick_for(&mut flourish, now, Duration::from_millis(900));

    let dust_drop = first_particle_of(&flourish, FeatherClass::Dust).position.y - dust_start;
    let feather_drop = first_particle_of(&flourish, FeatherClass::Feather)
        .position
        .y
        - feather_start;

    assert!(
        dust_drop > feather_drop + 18.0,
        "dust should fall faster than a full feather: dust={dust_drop}, feather={feather_drop}"
    );
}

#[test]
fn full_feathers_swoop_more_than_dust() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    let dust_start = first_particle_of(&flourish, FeatherClass::Dust).position.x;
    let feather_start = first_particle_of(&flourish, FeatherClass::Feather)
        .position
        .x;

    tick_for(&mut flourish, now, Duration::from_millis(1200));

    let dust_dx = (first_particle_of(&flourish, FeatherClass::Dust).position.x - dust_start).abs();
    let feather_dx = (first_particle_of(&flourish, FeatherClass::Feather)
        .position
        .x
        - feather_start)
        .abs();

    assert!(
        feather_dx > dust_dx + 8.0,
        "full feathers should have more lateral swoop: dust={dust_dx}, feather={feather_dx}"
    );
}

#[test]
fn same_class_feathers_diverge_from_seeded_wobble_phase() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), PanelTransition::Open, now);

    let starts = flourish
        .particles()
        .iter()
        .enumerate()
        .filter(|(_, p)| p.class == FeatherClass::Feather)
        .take(2)
        .map(|(i, p)| (i, p.position.x, p.velocity.x))
        .collect::<Vec<_>>();
    assert_eq!(starts.len(), 2);
    assert!(
        (starts[0].1 - starts[1].1).abs() <= f32::EPSILON,
        "chosen feathers must start at the same x position"
    );
    assert!(
        (starts[0].2 - starts[1].2).abs() <= f32::EPSILON,
        "chosen feathers must start with the same x velocity"
    );

    tick_for(&mut flourish, now, Duration::from_millis(1200));

    let first_dx = flourish.particles()[starts[0].0].position.x - starts[0].1;
    let second_dx = flourish.particles()[starts[1].0].position.x - starts[1].1;
    let divergence = (first_dx - second_dx).abs();

    assert!(
        divergence > 4.0,
        "same-class feathers should diverge from seeded wobble phase: first={first_dx}, second={second_dx}, divergence={divergence}"
    );
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
