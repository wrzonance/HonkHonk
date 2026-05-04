use honkhonk::audio::AudioError;
use std::io;

#[test]
fn audio_error_file_open_displays_path_context() {
    let err = AudioError::FileOpen(io::Error::new(io::ErrorKind::NotFound, "gone"));
    let msg = err.to_string();
    assert!(msg.contains("open"), "expected 'open' in: {msg}");
}

#[test]
fn audio_error_unsupported_format_displays() {
    let err = AudioError::UnsupportedFormat;
    let msg = err.to_string();
    assert!(msg.contains("unsupported") || msg.contains("format"), "got: {msg}");
}

#[test]
fn audio_error_no_track_displays() {
    let err = AudioError::NoTrack;
    let msg = err.to_string();
    assert!(msg.contains("track"), "got: {msg}");
}

#[test]
fn audio_error_missing_codec_params_displays() {
    let err = AudioError::MissingCodecParams;
    let msg = err.to_string();
    assert!(!msg.is_empty());
}

use honkhonk::audio::decode;
use std::path::Path;

#[test]
fn decode_mono_wav_returns_correct_metadata() {
    let path = Path::new("tests/fixtures/sine_mono.wav");
    let audio = decode(path).expect("decode mono WAV failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty(), "samples should not be empty");

    let expected_sample_count = 48000; // 1 second * 48kHz * 1 channel
    let tolerance = expected_sample_count / 10; // 10% tolerance for codec framing
    let diff = (audio.samples.len() as i64 - expected_sample_count as i64).unsigned_abs() as usize;
    assert!(
        diff < tolerance,
        "expected ~{expected_sample_count} samples, got {}",
        audio.samples.len()
    );
}

#[test]
fn decode_mono_wav_returns_valid_duration() {
    let path = Path::new("tests/fixtures/sine_mono.wav");
    let audio = decode(path).expect("decode mono WAV failed");

    let duration_secs = audio.duration.as_secs_f64();
    assert!(
        (duration_secs - 1.0).abs() < 0.1,
        "expected ~1.0s duration, got {duration_secs}s"
    );
}

#[test]
fn decode_mono_wav_samples_in_valid_range() {
    let path = Path::new("tests/fixtures/sine_mono.wav");
    let audio = decode(path).expect("decode mono WAV failed");

    for (i, &sample) in audio.samples.iter().enumerate() {
        assert!(
            (-1.0..=1.0).contains(&sample),
            "sample {i} out of range: {sample}"
        );
    }
}
