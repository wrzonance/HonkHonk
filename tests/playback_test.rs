use std::sync::Arc;

use honkhonk::audio::playback::PlaybackState;

#[test]
fn new_playback_state_is_inactive() {
    let state = PlaybackState::new();
    assert!(!state.is_active());
}

#[test]
fn start_activates_playback() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![0.0f32; 4800]);
    state.start("test-sound".into(), samples, 48000, 2);
    assert!(state.is_active());
    assert_eq!(state.sound_id(), Some("test-sound"));
}

#[test]
fn fill_buffer_writes_samples_and_advances_cursor() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![0.5f32; 20]);
    state.start("s1".into(), samples, 48000, 2);

    let mut buf = vec![0.0f32; 10];
    let wrote = state.fill_buffer(&mut buf);
    assert_eq!(wrote, 10);
    assert!(buf.iter().all(|&s| s == 0.5));
}

#[test]
fn fill_buffer_applies_volume() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![1.0f32; 20]);
    state.start("s1".into(), samples, 48000, 2);
    state.set_volume(0.5);

    let mut buf = vec![0.0f32; 10];
    state.fill_buffer(&mut buf);
    assert!(buf.iter().all(|&s| (s - 0.5).abs() < 0.001));
}

#[test]
fn fill_buffer_returns_zero_when_exhausted() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![1.0f32; 4]);
    state.start("s1".into(), samples, 48000, 2);

    let mut buf = vec![0.0f32; 4];
    let wrote = state.fill_buffer(&mut buf);
    assert_eq!(wrote, 4);

    let mut buf2 = vec![0.0f32; 4];
    let wrote2 = state.fill_buffer(&mut buf2);
    assert_eq!(wrote2, 0);
    assert!(!state.is_active());
}

#[test]
fn stop_deactivates_and_resets() {
    let mut state = PlaybackState::new();
    let samples = Arc::new(vec![0.5f32; 100]);
    state.start("s1".into(), samples, 48000, 2);
    state.stop();
    assert!(!state.is_active());
    assert_eq!(state.sound_id(), None);
}

#[test]
fn fill_buffer_on_inactive_state_returns_zero() {
    let mut state = PlaybackState::new();
    let mut buf = vec![0.0f32; 10];
    let wrote = state.fill_buffer(&mut buf);
    assert_eq!(wrote, 0);
}

#[test]
fn set_volume_clamps_to_zero_one() {
    let mut state = PlaybackState::new();
    state.set_volume(1.5);
    assert_eq!(state.volume(), 1.0);
    state.set_volume(-0.3);
    assert_eq!(state.volume(), 0.0);
}
