use honkhonk::audio::playback::PlaybackState;
use honkhonk::audio::processing::{OutputMode, SoundProcessing};
use std::sync::Arc;

fn state(samples: Vec<f32>, channels: u16, output: OutputMode, pan: f32) -> PlaybackState {
    let mut state = PlaybackState::new();
    state.start("source".into(), Arc::new(samples), 48_000, channels, 1.0);
    state.set_channel_processing(SoundProcessing {
        output,
        pan,
        ..Default::default()
    });
    state
}

#[test]
fn downmix_consumes_source_frames_and_keeps_source_progress() {
    let mut state = state(vec![0.6, 0.3, 0.0, 0.3, 0.0, 0.0], 3, OutputMode::Mono, 0.0);
    let mut output = [0.0];
    assert_eq!(state.fill_buffer(&mut output), 1);
    assert!((output[0] - 0.3).abs() < 0.0001);
    assert_eq!(state.progress(), 0.5);
    assert!(state.is_active());
    assert_eq!(state.fill_buffer(&mut output), 1);
    assert!((output[0] - 0.1).abs() < 0.0001);
    assert_eq!(state.progress(), 1.0);
    assert!(!state.is_active());
}

#[test]
fn mono_pan_expands_without_consuming_partial_output_frames() {
    let mut state = state(vec![0.3, 0.6], 1, OutputMode::Preserve, 0.5);
    assert_eq!(state.channels(), 2);
    assert_eq!(state.fill_buffer(&mut [0.0]), 0);
    assert!(state.is_active());
    assert_eq!(state.progress(), 0.0);
    let mut output = [9.0; 3];
    assert_eq!(state.fill_buffer(&mut output), 2);
    assert_eq!(output, [0.3, 0.3, 9.0]);
    assert_eq!(state.progress(), 0.5);
}

fn render(source_channels: u16, settings: SoundProcessing, chunk: usize) -> Vec<f32> {
    use honkhonk::audio::processing::{DynamicsSettings, VoiceProcessing};
    use honkhonk::audio::voices::{MixScratch, MixTarget, VoicePool, VoiceSpec};
    let mut pool = VoicePool::new();
    pool.set_dynamics(DynamicsSettings {
        enabled: false,
        ..Default::default()
    });
    pool.push(VoiceSpec {
        id: 1,
        sound_id: "source".into(),
        generation: 1,
        samples: Arc::new(
            (0..128 * source_channels as usize)
                .map(|n| (n % 5) as f32 * 0.1)
                .collect(),
        ),
        sample_rate: 48_000,
        channels: source_channels,
        gain: 1.0,
        master_volume: 1.0,
        effects: Default::default(),
        monitor_enabled: true,
        processing: VoiceProcessing {
            sound: settings,
            ..Default::default()
        },
    });
    let mut sink = vec![0.0; 256];
    let mut monitor = vec![0.0; 256];
    for (target, output) in [
        (MixTarget::Sink, &mut sink),
        (MixTarget::Monitor, &mut monitor),
    ] {
        for part in output.chunks_mut(chunk) {
            pool.mix(target, part, &mut MixScratch::new(1), 48_000);
        }
    }
    assert_eq!(sink, monitor);
    sink
}

#[test]
fn conversion_is_chunk_independent_on_sink_and_monitor() {
    for channels in [1, 2, 3] {
        for output in [OutputMode::Preserve, OutputMode::Mono, OutputMode::Stereo] {
            let settings = SoundProcessing {
                output,
                pan: 0.5,
                ..Default::default()
            };
            // Use complete output frames, including preserved 3-channel frames.
            assert_eq!(
                render(channels, settings, 12),
                render(channels, settings, 60)
            );
        }
    }
    let mono = render(
        1,
        SoundProcessing {
            pan: 0.5,
            ..Default::default()
        },
        2,
    );
    assert_eq!(&mono[..6], &[0.0, 0.0, 0.05, 0.1, 0.1, 0.2]);
}

#[test]
fn forced_stereo_folds_multichannel_mean_but_preserve_keeps_every_channel() {
    for channels in [3, 6] {
        let source: Vec<f32> = (0..channels * 2).map(|n| n as f32 * 0.1).collect();
        let mut stereo = state(source.clone(), channels, OutputMode::Stereo, 0.0);
        assert_eq!(stereo.channels(), 2);
        let mut frame = [0.0; 2];
        assert_eq!(stereo.fill_buffer(&mut frame), 2);
        let mean = source[..channels as usize].iter().sum::<f32>() / channels as f32;
        assert!(frame.iter().all(|sample| (*sample - mean).abs() < 0.0001));
        assert_eq!(stereo.progress(), 0.5);
        let mut preserved = state(source.clone(), channels, OutputMode::Preserve, 0.0);
        let mut output = vec![0.0; source.len()];
        assert_eq!(preserved.channels(), channels);
        assert_eq!(preserved.fill_buffer(&mut output), source.len());
        assert_eq!(output, source);
    }
}
