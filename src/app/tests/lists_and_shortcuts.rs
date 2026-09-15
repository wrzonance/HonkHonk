use super::*;

#[test]
fn search_filters_sounds() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        SoundEntry {
            id: "aaa".into(),
            name: "Goose Honk".into(),
            path: "/a.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            modified_ms: None,
            category: "Honk".into(),
        },
        SoundEntry {
            id: "bbb".into(),
            name: "Vine Boom".into(),
            path: "/b.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            modified_ms: None,
            category: "Memes".into(),
        },
    ];
    let _ = app.update(Message::SearchChanged("goose".into()));
    assert_eq!(app.filtered_sounds().len(), 1);
    assert_eq!(app.filtered_sounds()[0].id, "aaa");
}

#[test]
fn search_is_case_insensitive() {
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
    let _ = app.update(Message::SearchChanged("GOOSE".into()));
    assert_eq!(app.filtered_sounds().len(), 1);
}

#[test]
fn search_and_category_filter_stack() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        SoundEntry {
            id: "aaa".into(),
            name: "Goose Honk".into(),
            path: "/a.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            modified_ms: None,
            category: "Honk".into(),
        },
        SoundEntry {
            id: "bbb".into(),
            name: "Goose Boom".into(),
            path: "/b.mp3".into(),
            format: crate::state::AudioFormat::Mp3,
            duration_ms: Some(1000),
            modified_ms: None,
            category: "Memes".into(),
        },
    ];
    let _ = app.update(Message::SelectCategory(Some("Honk".into())));
    let _ = app.update(Message::SearchChanged("goose".into()));
    assert_eq!(app.filtered_sounds().len(), 1);
    assert_eq!(app.filtered_sounds()[0].id, "aaa");
}

#[test]
fn volume_changed_persists_in_config_across_sounds() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::VolumeChanged(0.15));
    assert!((app.config.volume - 0.15).abs() < f32::EPSILON);

    let _ = app.update(Message::AudioEvent(AudioEvent::PlaybackFinished {
        voice_id: 0,
        sound_id: "old".into(),
        generation: 0,
    }));

    assert!(
        (app.config.volume - 0.15).abs() < f32::EPSILON,
        "config.volume should survive playback cycle"
    );
}

#[test]
fn shortcuts_ready_sets_status_active() {
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.shortcuts_status(), &ShortcutsStatus::Initializing);
    let _ = app.update(Message::ShortcutsReady);
    assert_eq!(app.shortcuts_status(), &ShortcutsStatus::Active);
}

#[test]
fn shortcuts_unavailable_sets_status() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutsUnavailable("portal not found".into()));
    assert!(matches!(
        app.shortcuts_status(),
        ShortcutsStatus::Unavailable(_)
    ));
}

#[test]
fn shortcuts_unavailable_contains_reason() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutsUnavailable("no portal".into()));
    let ShortcutsStatus::Unavailable(reason) = app.shortcuts_status() else {
        panic!("expected Unavailable");
    };
    assert!(!reason.is_empty());
}

#[test]
fn dismiss_warning_sets_flag() {
    let mut app = HonkHonk::new_for_test();
    assert!(!app.shortcuts_warning_dismissed());
    let _ = app.update(Message::DismissShortcutsWarning);
    assert!(app.shortcuts_warning_dismissed());
}

#[test]
fn shortcut_activated_with_empty_slot_is_noop() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutActivated(0));
    assert!(app.playing().is_none());
}

#[test]
fn shortcut_activated_with_assigned_slot_does_not_panic() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/sounds/honk.mp3");
    app.sounds = vec![SoundEntry {
        id: "honk-id".into(),
        name: "Honk".into(),
        path: path.clone(),
        format: crate::state::AudioFormat::Mp3,
        duration_ms: Some(500),
        modified_ms: None,
        category: "Honk".into(),
    }];
    let _ = app.update(Message::AssignSlot(0, path.clone()));
    // audio=None means no audio command is sent; slot must remain assigned after activation
    let _ = app.update(Message::ShortcutActivated(0));
    assert_eq!(app.slots().get(0), Some(&path));
}

#[test]
fn assign_slot_updates_slot_map() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/sounds/boom.mp3");
    let _ = app.update(Message::AssignSlot(3, path.clone()));
    assert_eq!(app.slots().get(3), Some(&path));
}

#[test]
fn clear_slot_removes_assignment() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/sounds/boom.mp3");
    let _ = app.update(Message::AssignSlot(3, path.clone()));
    let _ = app.update(Message::ClearSlot(3));
    assert!(app.slots().get(3).is_none());
}

/// `Message::ClearSlot` must behave identically regardless of which
/// `SlotContent` variant it clears — pinned as a macro-slot counterpart
/// to `clear_slot_removes_assignment` above (#169).
#[test]
fn clear_slot_removes_macro_assignment() {
    let mut app = HonkHonk::new_for_test();
    let id = app.macros.add("Honk combo").id.clone();
    let _ = app.update(Message::AssignMacroSlot(3, id));
    assert!(app.slots().content(3).is_some());
    let _ = app.update(Message::ClearSlot(3));
    assert!(app.slots().content(3).is_none());
}

#[test]
fn open_context_menu_sets_sound_id() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::OpenContextMenu("some-id".into()));
    assert_eq!(app.context_menu(), Some("some-id"));
}

#[test]
fn close_context_menu_clears_it() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::OpenContextMenu("some-id".into()));
    let _ = app.update(Message::CloseContextMenu);
    assert!(app.context_menu().is_none());
}

#[test]
fn show_slots_sets_view_mode() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShowSlots);
    assert_eq!(app.view_mode(), ViewMode::SlotManager);
    assert!(app.selected_slot().is_none());
}

#[test]
fn show_main_resets_view_mode() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SelectSlot(3));
    let _ = app.update(Message::ShowSlots);
    let _ = app.update(Message::ShowMain);
    assert_eq!(app.view_mode(), ViewMode::Main);
    assert!(app.selected_slot().is_none());
}

#[test]
fn select_slot_sets_selected() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SelectSlot(3));
    assert_eq!(app.selected_slot(), Some(3));
}

#[test]
fn clear_slot_keeps_selection_showing_empty_panel() {
    let mut app = HonkHonk::new_for_test();
    let path = std::path::PathBuf::from("/tmp/test.mp3");
    let _ = app.update(Message::AssignSlot(3, path.clone()));
    let _ = app.update(Message::SelectSlot(3));
    let _ = app.update(Message::ClearSlot(3));
    assert_eq!(app.selected_slot(), Some(3));
    assert!(app.slots().get(3).is_none());
}

#[test]
fn shortcut_bindings_updated_stores_triggers() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutBindingsUpdated(vec![
        (0, "Meta+1".into()),
        (4, "Ctrl+5".into()),
    ]));
    assert_eq!(app.slot_triggers()[0].as_deref(), Some("Meta+1"));
    assert_eq!(app.slot_triggers()[4].as_deref(), Some("Ctrl+5"));
    assert!(app.slot_triggers()[1].is_none());
}

#[test]
fn shortcut_bindings_updated_ignores_out_of_range() {
    let mut app = HonkHonk::new_for_test();
    // slot index 20 is out of range — should not panic
    let _ = app.update(Message::ShortcutBindingsUpdated(vec![(20, "X".into())]));
}

#[test]
fn durations_loaded_fills_matching_sound_entries() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![SoundEntry {
        id: "abc123".into(),
        name: "Honk".into(),
        path: "/tmp/honk.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "Honk".into(),
    }];
    let map = std::collections::HashMap::from([("abc123".to_string(), 1500u64)]);
    let _ = app.update(Message::DurationsLoaded(map));
    assert_eq!(app.sounds[0].duration_ms, Some(1500));
    assert!(app.durations_loaded);
}

#[test]
fn durations_loaded_ignores_unmatched_ids() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![SoundEntry {
        id: "abc123".into(),
        name: "Honk".into(),
        path: "/tmp/honk.wav".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: None,
        modified_ms: None,
        category: "Honk".into(),
    }];
    let map = std::collections::HashMap::from([("no-match".to_string(), 999u64)]);
    let _ = app.update(Message::DurationsLoaded(map));
    assert_eq!(app.sounds[0].duration_ms, None);
}
