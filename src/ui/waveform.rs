//! Pure, deterministic waveform-sample generation and the render-key used to
//! decide when the now-playing `canvas::Cache` must be invalidated (#131).
//!
//! No Iced types live here so the cache-lifecycle decision stays unit-testable
//! without a renderer (ADR-009: prove the persistent-cache pattern first).

use std::hash::{Hash, Hasher};

/// Number of vertical bars in the now-playing waveform.
pub const WAVEFORM_BARS: usize = 48;

/// Quantization resolution for progress — one bucket per bar, matching
/// `WAVEFORM_BARS`. The cached bars are the only cache-sensitive content, and
/// they change only when `played_to` crosses a bar boundary, so `WAVEFORM_BARS`
/// distinct buckets are sufficient. The smooth playhead overlay uses raw
/// `progress` directly and does NOT depend on this bucket, so no visual
/// precision is lost.
///
/// This is the max bucket INDEX (inclusive); there are `PROGRESS_BUCKETS + 1`
/// distinct bucket values, including the exact-end bucket at `progress == 1.0`.
pub const PROGRESS_BUCKETS: u16 = WAVEFORM_BARS as u16;

/// Deterministic bar heights for a sound id, each in `0.15..=1.0`.
///
/// The id is hashed once up front; each bar mixes in its index via a
/// cheap integer multiply, so the id is not re-hashed 48 times.
pub fn samples(id: &str) -> [f32; WAVEFORM_BARS] {
    use std::collections::hash_map::DefaultHasher;
    // Hash the id once.
    let mut h = DefaultHasher::new();
    id.hash(&mut h);
    let base: u64 = h.finish();

    std::array::from_fn(|i| {
        // Mix bar index into the base hash with a fast integer combine.
        // All arithmetic is wrapping to avoid overflow in debug builds.
        let mixed = base
            .wrapping_mul(6364136223846793005)
            .wrapping_add((i as u64).wrapping_mul(1442695040888963407).wrapping_add(1));
        // Map the mixed value into 0.15..=1.0 so no bar fully disappears.
        let frac = (mixed % 1000) as f32 / 1000.0;
        0.15 + frac * 0.85
    })
}

/// Quantizes progress into `0..=PROGRESS_BUCKETS` (inclusive). Returns the
/// max bucket index `PROGRESS_BUCKETS` when `progress >= 1.0`, so there are
/// `PROGRESS_BUCKETS + 1` distinct return values in total.
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

impl RenderKey {
    /// Whether this key already describes `(playing, progress)` — the
    /// allocation-free equivalent of `*self == render_key(playing, progress)`.
    /// Lets the cache hot path (called every `update`) skip building an owned
    /// `id` on the common no-change frame.
    pub fn matches(&self, playing: Option<&str>, progress: f32) -> bool {
        self.bucket == progress_bucket(progress) && self.id.as_deref() == playing
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

    #[test]
    fn matches_is_allocation_free_equivalent_of_render_key() {
        let key = render_key(Some("s1"), 0.5);
        // Same id + same bucket (incl. sub-bucket jitter) ⇒ matches.
        assert!(key.matches(Some("s1"), 0.5));
        assert!(key.matches(Some("s1"), 0.5001));
        // Any of id / bucket / playing-state differing ⇒ no match.
        assert!(!key.matches(Some("s2"), 0.5));
        assert!(!key.matches(Some("s1"), 1.0));
        assert!(!key.matches(None, 0.5));
        // Equivalence with the owned-key comparison it replaces.
        for (p, id) in [(0.5_f32, Some("s1")), (1.0, Some("s2")), (0.0, None)] {
            assert_eq!(key.matches(id, p), key == render_key(id, p));
        }
    }
}
