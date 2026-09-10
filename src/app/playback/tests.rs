//! Decode-ownership lifecycle tests: cold-press optimism, pending-decode
//! consumption, StopAll staleness, and generation/highlight ownership.
//! Warm-path, eviction, and reconciliation tests live in `cache_tests`.

use super::test_support::*;
use super::*;

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
fn stale_same_id_decode_does_not_consume_new_pending_decode() {
    let mut app = app_with_audio();
    let snd = sound("a");
    app.sounds = vec![snd.clone()];

    // First cold press spawns decode task A, then StopAll cancels it.
    let _ = app.request_play(&snd, false);
    let stale_generation = app.play_generation;
    let _ = app.update(Message::StopAll);

    // A re-press spawns decode task B for the same sound id.
    let _ = app.request_play(&snd, false);
    let new_generation = app.play_generation;
    assert_eq!(app.playing(), Some("a"));

    // Task A's cancelled decode lands first, as an error. It must not consume
    // task B's pending entry or clear B's optimistic highlight.
    let _ = app.handle_decoded(
        "a".into(),
        Err("stale decode".into()),
        dispatch(&app, stale_generation),
    );
    assert_eq!(
        app.playing(),
        Some("a"),
        "a stale decode error must not clear the newer press's highlight"
    );

    // Task B's real result then lands and must still be accepted.
    let _ = app.handle_decoded("a".into(), Ok(pcm(16)), dispatch(&app, new_generation));
    assert_eq!(app.playing(), Some("a"));
    assert!(
        app.now_playing.has_playhead(),
        "the newer press's decode must still start playback"
    );
    assert_eq!(
        play_count(&app),
        1,
        "exactly the newer press's decode fires a Play"
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
        result: Ok(crate::audio::preparation::wrap_for_test(pcm(64))),
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
        result: Ok(crate::audio::preparation::wrap_for_test(pcm(8))),
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

    // Two cold presses: each bumps the generation and, in concurrent mode,
    // both stay pending awaiting their decode.
    let a = app.sounds[0].clone();
    let b = app.sounds[1].clone();
    let _ = app.request_play(&a, false);
    let a_generation = app.play_generation;
    let _ = app.request_play(&b, false);
    let b_generation = app.play_generation;

    // Decodes land out of order: the newest press (B) finishes first, then
    // the older press (A).
    let _ = app.handle_decoded("b".into(), Ok(pcm(16)), dispatch(&app, b_generation));
    let _ = app.handle_decoded("a".into(), Ok(pcm(16)), dispatch(&app, a_generation));

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
