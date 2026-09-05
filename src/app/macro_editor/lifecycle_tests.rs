use super::*;

#[test]
fn dismissing_sort_restores_typing_and_reentry_drops_stale_menu() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShowMacros);
    let _ = app.update(Message::MacroEditor(EditorMessage::ToggleSort));
    let _ = app.update(Message::EscapePressed);
    let _ = app.update(Message::TypeToFilter("ready".into()));
    assert_eq!(app.macro_editor.filter.query(), "ready");
    let _ = app.update(Message::MacroEditor(EditorMessage::ToggleSort));
    let _ = app.update(Message::ShowMain);
    let _ = app.update(Message::ShowMacros);
    assert!(!app.macro_editor.sort_open);
}

#[test]
fn preview_gets_final_frame_after_playback_finishes() {
    let mut app = HonkHonk::new_for_test();
    app.macro_editor.preview_start = Some(Instant::now());
    app.macro_playback = None;
    assert!(app.frame_subscription_needed());
    app.tick_frame(Instant::now());
    assert!(app.macro_editor.preview_start.is_none());
    assert!(!app.frame_subscription_needed());
}

#[test]
fn rescan_refreshes_missing_sound_bars_immediately() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShowMacros);
    let _ = app.update(Message::MacroEditor(EditorMessage::New));
    let _ = app.update(Message::MacroEditor(EditorMessage::Edit(Edit::Add(
        "/a.wav".into(),
        0,
    ))));
    assert!(app.macro_editor.timeline.bars[0].missing);
    let entry = crate::state::SoundEntry {
        id: "a".into(),
        path: "/a.wav".into(),
        name: "A".into(),
        format: crate::state::AudioFormat::Wav,
        duration_ms: Some(100),
        modified_ms: None,
        category: "Test".into(),
    };
    app.apply_library_scan(crate::state::LibraryScan {
        entries: vec![entry],
        complete: true,
    });
    assert!(!app.macro_editor.timeline.bars[0].missing);
}
