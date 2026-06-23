//! Predict-and-correct clock for the now-playing playhead. The 10 Hz audio
//! `AudioEvent::Progress` events are the authoritative anchor; `display`
//! extrapolates between them from wall-clock elapsed so the line moves at the
//! display refresh rate without drifting (#138, PR-A).

use std::time::{Duration, Instant};

/// Holds the last authoritative progress sample and extrapolates a smooth
/// display position from wall-clock time since that sample.
#[derive(Debug, Clone)]
pub struct PlayheadClock {
    anchor: f32,
    anchor_at: Instant,
    duration: Duration,
}

impl PlayheadClock {
    /// Starts a clock at progress 0 for a sound of length `duration`.
    pub fn new(duration: Duration, now: Instant) -> Self {
        Self {
            anchor: 0.0,
            anchor_at: now,
            duration,
        }
    }

    /// Re-anchors to an authoritative progress sample, snapping out any
    /// accumulated prediction error (forward or backward).
    pub fn on_progress(&mut self, progress: f32, now: Instant) {
        self.anchor = progress.clamp(0.0, 1.0);
        self.anchor_at = now;
    }

    /// Extrapolated display progress at `now`, clamped to `0.0..=1.0`.
    pub fn display(&self, now: Instant) -> f32 {
        extrapolate(
            self.anchor,
            now.saturating_duration_since(self.anchor_at),
            self.duration,
        )
    }
}

/// Pure extrapolation core: `anchor + elapsed/duration`, clamped to `0..=1`.
/// A zero `duration` yields the clamped anchor (no division).
fn extrapolate(anchor: f32, elapsed: Duration, duration: Duration) -> f32 {
    let anchor = anchor.clamp(0.0, 1.0);
    if duration.is_zero() {
        return anchor;
    }
    (anchor + elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrapolate_midpoint() {
        let p = extrapolate(0.0, Duration::from_secs(5), Duration::from_secs(10));
        assert!((p - 0.5).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn extrapolate_clamps_at_end() {
        let p = extrapolate(0.9, Duration::from_secs(100), Duration::from_secs(10));
        assert_eq!(p, 1.0);
    }

    #[test]
    fn extrapolate_zero_duration_returns_anchor() {
        let p = extrapolate(0.4, Duration::from_secs(5), Duration::ZERO);
        assert!((p - 0.4).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn display_advances_with_time() {
        let t0 = Instant::now();
        let clock = PlayheadClock::new(Duration::from_secs(10), t0);
        let early = clock.display(t0 + Duration::from_secs(2));
        let late = clock.display(t0 + Duration::from_secs(8));
        assert!(late > early, "expected {late} > {early}");
        assert!((early - 0.2).abs() < 1e-6, "got {early}");
    }

    #[test]
    fn on_progress_snaps_back_an_overshooting_prediction() {
        let t0 = Instant::now();
        let mut clock = PlayheadClock::new(Duration::from_secs(10), t0);
        let predicted = clock.display(t0 + Duration::from_secs(8));
        assert!((predicted - 0.8).abs() < 1e-6, "got {predicted}");
        // Authoritative sample says we are only at 0.3 — snap back.
        let t1 = t0 + Duration::from_secs(8);
        clock.on_progress(0.3, t1);
        assert!((clock.display(t1) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn display_never_exceeds_one_or_drops_below_zero() {
        let t0 = Instant::now();
        let clock = PlayheadClock::new(Duration::from_secs(1), t0);
        assert_eq!(clock.display(t0 + Duration::from_secs(100)), 1.0);
        assert!(clock.display(t0) >= 0.0);
    }
}
