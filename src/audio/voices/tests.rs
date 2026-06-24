use std::sync::Arc;

use super::*;
use crate::audio::effects::EffectSettings;

fn spec(id: u64, samples: Vec<f32>) -> VoiceSpec {
    VoiceSpec {
        id,
        sound_id: format!("sound-{id}"),
        generation: id + 100,
        samples: Arc::new(samples),
        sample_rate: 48_000,
        channels: 1,
        gain: 1.0,
        master_volume: 1.0,
        effects: EffectSettings::default(),
        monitor_enabled: true,
    }
}

#[test]
fn voice_pool_steals_oldest_when_full() {
    let mut pool = VoicePool::with_max_voices(2);
    assert!(pool.push(spec(1, vec![0.1])).is_empty());
    assert!(pool.push(spec(2, vec![0.2])).is_empty());

    let stolen = pool.push(spec(3, vec![0.3]));

    assert_eq!(stolen, vec![FinishedVoice::new(1, "sound-1", 101)]);
    assert_eq!(pool.voice_ids(), vec![2, 3]);
}

#[test]
fn stop_voice_finishes_only_target_once() {
    let mut pool = VoicePool::with_max_voices(4);
    pool.push(spec(1, vec![0.1]));
    pool.push(spec(2, vec![0.2]));

    let stopped = pool.stop_voice(1);
    let second_stop = pool.stop_voice(1);

    assert_eq!(stopped, vec![FinishedVoice::new(1, "sound-1", 101)]);
    assert!(second_stop.is_empty());
    assert_eq!(pool.voice_ids(), vec![2]);
}

#[test]
fn stop_all_finishes_every_remaining_voice() {
    let mut pool = VoicePool::with_max_voices(4);
    pool.push(spec(1, vec![0.1]));
    pool.push(spec(2, vec![0.2]));

    let stopped = pool.stop_all();

    assert_eq!(
        stopped,
        vec![
            FinishedVoice::new(1, "sound-1", 101),
            FinishedVoice::new(2, "sound-2", 102),
        ]
    );
    assert!(pool.is_empty());
}

#[test]
fn mix_sums_active_voices_and_clamps_output() {
    let mut pool = VoicePool::with_max_voices(4);
    pool.push(spec(1, vec![0.75, -0.75, 0.2, 0.2]));
    pool.push(spec(2, vec![0.75, -0.75, 0.2, -0.6]));
    let mut scratch = MixScratch::new(2);
    let mut out = [0.0_f32; 4];

    pool.mix(MixTarget::Sink, &mut out, &mut scratch, 48_000);

    let expected = [1.0, -1.0, 0.4, -0.4];
    for (actual, expected) in out.iter().zip(expected) {
        assert!(
            (*actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }
}

#[test]
fn natural_finish_waits_for_sink_and_monitor() {
    let mut pool = VoicePool::with_max_voices(4);
    pool.push(spec(1, vec![0.25, 0.25]));
    let mut scratch = MixScratch::new(2);
    let mut out = [0.0_f32; 2];

    pool.mix(MixTarget::Sink, &mut out, &mut scratch, 48_000);
    assert!(pool.drain_finished().is_empty());

    pool.mix(MixTarget::Monitor, &mut out, &mut scratch, 48_000);
    assert_eq!(
        pool.drain_finished(),
        vec![FinishedVoice::new(1, "sound-1", 101)]
    );
}

#[test]
fn pitched_voice_does_not_mutate_bypassed_concurrent_voice() {
    let mut pitched = spec(2, vec![0.25; 256]);
    pitched.effects.pitch.bypass = false;
    pitched.effects.pitch.semitones = 7.0;

    let mut pool = VoicePool::with_max_voices(4);
    pool.push(spec(1, vec![0.25; 256]));
    pool.push(pitched);
    let mut scratch = MixScratch::new(256);
    let mut out = [0.0_f32; 256];

    pool.mix(MixTarget::Sink, &mut out, &mut scratch, 48_000);

    assert!(
        out.iter().take(128).all(|s| (*s - 0.25).abs() < 1e-6),
        "pitch latency should silence only the pitched voice; bypassed voice stays dry"
    );
}
