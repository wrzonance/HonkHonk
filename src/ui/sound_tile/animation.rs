use std::time::{Duration, Instant};

pub const HOVER_ANIMATION_DURATION: Duration = Duration::from_millis(150);
const HOVER_ROTATION_SCALE: f32 = 8.0 / 3.0;

pub fn hover_rotation_degrees(seed: u64, hover_progress: f32) -> f32 {
    let scale = 1.0 + (HOVER_ROTATION_SCALE - 1.0) * hover_progress.clamp(0.0, 1.0);
    super::rotation_degrees(seed) * scale
}

#[derive(Debug, Clone, Copy)]
pub struct HoverAnimation {
    progress: f32,
    from: f32,
    to: f32,
    started_at: Instant,
}

impl Default for HoverAnimation {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            progress: 0.0,
            from: 0.0,
            to: 0.0,
            started_at: now,
        }
    }
}

impl HoverAnimation {
    pub fn retargeted(from_hovered: bool, to_hovered: bool, now: Instant) -> Self {
        Self {
            progress: f32::from(from_hovered),
            from: f32::from(from_hovered),
            to: f32::from(to_hovered),
            started_at: now,
        }
    }

    pub fn retarget(self, hovered: bool, now: Instant) -> Self {
        let progress = self.progress_at(now);
        Self {
            progress,
            from: progress,
            to: f32::from(hovered),
            started_at: now,
        }
    }

    pub fn retarget_if_changed(&mut self, hovered: bool, now: Instant) -> bool {
        if (self.to - f32::from(hovered)).abs() < f32::EPSILON {
            return false;
        }
        *self = self.retarget(hovered, now);
        true
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        let next = self.progress_at(now);
        let changed = (next - self.progress).abs() > f32::EPSILON;
        self.progress = next;
        if !self.is_animating_at(now) {
            self.from = self.to;
            self.progress = self.to;
        }
        changed
    }

    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn is_animating_at(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started_at) < HOVER_ANIMATION_DURATION
            && (self.from - self.to).abs() > f32::EPSILON
    }

    pub fn progress_at(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.started_at);
        let progress = elapsed.as_secs_f32() / HOVER_ANIMATION_DURATION.as_secs_f32();
        self.from + (self.to - self.from) * ease_out(progress)
    }
}

fn ease_out(progress: f32) -> f32 {
    let inverse = 1.0 - progress.clamp(0.0, 1.0);
    1.0 - inverse * inverse * inverse
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn hover_rotation_preserves_idle_rotation_at_zero_progress() {
        let seed = 6_000;

        assert_eq!(hover_rotation_degrees(seed, 0.0), 3.0);
    }

    #[test]
    fn hover_rotation_amplifies_idle_range_to_eight_degrees() {
        assert_eq!(hover_rotation_degrees(6_000, 1.0), 8.0);
        assert_eq!(hover_rotation_degrees(0, 1.0), -8.0);
    }

    #[test]
    fn hover_animation_eases_out_over_150ms() {
        let t0 = Instant::now();
        let anim = HoverAnimation::retargeted(false, true, t0);
        let mid = anim.progress_at(t0 + Duration::from_millis(75));

        assert!(
            mid > 0.5,
            "ease-out should advance past linear midpoint: {mid}"
        );
        assert_eq!(anim.progress_at(t0 + HOVER_ANIMATION_DURATION), 1.0);
    }

    #[test]
    fn hover_animation_exit_starts_from_current_progress() {
        let t0 = Instant::now();
        let entering = HoverAnimation::retargeted(false, true, t0);
        let mid = t0 + Duration::from_millis(75);
        let exiting = entering.retarget(false, mid);

        assert!(
            (exiting.progress_at(mid) - entering.progress_at(mid)).abs() < 1e-6,
            "retargeting should be continuous"
        );
        assert_eq!(exiting.progress_at(mid + HOVER_ANIMATION_DURATION), 0.0);
    }
}
