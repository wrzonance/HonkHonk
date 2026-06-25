use std::time::{Duration, Instant};

use honkhonk::ui::side_panel::{
    BurstSource, PanelFlourish, PanelRect, PanelTransition, panel_burst_origin,
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

#[test]
fn right_docked_panel_emits_from_inner_edge_away_from_panel() {
    let origin = panel_burst_origin(right_panel(), (1280.0, 800.0));
    assert_eq!(origin.point, Point::new(880.0, 400.0));
    assert_eq!(origin.source, BurstSource::Edge(Vector::new(-1.0, 0.0)));
}

#[test]
fn floating_panel_emits_from_center() {
    let origin = panel_burst_origin(
        PanelRect {
            x: 300.0,
            y: 180.0,
            w: 420.0,
            h: 320.0,
            center: Point::new(510.0, 340.0),
        },
        (1280.0, 800.0),
    );
    assert_eq!(origin.point, Point::new(510.0, 340.0));
    assert_eq!(origin.source, BurstSource::Center);
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
