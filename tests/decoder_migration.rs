use honkhonk::audio::{AudioError, decode};

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
fn truncated_pcm_payload_is_an_error_without_unwinding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("truncated.wav");
    let mut bytes = std::fs::read("tests/fixtures/sine_mono.wav").unwrap();
    bytes.truncate(bytes.len() / 2);
    std::fs::write(&path, bytes).unwrap();

    let result = std::panic::catch_unwind(|| decode(&path)).expect("decode must contain panics");

    assert!(matches!(result, Err(AudioError::Decode(_))));
}
