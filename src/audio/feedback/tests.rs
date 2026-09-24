use super::*;

fn block(detector: &mut FeedbackDetector, amplitude: f32, milliseconds: usize) -> bool {
    detector.observe(std::iter::repeat_n(amplitude, 48 * 2 * milliseconds))
}

#[test]
fn sustained_rms_requires_two_hundred_milliseconds_and_has_cooldown() {
    let mut detector = FeedbackDetector::new(48_000, 2);
    assert!(!block(&mut detector, 0.7, 190));
    assert!(block(&mut detector, 0.7, 10));
    assert!(!block(&mut detector, 0.7, 1900));
}

#[test]
fn low_level_resets_sustained_window_and_silence_does_not_trigger() {
    let mut detector = FeedbackDetector::new(48_000, 2);
    assert!(!block(&mut detector, 0.0, 100));
    assert!(!block(&mut detector, 0.55, 190));
    assert!(!block(&mut detector, 0.4, 10));
    assert!(!block(&mut detector, 0.55, 190));
    assert!(block(&mut detector, 0.55, 10));
}

#[test]
fn growth_requires_three_consecutive_windows_from_an_already_loud_baseline() {
    let mut detector = FeedbackDetector::new(48_000, 2);
    assert!(!block(&mut detector, 0.11, 10));
    assert!(!block(&mut detector, 0.2, 10));
    assert!(!block(&mut detector, 0.45, 10));
    assert!(block(&mut detector, 0.6, 10));
}

#[test]
fn ordinary_quiet_to_loud_onset_does_not_trip() {
    let mut detector = FeedbackDetector::new(48_000, 2);
    assert!(!block(&mut detector, 0.02, 20));
    assert!(!block(&mut detector, 0.7, 10));
}

#[test]
fn plateau_breaks_consecutive_growth() {
    let mut detector = FeedbackDetector::new(48_000, 2);
    for amplitude in [0.11, 0.2, 0.2, 0.3, 0.5] {
        assert!(!block(&mut detector, amplitude, 10));
    }
}

#[test]
fn chunk_boundaries_and_channel_count_do_not_change_detection_time() {
    let mut detector = FeedbackDetector::new(48_000, 1);
    for _ in 0..199 {
        assert!(!detector.observe(std::iter::repeat_n(0.7, 48)));
    }
    assert!(detector.observe(std::iter::repeat_n(0.7, 48)));
}

#[test]
fn nonfinite_samples_trip_safety() {
    let mut detector = FeedbackDetector::new(48_000, 2);
    assert!(detector.observe([f32::NAN].into_iter()));
}
