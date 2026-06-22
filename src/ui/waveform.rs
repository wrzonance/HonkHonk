//! Pure, deterministic waveform-sample generation and the render-key used to
//! decide when the now-playing `canvas::Cache` must be invalidated (#131).
//!
//! No Iced types live here so the cache-lifecycle decision stays unit-testable
//! without a renderer (ADR-009: prove the persistent-cache pattern first).

use std::hash::{Hash, Hasher};

/// Number of vertical bars in the now-playing waveform.
pub const WAVEFORM_BARS: usize = 48;

/// Quantization resolution for progress. The cache is keyed on the bucket, not
/// the raw float, so identical-looking frames reuse the cached geometry instead
/// of re-tessellating every sub-pixel `progress` tick.
pub const PROGRESS_BUCKETS: u16 = 240;

/// Deterministic bar heights for a sound id, each in `0.15..=1.0`.
pub fn samples(id: &str) -> [f32; WAVEFORM_BARS] {
    std::array::from_fn(|i| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut h);
        (i as u64).hash(&mut h);
        // Map the hash into 0.15..=1.0 so no bar fully disappears.
        let frac = (h.finish() % 1000) as f32 / 1000.0;
        0.15 + frac * 0.85
    })
}

/// Quantizes progress into `0..=PROGRESS_BUCKETS`.
pub fn progress_bucket(progress: f32) -> u16 {
    let p = progress.clamp(0.0, 1.0);
    (p * PROGRESS_BUCKETS as f32).round() as u16
}

/// What the cached waveform depends on. When this changes between frames the
/// cache must be cleared; when it is unchanged the cache is reused verbatim.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RenderKey {
    pub id: Option<String>,
    pub bucket: u16,
}

pub fn render_key(playing: Option<&str>, progress: f32) -> RenderKey {
    RenderKey {
        id: playing.map(str::to_owned),
        bucket: progress_bucket(progress),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_deterministic_for_same_id() {
        assert_eq!(samples("abc123"), samples("abc123"));
    }

    #[test]
    fn samples_differ_across_ids() {
        assert_ne!(samples("abc123"), samples("def456"));
    }

    #[test]
    fn samples_stay_in_visible_range() {
        for v in samples("any-id") {
            assert!((0.15..=1.0).contains(&v), "bar {v} out of range");
        }
    }

    #[test]
    fn progress_bucket_is_monotonic_and_bounded() {
        assert_eq!(progress_bucket(-1.0), 0);
        assert_eq!(progress_bucket(0.0), 0);
        assert_eq!(progress_bucket(1.0), PROGRESS_BUCKETS);
        assert_eq!(progress_bucket(2.0), PROGRESS_BUCKETS);
        assert!(progress_bucket(0.25) <= progress_bucket(0.5));
    }

    #[test]
    fn tiny_progress_changes_share_a_bucket() {
        // Sub-bucket jitter must NOT change the key (else the cache thrashes).
        assert_eq!(progress_bucket(0.5000), progress_bucket(0.5001));
    }

    #[test]
    fn render_key_changes_with_sound_and_bucket() {
        let a = render_key(Some("s1"), 0.0);
        let b = render_key(Some("s2"), 0.0);
        let c = render_key(Some("s1"), 1.0);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, render_key(Some("s1"), 0.0));
    }

    #[test]
    fn render_key_none_when_idle() {
        assert_eq!(render_key(None, 0.7).id, None);
    }
}
