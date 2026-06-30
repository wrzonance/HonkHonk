//! Motion and animation tests for the panel-feather flourish: drag, wobble,
//! gravity, fade, and cursor interaction. Emitter geometry lives in
//! `side_panel_flourish_emitter.rs`; shared fixtures in `flourish_support`.
mod flourish_support;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use flourish_support::*;

#[test]
fn burst_seeds_all_feather_classes() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);

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
fn feather_classes_have_distinct_visual_sizes() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);

    let dust_size = first_particle_of(&flourish, FeatherClass::Dust).size;
    let chunk_size = first_particle_of(&flourish, FeatherClass::Chunk).size;
    let feather_size = first_particle_of(&flourish, FeatherClass::Feather).size;

    assert!(dust_size < chunk_size);
    assert!(chunk_size < feather_size);
}

#[test]
fn same_class_particles_have_seed_variation() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);

    assert!(size_range_for(&flourish, FeatherClass::Dust) > 0.5);
    assert!(size_range_for(&flourish, FeatherClass::Chunk) > 0.5);
    assert!(size_range_for(&flourish, FeatherClass::Feather) > 0.5);
}

#[test]
fn dust_descends_farther_than_full_feather() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);

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

    // Assert the ordering invariant, not a tuning-specific pixel gap: an
    // aesthetic retune that keeps "dust descends farther" must stay green.
    assert!(
        dust_drop > feather_drop,
        "dust should fall faster than a full feather: dust={dust_drop}, feather={feather_drop}"
    );
}

#[test]
fn full_feathers_swoop_more_than_dust() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);

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

    // Ordering invariant only — no absolute-pixel coupling to current tuning.
    assert!(
        feather_dx > dust_dx,
        "full feathers should have more lateral swoop: dust={dust_dx}, feather={feather_dx}"
    );
}

#[test]
fn long_frame_hitch_does_not_snap_feather_drag_to_zero() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);

    let feather_start = first_particle_of(&flourish, FeatherClass::Feather)
        .position
        .y;

    assert!(flourish.tick(now + Duration::from_millis(900), None));

    let feather_drop = first_particle_of(&flourish, FeatherClass::Feather)
        .position
        .y
        - feather_start;

    assert!(
        feather_drop > 1.0,
        "frame-independent drag should preserve visible descent after a hitch: {feather_drop}"
    );
}

#[test]
fn same_class_feathers_diverge_laterally() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);

    // Record each full feather's seed lateral position, then tick and measure
    // how far each drifted sideways. Distinct seeded wobble phases must make the
    // class spread, not move in lockstep. Asserting spread across the whole set
    // avoids depending on two particles happening to share identical seeds.
    let starts: Vec<(usize, f32)> = flourish
        .particles()
        .iter()
        .enumerate()
        .filter(|(_, p)| p.class == FeatherClass::Feather)
        .map(|(i, p)| (i, p.position.x))
        .collect();
    assert!(starts.len() >= 2, "burst should seed multiple full feathers");

    tick_for(&mut flourish, now, Duration::from_millis(1200));

    let drifts: Vec<f32> = starts
        .iter()
        .map(|(i, start_x)| flourish.particles()[*i].position.x - start_x)
        .collect();
    let spread = drifts.iter().copied().fold(f32::MIN, f32::max)
        - drifts.iter().copied().fold(f32::MAX, f32::min);

    assert!(
        spread > 4.0,
        "same-class feathers should diverge laterally, not move in lockstep: drifts={drifts:?}"
    );
}

#[test]
fn feathers_float_down_and_fade_out() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);
    let start_y = flourish.particles()[0].position.y;
    assert!(flourish.tick(now + Duration::from_millis(1500), None));
    let mid = flourish.particles()[0];
    assert!(mid.position.y > start_y);
    assert!(mid.alpha > 0.0 && mid.alpha < 1.0);
    assert!(!flourish.tick(now + Duration::from_millis(3100), None));
    assert!(!flourish.is_animating());
}

#[test]
fn all_classes_clear_at_shared_burst_duration() {
    let now = Instant::now();
    let mut flourish = PanelFlourish::default();
    flourish.emit(right_panel(), (1280.0, 800.0), now);

    assert!(
        flourish
            .particles()
            .iter()
            .any(|p| p.class == FeatherClass::Dust)
    );
    assert!(
        flourish
            .particles()
            .iter()
            .any(|p| p.class == FeatherClass::Chunk)
    );
    assert!(
        flourish
            .particles()
            .iter()
            .any(|p| p.class == FeatherClass::Feather)
    );

    assert!(!flourish.tick(now + honkhonk::ui::side_panel::BURST_DURATION, None));
    assert!(!flourish.is_animating());
    assert!(flourish.particles().is_empty());
}

#[test]
fn cursor_gently_bumps_nearby_feathers() {
    let now = Instant::now();
    let mut plain = PanelFlourish::default();
    plain.emit(right_panel(), (1280.0, 800.0), now);
    let mut bumped = plain.clone();
    let first = plain.particles()[0];
    let cursor = Point::new(first.position.x + 8.0, first.position.y);
    plain.tick(now + Duration::from_millis(16), None);
    bumped.tick(now + Duration::from_millis(16), Some(cursor));
    let plain_dx = plain.particles()[0].position.x - first.position.x;
    let bumped_dx = bumped.particles()[0].position.x - first.position.x;
    assert!(bumped_dx < plain_dx, "cursor should push feather away");
}
