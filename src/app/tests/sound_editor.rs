use super::*;

#[test]
fn toggle_favorite_sets_and_clears_favorite() {
    let mut app = HonkHonk::new_for_test();
    assert!(!app.sound_meta.is_favorite("id1"));
    let _ = app.update(Message::ToggleFavorite("id1".into()));
    assert!(app.sound_meta.is_favorite("id1"));
    let _ = app.update(Message::ToggleFavorite("id1".into()));
    assert!(!app.sound_meta.is_favorite("id1"));
}

#[test]
fn open_sound_editor_stores_sound_id_and_draft_state() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![SoundEntry {
        id: "abc".into(),
        name: "Honk".into(),
        path: "/a.mp3".into(),
        format: crate::state::AudioFormat::Mp3,
        duration_ms: None,
        modified_ms: None,
        category: "General".into(),
    }];
    let _ = app.update(Message::OpenSoundEditor("abc".into()));
    assert_eq!(app.editor_sound_id(), Some("abc"));
    // draft volume defaults to 1.0 when no meta saved
    let eps = f32::EPSILON;
    assert!((app.editor_draft_volume - 1.0).abs() < eps);
}

#[test]
fn open_sound_editor_clears_context_menu() {
    // Regression: opening the editor must dismiss the context menu so the
    // editor overlay surfaces immediately (CodeRabbit review thread).
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::OpenContextMenu("some-id".into()));
    assert!(app.context_menu().is_some());
    let _ = app.update(Message::OpenSoundEditor("some-id".into()));
    assert!(app.context_menu().is_none());
    assert_eq!(app.editor_sound_id(), Some("some-id"));
}

#[test]
fn escape_dismisses_editor_before_search_focus() {
    // Regression: Esc should close the editor overlay, not consume search focus.
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SearchChanged("honk".into()));
    let _ = app.update(Message::OpenSoundEditor("abc".into()));
    assert!(app.editor_sound_id().is_some());
    let _ = app.update(Message::EscapePressed);
    assert!(app.editor_sound_id().is_none());
    // The filter focus stage must NOT be consumed — editor took priority.
    assert!(app.filter.had_focus());
}

#[test]
fn close_sound_editor_clears_editor_state() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::OpenSoundEditor("abc".into()));
    let _ = app.update(Message::CloseSoundEditor);
    assert!(app.editor_sound_id().is_none());
}

#[test]
fn sound_editor_name_changed_updates_draft() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SoundEditorNameChanged("New Name".into()));
    assert_eq!(app.editor_draft_name, "New Name");
}

#[test]
fn sound_editor_volume_changed_updates_draft() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SoundEditorVolumeChanged("id".into(), 1.5));
    let eps = 1e-5_f32;
    assert!((app.editor_draft_volume - 1.5).abs() < eps);
}

#[test]
fn sound_editor_volume_changed_clamps_above_two() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SoundEditorVolumeChanged("id".into(), 5.0));
    let eps = f32::EPSILON;
    assert!((app.editor_draft_volume - 2.0).abs() < eps);
}

#[test]
fn save_sound_meta_persists_volume_and_name() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SoundEditorNameChanged("Renamed".into()));
    let _ = app.update(Message::SoundEditorVolumeChanged("id1".into(), 1.25));
    let _ = app.update(Message::SaveSoundMeta("id1".into()));
    let meta = app.sound_meta.get("id1");
    assert_eq!(meta.display_name.as_deref(), Some("Renamed"));
    let eps = 1e-5_f32;
    assert!((meta.volume - 1.25).abs() < eps);
    assert!(
        app.editor_sound_id().is_none(),
        "editor must close after save"
    );
}

#[test]
fn save_sound_meta_blank_name_stores_none() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SoundEditorNameChanged("  ".into()));
    let _ = app.update(Message::SaveSoundMeta("id1".into()));
    assert!(app.sound_meta.get("id1").display_name.is_none());
}

#[test]
fn save_sound_meta_preserves_existing_favorite_flag() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ToggleFavorite("id1".into()));
    assert!(app.sound_meta.is_favorite("id1"));
    let _ = app.update(Message::SoundEditorVolumeChanged("id1".into(), 1.5));
    let _ = app.update(Message::SaveSoundMeta("id1".into()));
    // favorite must still be true after saving from editor
    assert!(app.sound_meta.is_favorite("id1"));
}
