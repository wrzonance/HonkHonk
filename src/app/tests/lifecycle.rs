use super::*;

#[test]
fn toggle_visibility_flips_state() {
    let mut app = HonkHonk::new_for_test();
    assert!(app.is_visible());
    let _ = app.update(Message::ToggleVisibility);
    assert!(!app.is_visible());
    let _ = app.update(Message::ToggleVisibility);
    assert!(app.is_visible());
}

#[test]
fn quit_sets_exit_flag() {
    let mut app = HonkHonk::new_for_test();
    assert!(!app.should_exit());
    let _ = app.update(Message::Quit);
    assert!(app.should_exit());
}

#[test]
fn quit_persists_config_only_when_it_loaded_cleanly() {
    // A config that failed to load falls back to in-memory defaults;
    // saving those on quit would destroy the user's repairable file, so
    // the quit save must be skipped for the whole session.
    let mut app = HonkHonk::new_for_test();
    let (handle, _evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);
    assert!(app.should_persist_config_on_quit());

    app.mark_config_load_failed();
    assert!(!app.should_persist_config_on_quit());
}

#[test]
fn quit_never_persists_config_without_audio_engine() {
    let app = HonkHonk::new_for_test();
    assert!(
        !app.should_persist_config_on_quit(),
        "test fixtures (audio: None) must never write the real config"
    );
}

#[test]
fn window_resize_records_dimensions_in_config() {
    // The live window size must flow into config so it can be persisted on
    // quit and restored on the next launch (the window_width/height fields
    // were previously dead).
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::WindowResized(1440.0, 912.0));
    assert_eq!(app.config.window_width, 1440);
    assert_eq!(app.config.window_height, 912);
}

#[test]
fn window_resize_ignores_degenerate_dimensions() {
    // Some compositors emit 0-size resize events (e.g. on minimize); those
    // must not clobber the last real size recorded for restore-on-launch.
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::WindowResized(1440.0, 912.0));
    let _ = app.update(Message::WindowResized(0.0, 912.0));
    let _ = app.update(Message::WindowResized(1440.0, 0.0));
    assert_eq!(app.config.window_width, 1440);
    assert_eq!(app.config.window_height, 912);
}

#[test]
fn test_fixtures_disable_persistence() {
    // Hermeticity guard: cargo test must never write the developer's real
    // ~/.config/honkhonk/{config,slots,meta}.json. Every disk write is gated
    // on `persist`, which must be false for test fixtures. If this flips,
    // settings/slot tests will clobber the user's real config.
    let app = HonkHonk::new_for_test();
    assert!(!app.persist, "new_for_test must disable disk persistence");
}

#[test]
fn settings_change_applies_in_memory_without_persisting() {
    // Disabling persistence must not disable the in-memory update — only the
    // disk write is skipped. (Confirms the persist gate didn't break the
    // handler's config mutation.)
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.config.overlap_mode, OverlapMode::Concurrent);
    let _ = app.update(Message::OverlapModeChanged(OverlapMode::Interrupt));
    assert_eq!(app.config.overlap_mode, OverlapMode::Interrupt);
}

#[test]
fn select_category_updates_active_category() {
    let mut app = HonkHonk::new_for_test();
    assert!(app.active_category().is_none());
    let _ = app.update(Message::SelectCategory(Some("Memes".into())));
    assert_eq!(app.active_category(), Some("Memes"));
    let _ = app.update(Message::SelectCategory(None));
    assert!(app.active_category().is_none());
}

#[test]
fn select_effect_preset_updates_ui_state() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SelectEffectPreset(PresetId::Robot));
    assert_eq!(app.effects_ui_preset(), PresetId::Robot);
}

#[test]
fn set_wet_dry_updates_ui_state() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SetWetDryMix(0.4));
    assert!((app.effects_ui_wet_dry() - 0.4).abs() < 1e-6);
}

#[test]
fn set_effect_bypass_updates_ui_state() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SetEffectBypassUi(true));
    assert!(app.effects_ui_chain_bypass());
}

#[test]
fn set_effect_param_switches_to_custom_preset() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SelectEffectPreset(PresetId::Robot));
    let _ = app.update(Message::SetEffectParamUi {
        slot: EffectSlot::Pitch,
        param: "semitones",
        value: -2.0,
    });
    assert_eq!(app.effects_ui_preset(), PresetId::Custom);
}

#[test]
fn stop_all_clears_playing() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "test-id".into(),
        generation: 0,
    }));
    let _ = app.update(Message::StopAll);
    assert!(app.playing().is_none());
}

#[test]
fn stop_all_clears_all_playback_state() {
    use std::time::{Duration, Instant};
    let mut app = HonkHonk::new_for_test();
    app.playing = Some("x".into());
    app.progress = 0.7;
    let samples = vec![0.25_f32; 64];
    app.now_playing.start(now_playing::PlaybackStart {
        id: "x",
        duration: Duration::from_secs(5),
        samples: &samples,
        channels: 1,
        now: Instant::now(),
    });
    let _ = app.update(Message::StopAll);
    assert!(app.playing.is_none());
    assert_eq!(app.progress, 0.0);
    assert_eq!(app.now_playing.display_progress(), 0.0);
    assert!(!app.now_playing.has_playhead());
}

#[test]
fn audio_event_playback_started_sets_playing() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "abc123".into(),
        generation: 0,
    }));
    assert_eq!(app.playing(), Some("abc123"));
}

#[test]
fn audio_event_playback_finished_clears_playing() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackStarted {
        sound_id: "abc123".into(),
        generation: 0,
    }));
    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        voice_id: 0,
        sound_id: "abc123".into(),
        generation: 0,
    }));
    assert!(app.playing().is_none());
}

/// Smoke test for the overlay layering in `view_main`: the element tree
/// must build in every overlay state. The structural invariant itself
/// (stable Stack root preserving scrollable offsets, #112) lives in iced's
/// private widget state and is covered by the manual test plan instead.
#[test]
fn view_builds_in_all_overlay_states() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![SoundEntry {
        id: "aaa".into(),
        name: "Goose Honk".into(),
        path: "/a.mp3".into(),
        format: crate::state::AudioFormat::Mp3,
        duration_ms: Some(1000),
        modified_ms: None,
        category: "Honk".into(),
    }];

    let _ = app.view(); // no overlay
    let _ = app.update(Message::OpenContextMenu("aaa".into()));
    let _ = app.view(); // context menu overlay
    let _ = app.update(Message::OpenSoundEditor("aaa".into()));
    let _ = app.view(); // editor overlay
}
