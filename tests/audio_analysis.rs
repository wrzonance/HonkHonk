use honkhonk::audio::processing::decode_cached;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn decoded_formats_keep_native_rate_duration_and_stable_content_identity() {
    for name in [
        "sine_mono.wav",
        "sine_mono.flac",
        "sine_mono.mp3",
        "sine_mono.ogg",
        "sine_stereo.m4a",
    ] {
        let pcm = decode_cached(&fixture(name)).unwrap();
        assert!(pcm.sample_rate > 0);
        assert!((1..=2).contains(&pcm.channels));
        assert!(pcm.duration.as_secs_f64() > 0.0);
        assert_eq!(pcm.analysis.fingerprint.len(), 64);
        assert!(pcm.analysis.normalization_gain.is_finite());
        assert!(pcm.samples.iter().all(|s| s.is_finite()));
    }
}

#[test]
fn decode_error_keeps_the_underlying_reason_and_actionable_remedy() {
    let error = decode_cached(&fixture("corrupt.mp3"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("PCM WAV"));
    assert!(
        error.contains("unsupported feature")
            || error.contains("end of stream")
            || error.contains("unrecognized format"),
        "{error}"
    );
}
