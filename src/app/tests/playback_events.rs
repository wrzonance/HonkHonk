use super::*;

#[test]
fn stale_playback_started_does_not_overwrite_newer_playing() {
    let mut app = HonkHonk::new_for_test();
    // "newer" is highlighted (set optimistically at dispatch); a Started
    // for an older press still sitting in the queue must not steal the
    // highlight back (#111).
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "newer".into(),
        generation: 0,
    }));
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "older".into(),
        generation: 0,
    }));
    assert_eq!(app.playing(), Some("newer"));
}

#[test]
fn late_superseded_started_does_not_highlight_while_idle() {
    let mut app = HonkHonk::new_for_test();
    // A newer concurrent press advanced the generation, then that short
    // sound finished, so the UI is idle (playing == None) at generation 2.
    app.play_generation = 2;
    // An older superseded voice (generation 1) finally finishes decoding and
    // starts in the engine. Its Started must not re-highlight its tile —
    // otherwise the stale, old-generation Finished is ignored and the tile
    // stays stuck highlighted (#164).
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "older".into(),
        generation: 1,
    }));
    assert!(
        app.playing().is_none(),
        "a late superseded voice's Started must not claim the idle highlight"
    );
}

#[test]
fn current_generation_started_claims_idle_highlight() {
    let mut app = HonkHonk::new_for_test();
    app.play_generation = 2;
    // A Started from the current generation still confirms the highlight
    // when the optimistic dispatch state was already cleared.
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "current".into(),
        generation: 2,
    }));
    assert_eq!(app.playing(), Some("current"));
}

#[test]
fn stale_playback_finished_does_not_clear_newer_playing() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "newer".into(),
        generation: 0,
    }));
    // A Finished event for an already-replaced sound must not blank the
    // highlight of the sound that superseded it (issue #111).
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        voice_id: 0,
        sound_id: "older".into(),
        generation: 0,
    }));
    assert_eq!(app.playing(), Some("newer"));
}

#[test]
fn drain_audio_events_processes_entire_backlog() {
    let mut app = HonkHonk::new_for_test();
    let (handle, evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);

    // Simulate the queue state after spamming tiles: stale started/finished
    // pairs piled up behind progress events (issue #111). One drain call
    // must consume them all so the UI reflects the latest engine state.
    let events = [
        AudioEvent::PlaybackStarted {
            sound_id: "a".into(),
            generation: 0,
        },
        AudioEvent::PlaybackFinished {
            voice_id: 0,
            sound_id: "a".into(),
            generation: 0,
        },
        AudioEvent::PlaybackStarted {
            sound_id: "b".into(),
            generation: 0,
        },
        AudioEvent::Progress(0.25),
        AudioEvent::PlaybackFinished {
            voice_id: 0,
            sound_id: "b".into(),
            generation: 0,
        },
        AudioEvent::PlaybackStarted {
            sound_id: "c".into(),
            generation: 0,
        },
        AudioEvent::Progress(0.5),
    ];
    for e in events {
        evt_tx.send(e).expect("send event");
    }

    let _ = app.drain_audio_events();

    assert_eq!(app.playing(), Some("c"));
    assert!((app.progress() - 0.5).abs() < f32::EPSILON);
}

#[test]
fn play_sound_no_op_for_unknown_id() {
    let mut app = HonkHonk::new_for_test();
    // Should not panic, just return Task::none()
    let _ = app.update(Message::PlaySound("nonexistent-id".into()));
    assert!(app.playing().is_none());
}

#[test]
fn from_tray_event_maps_correctly() {
    assert_eq!(
        Message::from_tray_event(TrayEvent::ToggleVisibility),
        Message::ToggleVisibility
    );
    assert_eq!(Message::from_tray_event(TrayEvent::Quit), Message::Quit);
}

#[test]
fn search_changed_updates_query() {
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.search_query(), "");
    let _ = app.update(Message::SearchChanged("honk".into()));
    assert_eq!(app.search_query(), "honk");
}

#[test]
fn volume_changed_updates_config() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::VolumeChanged(0.42));
    assert!((app.config.volume - 0.42).abs() < f32::EPSILON);
}
