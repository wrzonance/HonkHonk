use honkhonk::audio::decode;
use std::path::Path;

#[test]
fn stereo_pcm_preserves_sample_order_across_packet_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stereo.wav");
    let mut bytes = std::fs::read("tests/fixtures/sine_stereo.wav").unwrap();
    let data = bytes.windows(4).position(|chunk| chunk == b"data").unwrap() + 8;
    let samples: Vec<i16> = (0..(bytes.len() - data) / 2)
        .map(|index| ((index * 97) % 32_767) as i16 - 16_384)
        .collect();
    for (dst, sample) in bytes[data..]
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .zip(&samples)
    {
        dst.copy_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(&path, bytes).unwrap();

    let audio = decode(&path).expect("decode multiframe stereo PCM");

    assert_eq!(audio.channels, 2);
    assert_eq!(audio.samples.len(), samples.len());
    for (actual, expected) in audio.samples.iter().zip(samples) {
        assert_eq!(*actual, f32::from(expected) / 32_768.0);
    }
}

#[test]
fn truncated_pcm_payload_preserves_playable_samples_without_unwinding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("truncated.wav");
    let mut bytes = std::fs::read("tests/fixtures/sine_mono.wav").unwrap();
    bytes.truncate(bytes.len() / 2);
    std::fs::write(&path, bytes).unwrap();

    let result = std::panic::catch_unwind(|| decode(&path)).expect("decode must contain panics");

    let partial = result.expect("truncated payload should retain decoded packets");
    let complete = decode(Path::new("tests/fixtures/sine_mono.wav")).unwrap();
    assert_eq!(partial.sample_rate, complete.sample_rate);
    assert_eq!(partial.channels, complete.channels);
    assert!(!partial.samples.is_empty());
    assert!(partial.samples.len() < complete.samples.len());
    assert_eq!(partial.samples, complete.samples[..partial.samples.len()]);
    assert!(!partial.duration.is_zero());
    assert!(partial.duration < complete.duration);
}

#[test]
fn alac_m4a_decodes_lossless_pcm() {
    // Fixture: ffmpeg -i sine_mono.wav -t 0.1 -map_metadata -1 -c:a alac sine_mono_alac.m4a
    let alac = decode(Path::new("tests/fixtures/sine_mono_alac.m4a"))
        .expect("ALAC in an accepted m4a container must decode");
    let wav = decode(Path::new("tests/fixtures/sine_mono.wav")).unwrap();

    assert_eq!(alac.sample_rate, 48_000);
    assert_eq!(alac.channels, 1);
    assert_eq!(alac.samples, wav.samples[..4_800]);
    assert_eq!(alac.duration, std::time::Duration::from_millis(100));
}
