//! Wall-clock playhead clock for the now-playing waveform. The line position is
//! a pure linear function of elapsed wall-clock time since playback start, so it
//! flows smoothly and monotonically left→right over the clip's duration.
//!
//! PipeWire plays in real time, so wall-clock elapsed tracks the audio. An
//! earlier predict-and-correct design re-anchored to the 10 Hz `Progress`
//! samples, but those samples are measured up to ~100 ms before they are
//! drained — binding a stale measurement to the current instant snapped the line
//! backward every drain (visible left/right jitter, #138). Driving purely from
//! the start instant removes the re-anchoring, and monotonicity is then
//! guaranteed by construction.

use std::time::{Duration, Instant};

/// Maps elapsed wall-clock time since playback start to a `0.0..=1.0` position.
#[derive(Debug, Clone)]
pub struct PlayheadClock {
    start: Instant,
    duration: Duration,
}

impl PlayheadClock {
    /// Starts the clock at playback start: progress `0.0` at `now`, reaching
    /// `1.0` after `duration`.
    pub fn new(duration: Duration, now: Instant) -> Self {
        Self {
            start: now,
            duration,
        }
    }

    /// Linear progress `0.0..=1.0` from elapsed wall-clock since `start`.
    /// Monotonic non-decreasing in `now` (`Instant` is monotonic); a zero
    /// `duration` reads `0.0`.
    pub fn display(&self, now: Instant) -> f32 {
        fraction(now.saturating_duration_since(self.start), self.duration)
    }
}

/// Pure core: `elapsed / duration`, clamped to `0.0..=1.0`. A zero `duration`
/// yields `0.0` (no division).
fn fraction(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 0.0;
    }
    (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_midpoint() {
        let p = fraction(Duration::from_secs(5), Duration::from_secs(10));
        assert!((p - 0.5).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn fraction_clamps_at_end() {
        let p = fraction(Duration::from_secs(100), Duration::from_secs(10));
        assert_eq!(p, 1.0);
    }

    #[test]
    fn fraction_zero_duration_is_zero() {
        let p = fraction(Duration::from_secs(5), Duration::ZERO);
        assert_eq!(p, 0.0);
    }

    #[test]
    fn display_advances_linearly_with_time() {
        let t0 = Instant::now();
        let clock = PlayheadClock::new(Duration::from_secs(10), t0);
        assert!((clock.display(t0) - 0.0).abs() < 1e-6);
        assert!((clock.display(t0 + Duration::from_secs(2)) - 0.2).abs() < 1e-6);
        assert!((clock.display(t0 + Duration::from_secs(5)) - 0.5).abs() < 1e-6);
        assert_eq!(clock.display(t0 + Duration::from_secs(10)), 1.0);
    }

    #[test]
    fn display_is_monotonic_non_decreasing() {
        // The fix: the line must never move backward as time advances. Densely
        // sampling a short clip must yield a non-decreasing sequence ending at 1.0.
        let t0 = Instant::now();
        let clock = PlayheadClock::new(Duration::from_millis(500), t0);
        let mut prev = 0.0;
        for ms in 0..=600 {
            let v = clock.display(t0 + Duration::from_millis(ms));
            assert!(v >= prev, "went backward at {ms}ms: {v} < {prev}");
            prev = v;
        }
        assert_eq!(prev, 1.0);
    }

    #[test]
    fn display_never_below_zero() {
        let t0 = Instant::now();
        let clock = PlayheadClock::new(Duration::from_secs(1), t0);
        assert!(clock.display(t0) >= 0.0);
    }
}
