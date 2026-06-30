//! Shared fixtures and helpers for the panel-feather flourish integration
//! tests, used by `side_panel_flourish.rs` (motion) and
//! `side_panel_flourish_emitter.rs` (emitter geometry). Each test binary
//! compiles this module independently, so not every helper is used in both;
//! `allow(dead_code, unused_imports)` keeps the unused-in-one-binary warnings
//! quiet (each binary uses a different subset of the helpers and re-exports).
#![allow(dead_code, unused_imports)]

use std::time::{Duration, Instant};

pub use honkhonk::ui::side_panel::{
    BurstEmitter, BurstLine, FeatherClass, FeatherParticle, PanelFlourish, PanelRect,
    panel_burst_emitter,
};
pub use iced::{Point, Vector};

pub fn right_panel() -> PanelRect {
    PanelRect {
        x: 880.0,
        y: 0.0,
        w: 400.0,
        h: 800.0,
        center: Point::new(1080.0, 400.0),
    }
}

pub fn left_panel() -> PanelRect {
    PanelRect {
        x: 0.0,
        y: 0.0,
        w: 320.0,
        h: 700.0,
        center: Point::new(160.0, 350.0),
    }
}

pub fn top_panel() -> PanelRect {
    PanelRect {
        x: 120.0,
        y: 0.0,
        w: 720.0,
        h: 180.0,
        center: Point::new(480.0, 90.0),
    }
}

pub fn bottom_panel() -> PanelRect {
    PanelRect {
        x: 120.0,
        y: 620.0,
        w: 720.0,
        h: 180.0,
        center: Point::new(480.0, 710.0),
    }
}

pub fn floating_panel() -> PanelRect {
    PanelRect {
        x: 300.0,
        y: 180.0,
        w: 420.0,
        h: 320.0,
        center: Point::new(510.0, 340.0),
    }
}

pub fn assert_line(actual: BurstLine, start: Point, end: Point, direction: Vector) {
    assert_eq!(actual.start, start);
    assert_eq!(actual.end, end);
    assert_eq!(actual.direction, direction);
}

pub fn first_particle_of(flourish: &PanelFlourish, class: FeatherClass) -> FeatherParticle {
    *flourish
        .particles()
        .iter()
        .find(|p| p.class == class)
        .expect("burst should include requested feather class")
}

pub fn size_range_for(flourish: &PanelFlourish, class: FeatherClass) -> f32 {
    let sizes = flourish
        .particles()
        .iter()
        .filter(|p| p.class == class)
        .map(|p| p.size)
        .collect::<Vec<_>>();
    assert!(
        sizes.len() > 1,
        "burst should include multiple particles for {class:?}"
    );
    let min = sizes.iter().copied().fold(f32::INFINITY, f32::min);
    let max = sizes.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    max - min
}

pub fn tick_for(flourish: &mut PanelFlourish, start: Instant, total: Duration) {
    let total_ms = total.as_millis() as u64;
    let mut elapsed_ms = 16;
    while elapsed_ms < total_ms {
        assert!(flourish.tick(start + Duration::from_millis(elapsed_ms), None));
        elapsed_ms += 16;
    }
    assert!(flourish.tick(start + total, None));
}
