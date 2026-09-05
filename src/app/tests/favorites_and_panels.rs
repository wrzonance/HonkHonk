use super::*;

#[test]
fn filtered_sounds_favorites_tab_shows_only_starred_sounds() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        SoundEntry {
            id: "fav".into(),
            name: "Favourite".into(),
            path: "/fav.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: None,
            modified_ms: None,
            category: "General".into(),
        },
        SoundEntry {
            id: "nonfav".into(),
            name: "Regular".into(),
            path: "/nonfav.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: None,
            modified_ms: None,
            category: "General".into(),
        },
    ];
    let _ = app.update(Message::ToggleFavorite("fav".into()));
    let _ = app.update(Message::SelectCategory(Some(FAVORITES_TAB.to_owned())));
    let filtered = app.filtered_sounds();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "fav");
}

#[test]
fn filtered_sounds_all_tab_includes_all_when_no_favorites_selected() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        SoundEntry {
            id: "a".into(),
            name: "A".into(),
            path: "/a.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: None,
            modified_ms: None,
            category: "X".into(),
        },
        SoundEntry {
            id: "b".into(),
            name: "B".into(),
            path: "/b.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: None,
            modified_ms: None,
            category: "Y".into(),
        },
    ];
    app.refresh_filtered_sounds();
    let _ = app.update(Message::ToggleFavorite("a".into()));
    // Select All tab
    let _ = app.update(Message::SelectCategory(None));
    assert_eq!(app.filtered_sounds().len(), 2);
}

#[test]
fn unstarring_last_favorite_while_on_favorites_tab_resets_to_all() {
    // Regression: removing the last favorite while on the Favorites tab
    // would leave active_category pointing to the now-invisible chip,
    // showing an empty list with no way to navigate back.
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![SoundEntry {
        id: "only".into(),
        name: "Only Fav".into(),
        path: "/only.mp3".into(),
        format: crate::state::AudioFormat::Mp3,
        duration_ms: None,
        modified_ms: None,
        category: "General".into(),
    }];
    let _ = app.update(Message::ToggleFavorite("only".into()));
    let _ = app.update(Message::SelectCategory(Some(FAVORITES_TAB.to_owned())));
    assert_eq!(app.active_category(), Some(FAVORITES_TAB));
    // Unstar the only favorite — must fall back to "All"
    let _ = app.update(Message::ToggleFavorite("only".into()));
    assert!(
        app.active_category().is_none(),
        "active_category must reset to All when last favorite is removed"
    );
}

#[test]
fn search_matches_display_name_override() {
    // Regression: sounds renamed via the editor were invisible to search
    // because filtered_sounds() only matched SoundEntry.name, not the
    // display_name stored in SoundMetaStore.
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![SoundEntry {
        id: "id1".into(),
        name: "goose_honk_v2.wav".into(),
        path: "/id1.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "Animals".into(),
    }];
    // Rename the sound via the editor workflow
    app.sound_meta
        .set_display_name("id1", Some("Angry Goose".to_owned()));
    // Searching for the display name override must find the sound
    let _ = app.update(Message::SearchChanged("angry".into()));
    assert_eq!(
        app.filtered_sounds().len(),
        1,
        "renamed sound must be discoverable by its display name"
    );
    // Searching for the original filename still works too
    let _ = app.update(Message::SearchChanged("goose_honk".into()));
    assert_eq!(app.filtered_sounds().len(), 1);
}

#[test]
fn playing_a_sound_caches_its_waveform_envelope() {
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

    // Drive playback through the async path: `request_play` bumps the
    // generation and (on a cold cache) returns a decode `Task`; we feed the
    // matching `Decoded` directly, since the engine decode is off-thread now.
    let sound = app.sounds[0].clone();
    let _ = app.request_play(&sound, false);
    let decoded = crate::audio::decode(&sound.path).expect("decode test wav");
    let _ = app.update(Message::Decoded {
        generation: app.play_generation,
        voice_id: app.play_generation,
        id: "wav1".into(),
        result: Ok(crate::audio::CachedPcm {
            samples: std::sync::Arc::new(decoded.samples),
            sample_rate: decoded.sample_rate,
            channels: decoded.channels,
            duration: decoded.duration,
        }),
        gain: 1.0,
        effects: crate::audio::effects::EffectSettings::default(),
        mode: PlayMode::Concurrent,
    });
    let env = app
        .now_playing
        .envelope("wav1")
        .expect("envelope should be cached after play");
    assert_eq!(
        env.bars(crate::ui::waveform::WAVEFORM_BARS).len(),
        crate::ui::waveform::WAVEFORM_BARS
    );
}

#[test]
fn toggle_effects_panel_opens_then_closes() {
    let mut app = HonkHonk::new_for_test();
    assert!(!app.effects_panel.is_open());
    let _ = app.update(Message::ToggleEffectsPanel);
    assert!(app.effects_panel.is_open());
    assert!(app.effects_panel.is_animating());
    let _ = app.update(Message::ToggleEffectsPanel);
    assert!(!app.effects_panel.is_open());
}

#[test]
fn close_effects_panel_closes_open_panel() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ToggleEffectsPanel);
    assert!(app.effects_panel.is_open());
    let _ = app.update(Message::CloseEffectsPanel);
    assert!(!app.effects_panel.is_open());
}

#[test]
fn escape_closes_open_effects_panel() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ToggleEffectsPanel);
    assert!(app.effects_panel.is_open());
    let _ = app.update(Message::EscapePressed);
    assert!(!app.effects_panel.is_open());
}

#[test]
fn frame_settles_panel_progress_after_slide() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ToggleEffectsPanel); // opening
    let later = Instant::now() + crate::ui::side_panel::SLIDE_DURATION;
    let _ = app.update(Message::Frame(later));
    assert_eq!(app.panel_progress, 1.0);
    assert!(!app.effects_panel.is_animating());
}

#[test]
fn escape_during_close_does_not_clear_search() {
    // Regression: while the drawer is mid-close, is_open() is false but the
    // panel is still on screen. Escape must be absorbed by the drawer, not
    // fall through and wipe the search query.
    let mut app = HonkHonk::new_for_test();
    app.filter.replace("bark".to_owned());
    let _ = app.update(Message::ToggleEffectsPanel); // opening
    let open = Instant::now() + crate::ui::side_panel::SLIDE_DURATION;
    let _ = app.update(Message::Frame(open)); // settled open
    let _ = app.update(Message::ToggleEffectsPanel); // start closing
    assert!(!app.effects_panel.is_open());
    assert!(app.effects_panel.is_visible());
    let _ = app.update(Message::EscapePressed);
    assert_eq!(app.search_query(), "bark");
}

#[test]
fn stale_decoded_is_dropped() {
    // A Decoded carrying an older generation than the current play must not
    // start a playhead or change `playing` (a newer press superseded it, #149/#151).
    let mut app = HonkHonk::new_for_test();
    let (handle, _evt_tx) = crate::audio::test_handle();
    app.audio = Some(handle);
    app.play_generation = 5;
    app.playing = Some("newer".into());

    let pcm = std::sync::Arc::new(crate::audio::CachedPcm {
        samples: std::sync::Arc::new(vec![0.0_f32; 8]),
        sample_rate: 48_000,
        channels: 2,
        duration: std::time::Duration::from_secs(1),
    });
    let _ = app.update(Message::Decoded {
        generation: 4,
        voice_id: 4,
        id: "older".into(),
        result: Ok((*pcm).clone()),
        gain: 1.0,
        effects: crate::audio::effects::EffectSettings::default(),
        mode: PlayMode::Concurrent,
    });

    assert!(
        !app.now_playing.has_playhead(),
        "stale decode must not start a playhead"
    );
    assert_eq!(app.playing(), Some("newer"));
}
