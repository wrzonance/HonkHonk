use super::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn app_with_audio() -> HonkHonk {
    let mut app = HonkHonk::new_for_test();
    let (handle, _evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);
    app
}

fn sound(id: &str) -> SoundEntry {
    SoundEntry {
        id: id.into(),
        name: id.to_uppercase(),
        path: format!("/tmp/{id}.wav").into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: Some(100),
        category: "Test".into(),
    }
}

fn pcm(samples: usize) -> crate::audio::CachedPcm {
    crate::audio::CachedPcm {
        samples: Arc::new(vec![0.0_f32; samples]),
        sample_rate: 48_000,
        channels: 1,
        duration: Duration::from_millis(100),
    }
}

fn cache_pcm(app: &mut HonkHonk, id: &str) {
    app.audio_store.insert_pcm(id.to_owned(), Arc::new(pcm(8)));
}

fn dispatch(app: &HonkHonk, generation: u64) -> PlaybackDispatch {
    PlaybackDispatch {
        generation,
        voice_id: generation,
        gain: 1.0,
        effects: app.effects_ui.to_effect_settings(),
        mode: PlayMode::Concurrent,
    }
}

fn play_count(app: &HonkHonk) -> usize {
    app.audio
        .as_ref()
        .expect("audio handle")
        .sent_commands()
        .iter()
        .filter(|cmd| matches!(cmd, AudioCommand::Play { .. }))
        .count()
}

fn stopped_voices(app: &HonkHonk) -> Vec<u64> {
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

fn start_now_playing(app: &mut HonkHonk, id: &str) {
    app.playing = Some(id.to_owned());
    app.now_playing.start(now_playing::PlaybackStart {
        id,
        duration: Duration::from_secs(5),
        samples: &[0.25_f32; 64],
        channels: 1,
        now: Instant::now(),
    });
}

#[test]
fn cold_press_takes_highlight_then_clears_on_decode_failure() {
    let mut app = app_with_audio();
    let current = sound("a");
    let corrupt = sound("b");
    app.sounds = vec![current, corrupt.clone()];
    start_now_playing(&mut app, "a");

    let _ = app.request_play(&corrupt, false);
    let generation = app.play_generation;

    // Snappy-UI doctrine (#111): a cold press claims the highlight instantly,
    // before its decode lands, so the click never feels laggy. The previous
    // sound losing the highlight here is the accepted cold-miss tradeoff (#152).
    assert_eq!(
        app.playing(),
        Some("b"),
        "a cold press claims the highlight immediately (snappy UI)"
    );
    assert!(
        !app.now_playing.has_playhead(),
        "the playhead stays idle until the decode confirms the duration"
    );

    let _ = app.handle_decoded(
        "b".into(),
        Err("undecodable test fixture".into()),
        dispatch(&app, generation),
    );

    // A failed cold decode releases the optimistic highlight rather than
    // leaving it stuck on a sound that never produced audio.
    assert_eq!(
        app.playing(),
        None,
        "a failed cold decode clears the optimistic highlight"
    );
    assert!(!app.now_playing.has_playhead());
}

#[test]
fn decoded_sound_removed_from_library_is_dropped() {
    let mut app = app_with_audio();
    let gone = sound("gone");
    app.sounds = vec![gone.clone()];

    let _ = app.request_play(&gone, false);
    let generation = app.play_generation;
    app.sounds.clear();

    let _ = app.handle_decoded("gone".into(), Ok(pcm(16)), dispatch(&app, generation));

    assert!(
        app.audio_store.get_pcm("gone").is_none(),
        "removed sounds must not be cached when their decode lands"
    );
    assert_eq!(app.playing(), None);
    assert!(!app.now_playing.has_playhead());
    assert!(app.pending_play_ids.is_empty());
}

#[test]
fn library_reconcile_clears_playing_sound_removed_from_library() {
    let mut app = app_with_audio();
    app.sounds = vec![sound("gone")];
    start_now_playing(&mut app, "gone");

    app.sounds.clear();
    app.reconcile_playback_with_library();

    assert_eq!(app.playing(), None);
    assert!(!app.now_playing.has_playhead());
}

#[test]
fn repeated_same_id_cold_press_reuses_one_decode_for_latest_dispatch() {
    let mut app = app_with_audio();
    let snd = sound("same");
    app.sounds = vec![snd.clone()];

    let _ = app.request_play(&snd, false);
    let first_generation = app.play_generation;
    let _ = app.request_play(&snd, false);

    assert_eq!(
        app.pending_play_ids.len(),
        1,
        "same-id cold repeats should coalesce onto the existing decode"
    );

    let _ = app.handle_decoded("same".into(), Ok(pcm(16)), dispatch(&app, first_generation));

    assert_eq!(app.playing(), Some("same"));
    assert!(app.now_playing.has_playhead());
    assert!(app.pending_play_ids.is_empty());
}

#[test]
fn warm_play_sound_sets_playing_immediately() {
    let mut app = app_with_audio();
    let snd = sound("wav1");
    app.sounds = vec![snd];
    cache_pcm(&mut app, "wav1");

    // Warm cache hits claim the highlight synchronously via start_playback;
    // cold misses claim it optimistically in request_play (both #111).
    let _ = app.update(Message::PlaySound("wav1".into()));

    assert_eq!(app.playing(), Some("wav1"));
}

#[test]
fn warm_shortcut_activation_sets_playing_immediately() {
    let mut app = app_with_audio();
    let snd = sound("wav1");
    app.slots.set(0, snd.path.clone());
    app.sounds = vec![snd];
    cache_pcm(&mut app, "wav1");

    let _ = app.update(Message::ShortcutActivated(0));

    assert_eq!(app.playing(), Some("wav1"));
}

#[test]
fn pcm_eviction_removes_matching_waveform_envelope() {
    let mut app = app_with_audio();
    app.audio_store = crate::audio::AudioStore::new(32);
    let a = sound("a");
    let b = sound("b");
    app.sounds = vec![a.clone(), b.clone()];

    let _ = app.request_play(&a, false);
    let a_generation = app.play_generation;
    let _ = app.handle_decoded("a".into(), Ok(pcm(4)), dispatch(&app, a_generation));
    assert!(app.now_playing.envelope("a").is_some());

    let _ = app.request_play(&b, false);
    let b_generation = app.play_generation;
    let _ = app.handle_decoded("b".into(), Ok(pcm(8)), dispatch(&app, b_generation));

    assert!(app.audio_store.get_pcm("a").is_none());
    assert!(
        app.now_playing.envelope("a").is_none(),
        "waveform envelope must be evicted with its PCM victim"
    );
}

#[test]
fn current_decoded_starts_playhead_and_caches_pcm() {
    let mut app = app_with_audio();
    app.play_generation = 2;
    app.playing = Some("snd".into());
    app.sounds = vec![sound("snd")];
    app.pending_play_ids.insert(2);

    let _ = app.update(Message::Decoded {
        generation: 2,
        voice_id: 2,
        id: "snd".into(),
        result: Ok(pcm(64)),
        gain: 1.0,
        effects: crate::audio::effects::EffectSettings::default(),
        mode: PlayMode::Concurrent,
    });

    assert!(
        app.now_playing.has_playhead(),
        "current decode must start the playhead"
    );
    assert!(
        app.audio_store.get_pcm("snd").is_some(),
        "decode result must be cached for instant re-fire"
    );
}

#[test]
fn stopall_mid_decode_does_not_resurrect_playback() {
    let mut app = app_with_audio();
    let snd = sound("wav1");
    app.sounds = vec![snd.clone()];

    let _ = app.request_play(&snd, false);
    let in_flight_gen = app.play_generation;
    assert_eq!(
        app.playing(),
        Some("wav1"),
        "a cold press claims the highlight optimistically (snappy UI)"
    );
    assert_eq!(app.pending_play_ids.len(), 1);

    let _ = app.update(Message::StopAll);
    assert_eq!(app.playing(), None);

    let _ = app.update(Message::Decoded {
        generation: in_flight_gen,
        voice_id: in_flight_gen,
        id: "wav1".into(),
        result: Ok(pcm(8)),
        gain: 1.0,
        effects: crate::audio::effects::EffectSettings::default(),
        mode: PlayMode::Concurrent,
    });

    assert_eq!(app.playing(), None, "StopAll must win: no resurrection");
    assert!(
        !app.now_playing.has_playhead(),
        "no playhead after a stopped, stale decode"
    );
}

#[test]
fn late_concurrent_decode_keeps_newest_in_now_playing() {
    let mut app = app_with_audio();
    app.sounds = vec![sound("a"), sound("b")];
    // Default overlap mode is Concurrent, so superseded presses stay pending.

    // Two cold presses: each bumps the generation (A=1, B=2) and, in
    // concurrent mode, both stay pending awaiting their decode.
    let a = app.sounds[0].clone();
    let b = app.sounds[1].clone();
    let _ = app.request_play(&a, false);
    let _ = app.request_play(&b, false);

    // Decodes land out of order: the newest press (B) finishes first, then
    // the older press (A).
    let _ = app.handle_decoded("b".into(), Ok(pcm(16)), dispatch(&app, 2));
    let _ = app.handle_decoded("a".into(), Ok(pcm(16)), dispatch(&app, 1));

    // The older decode must still start its audio (cached + accepted, not
    // dropped as stale)...
    assert!(
        app.audio_store.get_pcm("a").is_some(),
        "older concurrent decode must still start playing"
    );
    // ...but the newest press keeps ownership of the highlight/playhead.
    assert_eq!(
        app.playing.as_deref(),
        Some("b"),
        "a late older decode must not retake `playing` from the newer press"
    );
    assert!(
        app.now_playing
            .current_key()
            .is_some_and(|key| key.matches(Some("b"), 0.0)),
        "a late older decode must not retake the now-playing highlight"
    );
}

#[test]
fn cold_press_emits_exactly_one_play_when_its_decode_lands() {
    let mut app = app_with_audio();
    let snd = sound("a");
    app.sounds = vec![snd.clone()];

    let _ = app.request_play(&snd, false);
    let generation = app.play_generation;
    assert_eq!(
        play_count(&app),
        0,
        "a cold press queues a decode and must not fire before it lands"
    );

    let _ = app.handle_decoded("a".into(), Ok(pcm(16)), dispatch(&app, generation));

    assert_eq!(
        play_count(&app),
        1,
        "a landed cold decode fires exactly one engine Play"
    );
}

#[test]
fn stale_decode_after_stopall_emits_no_play() {
    let mut app = app_with_audio();
    let snd = sound("a");
    app.sounds = vec![snd.clone()];

    let _ = app.request_play(&snd, false);
    let generation = app.play_generation;
    let _ = app.update(Message::StopAll);
    let plays_before = play_count(&app);

    let _ = app.handle_decoded("a".into(), Ok(pcm(16)), dispatch(&app, generation));

    assert_eq!(
        play_count(&app),
        plays_before,
        "a decode landing after StopAll is stale and must fire no Play"
    );
}

#[test]
fn reconcile_stops_engine_voice_for_removed_playing_sound() {
    let mut app = app_with_audio();
    let snd = sound("gone");
    app.sounds = vec![snd.clone()];
    // Warm play so a real engine voice exists, then drop the sound from the
    // library and reconcile: the orphaned voice must be stopped, not left to
    // honk to completion with no UI.
    cache_pcm(&mut app, "gone");
    let _ = app.request_play(&snd, false);
    let voice = app.play_generation;
    assert_eq!(app.playing(), Some("gone"));

    app.sounds.clear();
    app.reconcile_playback_with_library();

    assert_eq!(app.playing(), None);
    assert!(
        stopped_voices(&app).contains(&voice),
        "reconcile must stop the removed playing sound's engine voice"
    );
}
