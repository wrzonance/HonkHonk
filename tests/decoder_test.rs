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
    use symphonia::core::errors::Error as SymphoniaError;
    let err = AudioError::UnsupportedFormat(SymphoniaError::Unsupported("test format"));
    let msg = err.to_string();
    assert!(
        msg.contains("unsupported") || msg.contains("format"),
        "got: {msg}"
    );
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
    assert!(
        msg.contains("codec") || msg.contains("parameters"),
        "expected codec/parameters in error message, got: {msg}"
    );
}

use honkhonk::audio::decode;
use std::path::Path;

fn write_stereo_pcm16_wav(path: &Path, frames: &[[i16; 2]], sample_rate: u32) {
    let data_len = u32::try_from(frames.len() * 4).expect("test WAV fits u32");
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for frame in frames {
        bytes.extend_from_slice(&frame[0].to_le_bytes());
        bytes.extend_from_slice(&frame[1].to_le_bytes());
    }
    std::fs::write(path, bytes).expect("write test WAV");
}

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

#[test]
fn decode_mp3_returns_correct_metadata() {
    let path = Path::new("tests/fixtures/sine_mono.mp3");
    let audio = decode(path).expect("decode MP3 failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty());

    let duration_secs = audio.duration.as_secs_f64();
    assert!(
        (duration_secs - 1.0).abs() < 0.15,
        "expected ~1.0s, got {duration_secs}s (MP3 framing tolerance)"
    );
}

#[test]
fn decode_ogg_returns_correct_metadata() {
    let path = Path::new("tests/fixtures/sine_mono.ogg");
    let audio = decode(path).expect("decode OGG failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty());

    let duration_secs = audio.duration.as_secs_f64();
    assert!(
        (duration_secs - 1.0).abs() < 0.1,
        "expected ~1.0s, got {duration_secs}s"
    );
}

#[test]
fn decode_flac_returns_correct_metadata() {
    let path = Path::new("tests/fixtures/sine_mono.flac");
    let audio = decode(path).expect("decode FLAC failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty());

    let duration_secs = audio.duration.as_secs_f64();
    assert!(
        (duration_secs - 1.0).abs() < 0.05,
        "expected ~1.0s, got {duration_secs}s (FLAC is lossless, tight tolerance)"
    );
}

#[test]
fn decode_stereo_wav_returns_two_channels() {
    let path = Path::new("tests/fixtures/sine_stereo.wav");
    let audio = decode(path).expect("decode stereo WAV failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 2);

    // Stereo: samples are interleaved [L, R, L, R, ...]
    // 1 second * 48kHz * 2 channels = 96000 samples
    let expected = 96000;
    let tolerance = expected / 10;
    let diff = (audio.samples.len() as i64 - expected as i64).unsigned_abs() as usize;
    assert!(
        diff < tolerance,
        "expected ~{expected} interleaved samples, got {}",
        audio.samples.len()
    );
}

#[test]
fn decode_stereo_wav_duration_matches_mono() {
    let mono = decode(Path::new("tests/fixtures/sine_mono.wav")).expect("decode mono failed");
    let stereo = decode(Path::new("tests/fixtures/sine_stereo.wav")).expect("decode stereo failed");

    let diff = (mono.duration.as_secs_f64() - stereo.duration.as_secs_f64()).abs();
    assert!(
        diff < 0.05,
        "mono ({:.3}s) and stereo ({:.3}s) durations should match",
        mono.duration.as_secs_f64(),
        stereo.duration.as_secs_f64()
    );
}

#[test]
fn decode_repairs_dead_stereo_lane_before_returning_pcm() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("dead-left.wav");
    let input_frames = vec![[0_i16, 8_192_i16]; 2_400];
    write_stereo_pcm16_wav(&path, &input_frames, 48_000);

    let audio = decode(&path).expect("decode dead-left WAV");

    assert_eq!(audio.sample_rate, 48_000);
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.samples.len(), input_frames.len() * 2);
    assert_eq!(audio.duration, std::time::Duration::from_millis(50));
    for frame in audio.samples.as_chunks::<2>().0 {
        assert_eq!(
            frame[0].to_bits(),
            frame[1].to_bits(),
            "repair must copy the right sample without changing its amplitude"
        );
    }
    let peak = audio.samples.iter().copied().fold(0.0_f32, f32::max);
    assert!(
        (peak - 0.25).abs() <= f32::EPSILON,
        "expected original 0.25 amplitude, got {peak}"
    );
}

#[test]
fn decode_nonexistent_file_returns_file_open_error() {
    let result = decode(Path::new("tests/fixtures/does_not_exist.wav"));
    assert!(result.is_err());

    let err = result.err().expect("already checked is_err");
    assert!(
        matches!(err, AudioError::FileOpen(_)),
        "expected FileOpen, got: {err}"
    );
}

#[test]
fn decode_corrupt_file_returns_error() {
    let result = decode(Path::new("tests/fixtures/corrupt.mp3"));
    assert!(
        result.is_err(),
        "corrupt file should not decode successfully"
    );
}

#[test]
fn decode_empty_file_returns_error() {
    let result = decode(Path::new("tests/fixtures/empty.wav"));
    assert!(result.is_err(), "empty file should not decode successfully");
}

#[test]
fn decode_wav_samples_contain_nonzero_signal() {
    let audio = decode(Path::new("tests/fixtures/sine_mono.wav")).expect("decode failed");

    let max_abs = audio
        .samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0_f32, f32::max);

    // 440Hz sine wave should have significant amplitude
    assert!(
        max_abs > 0.5,
        "expected peak amplitude > 0.5, got {max_abs} — signal may be silent or corrupt"
    );
}

#[test]
fn decode_all_formats_produce_similar_sample_counts() {
    let wav = decode(Path::new("tests/fixtures/sine_mono.wav")).expect("WAV");
    let mp3 = decode(Path::new("tests/fixtures/sine_mono.mp3")).expect("MP3");
    let ogg = decode(Path::new("tests/fixtures/sine_mono.ogg")).expect("OGG");
    let flac = decode(Path::new("tests/fixtures/sine_mono.flac")).expect("FLAC");

    // All should be ~48000 samples (1s * 48kHz * 1ch)
    // MP3 has encoder padding so wider tolerance
    let expected = 48000_usize;
    for (name, audio) in [("WAV", &wav), ("MP3", &mp3), ("OGG", &ogg), ("FLAC", &flac)] {
        let diff = (audio.samples.len() as i64 - expected as i64).unsigned_abs() as usize;
        assert!(
            diff < 7000,
            "{name}: expected ~{expected} samples, got {} (diff {diff})",
            audio.samples.len()
        );
    }
}

#[test]
fn decode_stereo_m4a_backfills_channels_from_first_frame() {
    // AAC-in-MP4 (.m4a) reports sample_rate but NOT channels in the container
    // header — the count lives in the AudioSpecificConfig and is only resolved
    // by the decoder on the first frame. decode() must backfill it rather than
    // failing with MissingCodecParams (#153, the "Directed by" music-video rip).
    // Stereo proves the real count is read, not defaulted to mono.
    let audio = decode(Path::new("tests/fixtures/sine_stereo.m4a")).expect("decode stereo m4a");
    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 2);
    assert!(!audio.samples.is_empty(), "samples should not be empty");
}

#[test]
fn decode_mono_m4a_succeeds() {
    let audio = decode(Path::new("tests/fixtures/sine_mono.m4a")).expect("decode mono m4a");
    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty(), "samples should not be empty");
}

#[test]
fn decode_probes_audio_content_when_extension_is_wrong() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mislabeled = dir.path().join("clip.mp3");
    std::fs::copy(Path::new("tests/fixtures/sine_mono.wav"), &mislabeled)
        .expect("copy WAV fixture");

    let audio = decode(&mislabeled).expect("content probe should decode mislabeled WAV");

    assert_eq!(audio.sample_rate, 48_000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty());
}

#[test]
fn decode_rejects_zero_rate_wav_without_panicking() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("zero-rate.wav");
    let mut wav = Vec::from(&b"RIFF"[..]);
    wav.extend_from_slice(&40_u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&0_u32.to_le_bytes());
    wav.extend_from_slice(&0_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&4_u32.to_le_bytes());
    wav.extend_from_slice(&0_i16.to_le_bytes());
    wav.extend_from_slice(&0_i16.to_le_bytes());
    std::fs::write(&path, wav).expect("write malformed WAV");

    let result = std::panic::catch_unwind(|| decode(&path));
    assert!(result.is_ok(), "malformed audio must not panic");
    assert!(result.expect("panic already checked").is_err());
}
