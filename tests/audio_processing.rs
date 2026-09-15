use honkhonk::audio::processing::{Dynamics, DynamicsSettings, fingerprint, measure_gain};

fn tone(amplitude: f32) -> Vec<f32> {
    (0..48_000)
        .map(|n| (n as f32 * 1000.0 * std::f32::consts::TAU / 48_000.0).sin() * amplitude)
        .collect()
}

#[test]
fn normalization_equalizes_loudness_and_bounds_silence_and_short_clips() {
    let quiet = measure_gain(&tone(0.1), 48_000, 1).unwrap();
    let loud = measure_gain(&tone(0.5), 48_000, 1).unwrap();
    assert!((quiet / loud - 5.0).abs() < 0.02);
    assert_eq!(measure_gain(&[0.0; 48_000], 48_000, 1).unwrap(), 1.0);
    assert_eq!(measure_gain(&[0.1; 10], 48_000, 1).unwrap(), 1.0);
    assert!(measure_gain(&tone(0.0001), 48_000, 1).unwrap() <= 4.0);
    assert!(measure_gain(&[f32::NAN], 48_000, 1).is_err());
    assert!(measure_gain(&[0.0], 0, 1).is_err());
}

#[test]
fn content_identity_is_sha256_and_independent_of_path() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("renamed.wav");
    std::fs::write(&a, b"abc").unwrap();
    std::fs::copy(&a, &b).unwrap();
    assert_eq!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
    assert_eq!(
        fingerprint(&a).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    std::fs::write(&b, b"abd").unwrap();
    assert_ne!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
}

#[test]
fn linked_dynamics_limits_peaks_and_preserves_stereo_balance() {
    let mut dynamics = Dynamics::default();
    let mut samples = [2.0, 1.0].repeat(4800);
    dynamics.process(&mut samples, (48_000, 2), DynamicsSettings::default(), true);
    assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 0.98));
    for frame in samples.as_chunks::<2>().0 {
        assert!((frame[0] / frame[1] - 2.0).abs() < 0.0001);
    }
    let mut invalid = [f32::NAN, f32::INFINITY];
    dynamics.process(&mut invalid, (48_000, 2), DynamicsSettings::default(), true);
    assert!(invalid.iter().all(|s| s.is_finite()));
}

#[test]
fn disabled_compressor_preserves_audio_below_existing_full_scale_ceiling() {
    let mut samples = [0.8, -0.4];
    let settings = DynamicsSettings {
        enabled: false,
        ..Default::default()
    };
    Dynamics::default().process(&mut samples, (48_000, 2), settings, true);
    assert_eq!(samples, [0.8, -0.4]);
}

#[test]
fn voice_pool_applies_processing_and_final_limiter_to_both_outputs() {
    use honkhonk::audio::processing::{SoundProcessing, VoiceProcessing};
    use honkhonk::audio::voices::{MixScratch, MixTarget, VoicePool, VoiceSpec};
    let mut pool = VoicePool::new();
    for id in 0..2 {
        pool.push(VoiceSpec {
            id,
            sound_id: id.to_string(),
            generation: id,
            samples: std::sync::Arc::new(vec![0.8; 64]),
            sample_rate: 48_000,
            channels: 2,
            gain: 2.0,
            master_volume: 1.0,
            effects: Default::default(),
            monitor_enabled: true,
            processing: VoiceProcessing {
                normalization_gain: 2.0,
                sound: SoundProcessing {
                    pan: 1.0,
                    ..Default::default()
                },
            },
        });
    }
    for target in [MixTarget::Sink, MixTarget::Monitor] {
        let mut output = [0.0_f32; 32];
        pool.mix(target, &mut output, &mut MixScratch::new(8), 48_000);
        assert!(output.iter().all(|s| s.abs() <= 0.98));
        assert_eq!(output[0], 0.0);
        assert!(output[1] > 0.0);
    }
}

#[test]
fn sample_ceiling_holds_across_fractional_input_peaks() {
    for index in 1..10_000 {
        let peak = index as f32 / 719.0;
        let mut samples = [peak, -peak];
        Dynamics::default().process(&mut samples, (48_000, 2), DynamicsSettings::default(), true);
        assert!(
            samples.iter().all(|s| s.abs() <= 0.98),
            "input {peak}: {samples:?}"
        );
    }
}
