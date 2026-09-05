use super::*;
use crate::audio::effects::EffectSlot;
use crate::state::{AudioFormat, SoundEntry};

fn editor() -> HonkHonk {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShowMacros);
    let _ = app.update(Message::MacroEditor(EditorMessage::New));
    app
}

#[test]
fn clicking_off_grid_step_preserves_time_but_dragging_still_snaps() {
    let mut app = editor();
    let _ = app.update(Message::MacroEditor(EditorMessage::Edit(Edit::Add(
        "/a.wav".into(),
        1234,
    ))));
    let id = app.macro_editor.active.clone().unwrap();
    let press = Point::new(143.4, 30.0);
    let _ = app.update(Message::MacroEditor(EditorMessage::MoveStart(0, 20.0)));
    let _ = app.update(Message::MacroEditor(EditorMessage::Release(Some(press))));
    assert_eq!(app.macros.get(&id).unwrap().steps[0].start_offset_ms, 1234);
    assert!(app.macro_editor.dragging.is_none());
    assert!(app.macro_editor.pointer.is_none());

    let _ = app.update(Message::MacroEditor(EditorMessage::MoveStart(0, 20.0)));
    let moved = Point::new(153.4, 30.0);
    let _ = app.update(Message::MacroEditor(EditorMessage::Pointer(moved)));
    let _ = app.update(Message::MacroEditor(EditorMessage::Release(Some(moved))));
    assert_eq!(app.macros.get(&id).unwrap().steps[0].start_offset_ms, 1350);
    assert!(app.macro_editor.pointer.is_none());
}

#[test]
fn new_drag_discards_stale_pointer_position() {
    let mut app = editor();
    app.macro_editor.pointer = Some(Point::new(999.0, 30.0));
    let _ = app.update(Message::MacroEditor(EditorMessage::PaletteDrag(
        "/a.wav".into(),
    )));
    assert!(app.macro_editor.pointer.is_none());
    app.macro_editor.pointer = Some(Point::new(999.0, 30.0));
    let _ = app.update(Message::MacroEditor(EditorMessage::MoveStart(0, 20.0)));
    assert!(app.macro_editor.pointer.is_none());
}

#[test]
fn palette_drop_and_move_use_timeline_coordinates_and_cancel_outside() {
    let mut app = editor();
    let _ = app.update(Message::MacroEditor(EditorMessage::PaletteDrag(
        "/a.wav".into(),
    )));
    let _ = app.update(Message::MacroEditor(EditorMessage::Release(Some(
        Point::new(12.6, 30.0),
    ))));
    let id = app.macro_editor.active.clone().unwrap();
    assert_eq!(app.macros.get(&id).unwrap().steps[0].start_offset_ms, 150);
    let _ = app.update(Message::MacroEditor(EditorMessage::MoveStart(0, 10.0)));
    let _ = app.update(Message::MacroEditor(EditorMessage::Release(Some(
        Point::new(-20.0, 30.0),
    ))));
    assert_eq!(app.macros.get(&id).unwrap().steps[0].start_offset_ms, 0);
    let _ = app.update(Message::MacroEditor(EditorMessage::PaletteDrag(
        "/b.wav".into(),
    )));
    let _ = app.update(Message::MacroEditor(EditorMessage::Release(None)));
    assert_eq!(app.macros.get(&id).unwrap().steps.len(), 1);
    assert!(app.macro_editor.dragging.is_none());
}

#[test]
fn effects_menu_changes_only_the_step_and_round_trips() {
    let mut app = editor();
    let original = app.effects_ui;
    let _ = app.update(Message::MacroEditor(EditorMessage::Edit(Edit::Add(
        "/a.wav".into(),
        0,
    ))));
    let _ = app.update(Message::MacroEditor(EditorMessage::Menu(0)));
    let effect = Message::SetEffectParamUi {
        slot: EffectSlot::Pitch,
        param: "semitones",
        value: -4.0,
    };
    let _ = app.update(Message::MacroEditor(EditorMessage::Effects(Box::new(
        effect,
    ))));
    let id = app.macro_editor.active.clone().unwrap();
    let step = &app.macros.get(&id).unwrap().steps[0];
    assert_eq!(step.effects.pitch.semitones, -4.0);
    assert!(!step.effects.pitch.bypass);
    assert_eq!(app.effects_ui, original);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("macros.json");
    app.macros.save_to(&path).unwrap();
    assert_eq!(crate::state::MacroStore::load_from(&path), app.macros);
}

#[test]
fn leaving_editor_releases_typing_guard_and_ignores_late_drag() {
    let mut app = editor();
    let _ = app.update(Message::MacroEditor(EditorMessage::BeginRename));
    let _ = app.update(Message::MacroEditor(EditorMessage::PaletteDrag(
        "/a.wav".into(),
    )));
    let _ = app.update(Message::ShowMain);
    let _ = app.update(Message::TypeToFilter("honk".into()));
    assert_eq!(app.filter.query(), "honk");
    let _ = app.update(Message::MacroEditor(EditorMessage::Release(Some(
        Point::new(100.0, 30.0),
    ))));
    assert!(app.macros.iter().next().unwrap().steps.is_empty());
}

#[test]
fn sorting_uses_creation_order_and_real_end_time_and_restores_preference() {
    let mut app = editor();
    let first = app.macro_editor.active.clone().unwrap();
    app.macros.rename(&first, "Zulu");
    app.sounds.push(SoundEntry {
        id: "a".into(),
        path: "/a.wav".into(),
        name: "A".into(),
        format: AudioFormat::Wav,
        duration_ms: Some(2000),
        modified_ms: None,
        category: "Test".into(),
    });
    let _ = app.update(Message::MacroEditor(EditorMessage::Edit(Edit::Add(
        "/a.wav".into(),
        10,
    ))));
    let _ = app.update(Message::MacroEditor(EditorMessage::New));
    let second = app.macro_editor.active.clone().unwrap();
    app.macros.rename(&second, "Alpha");
    let _ = app.update(Message::MacroEditor(EditorMessage::Sort(
        MacroSortKey::Created,
    )));
    assert_eq!(app.macro_rows()[0].value.id, first);
    let _ = app.update(Message::MacroEditor(EditorMessage::Sort(
        MacroSortKey::Length,
    )));
    assert_eq!(app.macro_rows()[0].value.id, second);
    let _ = app.update(Message::MacroEditor(EditorMessage::ToggleDirection));
    assert_eq!(app.macro_rows()[0].value.id, first);
    let mut restored = EditorState::default();
    restored.restore_sort(&app.config);
    assert_eq!(restored.sort, app.macro_editor.sort);
    let _ = app.update(Message::MacroEditor(EditorMessage::Filter("ALP".into())));
    assert_eq!(app.macro_rows().len(), 1);
    assert_eq!(app.macro_rows()[0].value.id, second);
}
