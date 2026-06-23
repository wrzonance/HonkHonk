//! Wall-clock open/close animation for side panels. Mirrors the playhead's
//! drift-free wall-clock approach (#139): progress is a pure function of elapsed
//! time since the current leg began, eased with smoothstep — no predict-and-correct.

use std::time::{Duration, Instant};

/// Duration of a full open or close slide.
pub const SLIDE_DURATION: Duration = Duration::from_millis(150);

/// Open/close animation state. `0.0` = fully closed, `1.0` = fully open.
#[derive(Debug, Clone, Copy)]
pub enum PanelAnim {
    /// Steady at `progress` (`0.0` closed or `1.0` open).
    Settled(f32),
    /// Mid-slide: `from`→`to` over `SLIDE_DURATION` starting at `start`.
    Animating { from: f32, to: f32, start: Instant },
}

impl Default for PanelAnim {
    fn default() -> Self {
        PanelAnim::Settled(0.0)
    }
}

impl PanelAnim {
    /// Eased progress `0.0..=1.0` at `now`, without mutating state.
    pub fn progress(&self, now: Instant) -> f32 {
        match *self {
            PanelAnim::Settled(v) => v,
            PanelAnim::Animating { from, to, start } => {
                let frac = ease(fraction(
                    now.saturating_duration_since(start),
                    SLIDE_DURATION,
                ));
                from + (to - from) * frac
            }
        }
    }

    /// Settles the leg once it has fully elapsed, then returns progress at `now`.
    /// Call once per frame.
    pub fn tick(&mut self, now: Instant) -> f32 {
        if let PanelAnim::Animating { to, start, .. } = *self {
            if now.saturating_duration_since(start) >= SLIDE_DURATION {
                *self = PanelAnim::Settled(to);
            }
        }
        self.progress(now)
    }

    /// True while a slide is in progress (drives the frame subscription).
    pub fn is_animating(&self) -> bool {
        matches!(self, PanelAnim::Animating { .. })
    }

    /// True when open or *opening* (target progress > 0). Drives slide direction
    /// in [`toggle`](Self::toggle)/[`close`](Self::close): a closing panel reads
    /// `false` so a tab press reverses it back open. For "is the drawer drawn on
    /// screen right now" (e.g. should Escape dismiss it), use
    /// [`is_visible`](Self::is_visible) instead — it stays `true` mid-close.
    pub fn is_open(&self) -> bool {
        self.target() > 0.0
    }

    /// True whenever the drawer occupies the screen — open, opening, *or* closing
    /// (only fully `Settled(0.0)` reads `false`). Use this to decide whether the
    /// panel should absorb a dismiss (Escape) rather than [`is_open`](Self::is_open),
    /// which is `false` during the close slide.
    pub fn is_visible(&self) -> bool {
        !matches!(self, PanelAnim::Settled(v) if *v == 0.0)
    }

    /// Reverses or begins a slide toward the opposite end, continuous from the
    /// current visible progress (no snap on mid-slide reversal).
    pub fn toggle(&mut self, now: Instant) {
        let to = if self.is_open() { 0.0 } else { 1.0 };
        self.retarget(to, now);
    }

    /// Slides toward closed, continuous from current progress. No-op if already
    /// fully closed or closing.
    pub fn close(&mut self, now: Instant) {
        if self.is_open() {
            self.retarget(0.0, now);
        }
    }

    fn target(&self) -> f32 {
        match *self {
            PanelAnim::Settled(v) => v,
            PanelAnim::Animating { to, .. } => to,
        }
    }

    fn retarget(&mut self, to: f32, now: Instant) {
        let from = self.progress(now);
        *self = if (from - to).abs() < f32::EPSILON {
            PanelAnim::Settled(to)
        } else {
            PanelAnim::Animating {
                from,
                to,
                start: now,
            }
        };
    }
}

/// `elapsed / duration` clamped to `0.0..=1.0`; zero duration yields `1.0`
/// (instantly complete — never divides by zero).
fn fraction(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

/// Smoothstep ease-in-out on `0.0..=1.0`: zero velocity at both ends.
fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn default_is_closed() {
        let a = PanelAnim::default();
        let now = Instant::now();
        assert_eq!(a.progress(now), 0.0);
        assert!(!a.is_open());
        assert!(!a.is_animating());
    }

    #[test]
    fn toggle_opens_then_settles_to_one() {
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        a.toggle(t0);
        assert!(a.is_open());
        assert!(a.is_animating());
        assert!(a.progress(t0) < 0.01);
        let settled = a.tick(t0 + SLIDE_DURATION);
        assert_eq!(settled, 1.0);
        assert!(!a.is_animating());
        assert!(a.is_open());
    }

    #[test]
    fn toggle_again_closes_to_zero() {
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        a.toggle(t0);
        a.tick(t0 + SLIDE_DURATION); // now Open
        let t1 = t0 + SLIDE_DURATION;
        a.toggle(t1);
        assert!(!a.is_open());
        assert!(a.is_animating());
        let settled = a.tick(t1 + SLIDE_DURATION);
        assert_eq!(settled, 0.0);
        assert!(!a.is_animating());
    }

    #[test]
    fn progress_monotonic_and_clamped_while_opening() {
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        a.toggle(t0);
        let mut prev = 0.0;
        for ms in 0..=200 {
            let v = a.progress(t0 + Duration::from_millis(ms));
            assert!((0.0..=1.0).contains(&v), "out of range at {ms}ms: {v}");
            assert!(v >= prev - 1e-6, "went backward at {ms}ms: {v} < {prev}");
            prev = v;
        }
        assert_eq!(a.progress(t0 + Duration::from_millis(200)), 1.0);
    }

    #[test]
    fn mid_slide_reversal_is_continuous() {
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        a.toggle(t0); // opening
        let mid = t0 + Duration::from_millis(75);
        let before = a.progress(mid);
        a.toggle(mid); // reverse to closing
        let after = a.progress(mid);
        assert!(
            (after - before).abs() < 1e-3,
            "snapped: {before} -> {after}"
        );
        assert!(!a.is_open());
    }

    #[test]
    fn is_visible_stays_true_through_close_slide() {
        // Regression: is_open() is target-based and reads false during the close
        // slide; is_visible() must stay true so a dismiss (Escape) is absorbed by
        // the still-on-screen drawer instead of falling through to other handlers.
        let t0 = Instant::now();
        let mut a = PanelAnim::default();
        assert!(!a.is_visible());
        a.toggle(t0); // opening
        assert!(a.is_visible());
        a.tick(t0 + SLIDE_DURATION); // settled open
        assert!(a.is_visible());
        let t1 = t0 + SLIDE_DURATION;
        a.toggle(t1); // closing
        assert!(!a.is_open(), "closing reads not-open (target-based)");
        assert!(a.is_visible(), "but is still on screen mid-close");
        a.tick(t1 + SLIDE_DURATION); // settled closed
        assert!(!a.is_visible());
    }

    #[test]
    fn close_is_idempotent_when_closed() {
        let mut a = PanelAnim::default();
        let now = Instant::now();
        a.close(now);
        assert!(!a.is_open());
        assert!(!a.is_animating());
    }

    #[test]
    fn ease_hits_endpoints_and_midpoint() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        assert!((ease(0.5) - 0.5).abs() < 1e-6);
    }
}
