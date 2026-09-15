use super::*;

#[test]
fn progress_at_start_is_zero() {
    let samples = Arc::new(vec![0.0_f32; 20]);
    let mut state = PlaybackState::new();
    state.start("test".into(), samples, 48000, 2, 1.0);
    assert_eq!(state.progress(), 0.0);
}

#[test]
fn progress_at_midpoint() {
    let samples = Arc::new(vec![0.0_f32; 20]);
    let mut state = PlaybackState::new();
    state.start("test".into(), samples, 48000, 2, 1.0);
    let mut buf = vec![0.0_f32; 10];
    state.fill_buffer(&mut buf);
    let p = state.progress();
    assert!((p - 0.5).abs() < f32::EPSILON, "expected ~0.5, got {p}");
}

#[test]
fn progress_at_end_is_one() {
    let samples = Arc::new(vec![0.0_f32; 20]);
    let mut state = PlaybackState::new();
    state.start("test".into(), samples, 48000, 2, 1.0);
    let mut buf = vec![0.0_f32; 20];
    state.fill_buffer(&mut buf);
    assert_eq!(state.progress(), 1.0);
}

#[test]
fn progress_with_no_samples_is_zero() {
    let state = PlaybackState::new();
    assert_eq!(state.progress(), 0.0);
}

#[test]
fn with_volume_sets_initial_volume() {
    let state = PlaybackState::with_volume(0.42);
    assert!((state.volume() - 0.42).abs() < f32::EPSILON);
    assert!(!state.is_active());
}

#[test]
fn with_volume_clamps_above_one() {
    let state = PlaybackState::with_volume(1.5);
    assert!((state.volume() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn with_volume_clamps_below_zero() {
    let state = PlaybackState::with_volume(-0.3);
    assert!((state.volume() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn fill_buffer_respects_initial_volume() {
    let samples = Arc::new(vec![1.0_f32; 100]);
    let mut state = PlaybackState::with_volume(0.5);
    state.start("test".into(), samples, 48000, 1, 1.0);

    let mut buf = vec![0.0_f32; 10];
    let wrote = state.fill_buffer(&mut buf);

    assert_eq!(wrote, 10);
    for &s in &buf[..wrote] {
        assert!(
            (s - 0.5).abs() < f32::EPSILON,
            "expected 0.5 (1.0 * 0.5 volume), got {s}"
        );
    }
}

#[test]
fn fill_buffer_multiplies_master_and_per_sound_gain() {
    // master 0.5 (with_volume) * per-sound gain 0.5 = 0.25 effective.
    let samples = Arc::new(vec![1.0_f32; 100]);
    let mut state = PlaybackState::with_volume(0.5);
    state.start("test".into(), samples, 48_000, 1, 0.5);

    let mut buf = vec![0.0_f32; 10];
    let wrote = state.fill_buffer(&mut buf);

    assert_eq!(wrote, 10);
    for &s in &buf[..wrote] {
        assert!((s - 0.25).abs() < f32::EPSILON, "expected 0.25, got {s}");
    }
}

#[test]
fn fill_buffer_preserves_per_sound_boost_above_unity() {
    // The per-sound volume slider ranges to 2.0; a boost above unity must
    // survive (the pre-#151 path scaled samples uncapped). master 1.0 *
    // gain 2.0 = 2.0, so a 0.4 sample becomes 0.8.
    let samples = Arc::new(vec![0.4_f32; 100]);
    let mut state = PlaybackState::with_volume(1.0);
    state.start("test".into(), samples, 48_000, 1, 2.0);

    let mut buf = vec![0.0_f32; 10];
    let wrote = state.fill_buffer(&mut buf);

    assert_eq!(wrote, 10);
    for &s in &buf[..wrote] {
        assert!(
            (s - 0.8).abs() < f32::EPSILON,
            "expected 0.8 boost, got {s}"
        );
    }
}
