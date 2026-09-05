use honkhonk::audio::{AudioError, decode, decode_limited};
use std::path::Path;

const FIXTURES: &[&str] = &[
    "sine_mono.wav",
    "sine_stereo.wav",
    "sine_mono.flac",
    "sine_mono.ogg",
    "sine_mono.mp3",
    "sine_mono.m4a",
    "sine_stereo.m4a",
];

#[test]
fn exact_sample_limit_accepts_partial_final_packets() {
    for fixture in FIXTURES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let expected = decode(&path).expect("unlimited decode");
        let actual = decode_limited(&path, expected.samples.len()).unwrap_or_else(|error| {
            panic!("{fixture}: exact decoded sample count rejected: {error}")
        });
        assert_eq!(actual.samples, expected.samples, "{fixture}");
        assert_eq!(actual.duration, expected.duration, "{fixture}");
    }
}

#[test]
fn one_sample_over_the_limit_is_rejected() {
    for fixture in FIXTURES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let expected = decode(&path).expect("unlimited decode");
        assert!(
            matches!(
                decode_limited(&path, expected.samples.len() - 1),
                Err(AudioError::SampleLimit)
            ),
            "{fixture}"
        );
    }
}
