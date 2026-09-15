//! Shared fixtures for the playback test modules (`tests`, `cache_tests`).

use super::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(super) fn app_with_audio() -> HonkHonk {
    let mut app = HonkHonk::new_for_test();
    let (handle, _evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);
    app
}

pub(super) fn sound(id: &str) -> SoundEntry {
    SoundEntry {
        id: id.into(),
        name: id.to_uppercase(),
        path: format!("/tmp/{id}.wav").into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: Some(100),
        modified_ms: None,
        category: "Test".into(),
    }
}

pub(super) fn pcm(samples: usize) -> crate::audio::CachedPcm {
    crate::audio::CachedPcm {
        analysis: Default::default(),
        samples: Arc::new(vec![0.0_f32; samples]),
        sample_rate: 48_000,
        channels: 1,
        duration: Duration::from_millis(100),
    }
}

pub(super) fn cache_pcm(app: &mut HonkHonk, id: &str) {
    app.audio_store.insert_pcm(id.to_owned(), Arc::new(pcm(8)));
}

pub(super) fn dispatch(app: &HonkHonk, generation: u64) -> PlaybackDispatch {
    PlaybackDispatch {
        generation,
        voice_id: generation,
        gain: 1.0,
        effects: app.effects_ui.to_effect_settings(),
        mode: PlayMode::Concurrent,
    }
}

pub(super) fn play_count(app: &HonkHonk) -> usize {
    app.audio
        .as_ref()
        .expect("audio handle")
        .sent_commands()
        .iter()
        .filter(|cmd| matches!(cmd, AudioCommand::Play { .. }))
        .count()
}

pub(super) fn stopped_voices(app: &HonkHonk) -> Vec<u64> {
    app.audio
        .as_ref()
        .expect("audio handle")
        .sent_commands()
        .iter()
        .filter_map(|cmd| match cmd {
            AudioCommand::StopVoice(voice) => Some(*voice),
            _ => None,
        })
        .collect()
}

pub(super) fn start_now_playing(app: &mut HonkHonk, id: &str) {
    app.playing = Some(id.to_owned());
    app.now_playing.start(now_playing::PlaybackStart {
        id,
        duration: Duration::from_secs(5),
        samples: &[0.25_f32; 64],
        channels: 1,
        now: Instant::now(),
    });
}
