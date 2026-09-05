use super::*;
use crate::state::Step;

#[test]
fn edits_and_selection_preserve_saved_buffers() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShowMacros);
    let _ = app.update(Message::MacroEditor(EditorMessage::New));
    let id = app.macro_editor.active.clone().unwrap();
    for edit in [
        Edit::Add("/a.wav".into(), 125),
        Edit::Move(0, 250),
        Edit::Gain(0, 1.5),
        Edit::Duplicate(0),
    ] {
        let _ = app.update(Message::MacroEditor(EditorMessage::Edit(edit)));
    }
    assert_eq!(app.macros.get(&id).unwrap().steps.len(), 2);
    assert_eq!(app.macros.get(&id).unwrap().steps[1].start_offset_ms, 250);
    assert_eq!(app.macros.get(&id).unwrap().steps[1].gain, 1.5);
    let _ = app.update(Message::MacroEditor(EditorMessage::Edit(Edit::Remove(0))));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("macros.json");
    app.macros.save_to(&path).unwrap();
    assert_eq!(crate::state::MacroStore::load_from(&path), app.macros);
    let _ = app.update(Message::MacroEditor(EditorMessage::New));
    let _ = app.update(Message::MacroEditor(EditorMessage::Select(id.clone())));
    assert_eq!(app.macros.get(&id).unwrap().steps.len(), 1);
}

#[test]
fn rename_guards_type_to_filter_and_sort_persists() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShowMacros);
    let _ = app.update(Message::MacroEditor(EditorMessage::New));
    let _ = app.update(Message::MacroEditor(EditorMessage::BeginRename));
    let _ = app.update(Message::TypeToFilter("hidden".into()));
    assert!(app.macro_editor.filter.query().is_empty());
    let _ = app.update(Message::MacroEditor(EditorMessage::Rename("Name".into())));
    let _ = app.update(Message::MacroEditor(EditorMessage::EndRename));
    let _ = app.update(Message::TypeToFilter("Name".into()));
    assert_eq!(app.macro_editor.filter.query(), "Name");
    let _ = app.update(Message::MacroEditor(EditorMessage::Sort(
        MacroSortKey::Length,
    )));
    assert_eq!(app.config.sort_prefs["macros"].key(), "length");
}

#[test]
fn recording_is_adopted_and_edit_cancels_preview() {
    let mut app = HonkHonk::new_for_test();
    app.start_recording_at(Instant::now());
    app.capture_recording_at(std::path::Path::new("/a.wav"), Instant::now());
    let _ = app.update(Message::StopRecording);
    let _ = app.update(Message::ShowMacros);
    let id = app.macro_editor.active.clone().unwrap();
    assert_eq!(app.macros.get(&id).unwrap().steps.len(), 1);
    let _ = app.update(Message::MacroEditor(EditorMessage::Play));
    assert!(app.macro_playback.is_some());
    let _ = app.update(Message::MacroEditor(EditorMessage::Edit(Edit::Move(
        0, 200,
    ))));
    assert!(app.macro_playback.is_none());
    assert_eq!(
        app.macros.get(&id).unwrap().steps[0],
        Step::new("/a.wav".into(), 200)
    );
}
