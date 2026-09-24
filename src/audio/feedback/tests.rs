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
fn growth_over_twelve_db_in_fifty_ms_triggers_but_ordinary_changes_do_not() {
    let mut detector = FeedbackDetector::new(48_000, 2);
    assert!(!block(&mut detector, 0.1, 10));
    assert!(!block(&mut detector, 0.2, 10));
    assert!(block(&mut detector, 0.45, 10));
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
