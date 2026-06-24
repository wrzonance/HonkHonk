use super::*;

#[test]
fn first_sync_clears_and_stores_key() {
    let mut np = NowPlaying::default();
    assert!(np.current_key().is_none());
    let cleared = np.sync(Some("s1"), 0.0);
    assert!(cleared, "first sync must populate the cache key");
    assert!(np.current_key().is_some());
}

#[test]
fn start_creates_playhead_resets_display_and_caches_envelope() {
    use std::time::{Duration, Instant};

    let mut np = NowPlaying::default();
    let t0 = Instant::now();
    let samples = vec![0.25_f32; 128];

    np.start(PlaybackStart {
        id: "s1",
        duration: Duration::from_secs(4),
        samples: &samples,
        channels: 1,
        now: t0,
    });

    assert!(np.has_playhead(), "start must create the playhead");
    assert_eq!(np.display_progress(), 0.0);
    assert!(
        np.current_key()
            .is_some_and(|key| key.matches(Some("s1"), 0.0)),
        "start must sync the waveform cache to the active sound"
    );
    let env = np
        .envelope("s1")
        .expect("start must cache the waveform envelope");
    assert_eq!(
        env.bars(crate::ui::waveform::WAVEFORM_BARS).len(),
        crate::ui::waveform::WAVEFORM_BARS
    );
}

#[test]
fn tick_advances_display_progress_from_the_owned_playhead() {
    use std::time::{Duration, Instant};

    let mut np = NowPlaying::default();
    let t0 = Instant::now();
    let samples = vec![0.25_f32; 128];
    np.start(PlaybackStart {
        id: "s1",
        duration: Duration::from_secs(10),
        samples: &samples,
        channels: 1,
        now: t0,
    });

    np.tick(t0 + Duration::from_secs(5));
    let midpoint = np.display_progress();
    np.tick(t0 + Duration::from_secs(7));

    assert!((midpoint - 0.5).abs() < 1e-3, "got {midpoint}");
    assert!(
        np.display_progress() >= midpoint,
        "tick must keep display progress monotonic"
    );
}

#[test]
fn clear_resets_playback_without_dropping_cached_envelope() {
    use std::time::{Duration, Instant};

    let mut np = NowPlaying::default();
    let t0 = Instant::now();
    let samples = vec![0.25_f32; 128];
    np.start(PlaybackStart {
        id: "s1",
        duration: Duration::from_secs(10),
        samples: &samples,
        channels: 1,
        now: t0,
    });
    np.tick(t0 + Duration::from_secs(5));

    np.clear();

    assert!(!np.has_playhead(), "clear must stop the playhead");
    assert_eq!(np.display_progress(), 0.0);
    assert!(
        np.current_key().is_some_and(|key| key.matches(None, 0.0)),
        "clear must sync the waveform cache to idle"
    );
    assert!(
        np.envelope("s1").is_some(),
        "per-sound envelopes remain cached across playback stops"
    );
}

#[test]
fn same_state_reuses_cache() {
    let mut np = NowPlaying::default();
    np.sync(Some("s1"), 0.5);
    let cleared = np.sync(Some("s1"), 0.5);
    assert!(
        !cleared,
        "identical state must reuse the cache, not clear it"
    );
}

#[test]
fn sub_bucket_progress_jitter_reuses_cache() {
    let mut np = NowPlaying::default();
    np.sync(Some("s1"), 0.5000);
    // A tiny progress tick within the same bucket must NOT thrash the cache.
    // This assertion depends on the bucket width (1/PROGRESS_BUCKETS ~ 0.021)
    // being larger than the 0.0001 jitter.
    let cleared = np.sync(Some("s1"), 0.5001);
    assert!(!cleared, "sub-bucket jitter must not invalidate the cache");
}

#[test]
fn changing_sound_clears_cache() {
    let mut np = NowPlaying::default();
    np.sync(Some("s1"), 0.5);
    assert!(np.sync(Some("s2"), 0.5), "new sound must invalidate cache");
}

#[test]
fn crossing_a_progress_bucket_clears_cache() {
    let mut np = NowPlaying::default();
    np.sync(Some("s1"), 0.0);
    assert!(
        np.sync(Some("s1"), 1.0),
        "large progress jump must invalidate"
    );
}

#[test]
fn stopping_playback_clears_cache() {
    let mut np = NowPlaying::default();
    np.sync(Some("s1"), 0.5);
    assert!(np.sync(None, 0.0), "stopping must invalidate the cache");
}

#[test]
fn steady_playback_frames_reuse_cache() {
    let mut np = NowPlaying::default();
    let mut clears = 0;
    for _ in 0..60 {
        if np.sync(Some("s1"), 0.5) {
            clears += 1;
        }
    }
    assert_eq!(clears, 1, "only the first frame may clear; rest reuse");
}

#[test]
fn smooth_progress_clears_once_per_bucket_not_per_frame() {
    use crate::ui::waveform::PROGRESS_BUCKETS;

    let mut np = NowPlaying::default();
    let step = 1.0 / (PROGRESS_BUCKETS as f32 * 4.0);
    let mut clears = 0;
    let mut p = 0.0;
    for _ in 0..16 {
        if np.sync(Some("s1"), p) {
            clears += 1;
        }
        p += step;
    }

    assert!(
        clears <= 5,
        "got {clears} clears in 16 frames: cache thrashing"
    );
}
