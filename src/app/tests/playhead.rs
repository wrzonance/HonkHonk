use super::*;

#[test]
fn frame_message_advances_display_progress_while_playing() {
    use std::time::{Duration, Instant};
    let mut app = HonkHonk::new_for_test();
    let t0 = Instant::now();
    let samples = vec![0.25_f32; 64];
    app.now_playing.start(now_playing::PlaybackStart {
        id: "test",
        duration: Duration::from_secs(10),
        samples: &samples,
        channels: 1,
        now: t0,
    });
    let _ = app.update(Message::Frame(t0 + Duration::from_secs(5)));
    let progress = app.now_playing.display_progress();
    assert!((progress - 0.5).abs() < 1e-3, "got {}", progress);
}

#[test]
fn frame_message_is_noop_when_idle() {
    use std::time::{Duration, Instant};
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::Frame(Instant::now() + Duration::from_secs(1)));
    assert_eq!(app.now_playing.display_progress(), 0.0);
}

#[test]
fn progress_event_does_not_drive_display_progress() {
    // The smooth playhead is wall-clock driven (`Message::Frame`), NOT the
    // raw 10 Hz `Progress` anchor — re-anchoring to stale samples caused the
    // left/right jitter (#138). A Progress event updates the raw anchor but
    // must leave the smooth `display_progress` untouched.
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.65)));
    assert!((app.progress() - 0.65).abs() < f32::EPSILON);
    assert_eq!(app.now_playing.display_progress(), 0.0);
}

#[test]
fn playback_finished_clears_playhead_and_display_progress() {
    use std::time::{Duration, Instant};
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "test".into(),
        generation: 0,
    }));
    let samples = vec![0.25_f32; 64];
    app.now_playing.start(now_playing::PlaybackStart {
        id: "test",
        duration: Duration::from_secs(5),
        samples: &samples,
        channels: 1,
        now: Instant::now(),
    });
    let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.8)));
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        voice_id: 0,
        sound_id: "test".into(),
        generation: 0,
    }));
    assert!(!app.now_playing.has_playhead());
    assert_eq!(app.now_playing.display_progress(), 0.0);
}

#[allow(
    clippy::too_many_lines,
    reason = "regression test spells out the same-sound re-press event timeline from #149"
)]
#[test]
fn re_pressing_same_sound_keeps_playhead_alive() {
    // Re-pressing the SAME tile while it is still playing must re-trigger the
    // playhead. The engine replaces the active voice and emits a
    // `PlaybackFinished` for the *displaced* voice carrying the SAME
    // `sound_id`; the app must not mistake that stale event for a genuine end
    // and tear down the freshly-created playhead, freezing it at 0 (#149).
    let mut app = HonkHonk::new_for_test();
    let (handle, _evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);

    let dir = tempfile::tempdir().expect("tempdir");
    let wav_path = dir.path().join("honk.wav");
    write_test_wav(&wav_path);
    app.sounds = vec![SoundEntry {
        id: "wav1".into(),
        name: "Honk".into(),
        path: wav_path,
        format: crate::state::AudioFormat::Wav,
        duration_ms: Some(100),
        modified_ms: None,
        category: "Test".into(),
    }];

    // First press → decode dispatched; the playhead is created when its
    // `Decoded` (generation 1) lands. Decode is off-thread now, so we feed
    // the matching `Decoded` directly.
    let sound = app.sounds[0].clone();
    let _ = app.request_play(&sound, false);
    let decoded = crate::audio::decode(&sound.path).expect("decode test wav");
    let to_pcm = |d: &crate::audio::DecodedAudio| crate::audio::CachedPcm {
        analysis: Default::default(),
        samples: std::sync::Arc::new(d.samples.clone()),
        sample_rate: d.sample_rate,
        channels: d.channels,
        duration: d.duration,
    };
    let _ = app.update(Message::Decoded {
        generation: app.play_generation,
        voice_id: app.play_generation,
        id: "wav1".into(),
        result: Ok(to_pcm(&decoded)),
        gain: 1.0,
        effects: crate::audio::effects::EffectSettings::default(),
        mode: PlayMode::Concurrent,
    });
    // Second press re-triggers the same sound. The PCM is now cached, so
    // `request_play` fires synchronously and creates a fresh playhead
    // (generation 2) without another decode.
    let _ = app.request_play(&sound, false);
    assert!(
        app.now_playing.has_playhead(),
        "re-press should create a fresh playhead"
    );

    // The displaced first voice's Finished (older generation) arrives on the
    // next drain — it must NOT clear the re-triggered playhead.
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        voice_id: 1,
        sound_id: "wav1".into(),
        generation: 1,
    }));
    assert!(
        app.now_playing.has_playhead(),
        "stale displaced Finished must not clear the re-triggered playhead"
    );
    assert_eq!(app.playing(), Some("wav1"));

    // The genuine end of the current voice (matching generation) still clears.
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        voice_id: 2,
        sound_id: "wav1".into(),
        generation: 2,
    }));
    assert!(
        !app.now_playing.has_playhead(),
        "genuine end clears the playhead"
    );
    assert_eq!(app.playing(), None);
}

#[test]
fn progress_event_updates_progress() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.65)));
    assert!((app.progress() - 0.65).abs() < f32::EPSILON);
}

#[test]
fn playback_finished_resets_progress() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "test".into(),
        generation: 0,
    }));
    let _ = app.update(Message::AudioEvent(AudioEvent::Progress(0.8)));
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        voice_id: 0,
        sound_id: "test".into(),
        generation: 0,
    }));
    assert!((app.progress() - 0.0).abs() < f32::EPSILON);
}
