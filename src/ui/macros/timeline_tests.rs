use super::timeline::*;
use crate::app::macro_editor::{Drag, EditorMessage, EditorState};
use crate::state::{AudioFormat, MacroStore, SoundEntry, Step};
use crate::ui::theme::Theme;
use iced::{Point, Rectangle, Size, mouse};

#[test]
fn extreme_persisted_times_have_bounded_geometry_without_changing_steps() {
    let mut store = MacroStore::default();
    let id = store.add("Extreme").id.clone();
    store.replace_steps(&id, vec![Step::new("/a.wav".into(), u64::MAX)]);
    let before = store.clone();
    let sound = SoundEntry {
        id: "a".into(),
        path: "/a.wav".into(),
        name: "A".into(),
        format: AudioFormat::Wav,
        duration_ms: Some(u64::MAX),
        modified_ms: None,
        category: "Test".into(),
    };
    let mut state = TimelineState::default();
    for sounds in [vec![], vec![sound]] {
        state.sync(store.get(&id), &sounds, Theme::Dark);
        let size = state.size();
        assert!(size.width.is_finite() && size.width <= 16_000.0);
        let rect = state.rectangle(&state.bars[0]);
        assert!(rect.x.is_finite() && rect.width.is_finite());
        assert!(rect.x + rect.width <= size.width + 0.01);
        assert_eq!(state.bars[0].start, u64::MAX);
        assert_eq!(store, before);
    }
}

#[test]
fn resolved_durations_missing_placeholders_and_overlap_lanes_refresh() {
    let mut store = MacroStore::default();
    let id = store.add("Example").id.clone();
    store.replace_steps(
        &id,
        vec![
            Step::new("/a.wav".into(), 0),
            Step::new("/gone.wav".into(), 100),
        ],
    );
    let sound = SoundEntry {
        id: "a".into(),
        path: "/a.wav".into(),
        name: "A".into(),
        format: AudioFormat::Wav,
        duration_ms: Some(2300),
        modified_ms: None,
        category: "Test".into(),
    };
    let mut state = TimelineState::default();
    state.sync(store.get(&id), &[sound], Theme::Dark);
    assert_eq!(state.bars[0].duration, 2300);
    assert!(!state.bars[0].missing);
    assert!(state.bars[1].missing);
    assert_eq!(state.bars[1].label, "Missing sound");
    assert_eq!(state.bars[1].lane, 1);
    state.sync(store.get(&id), &[], Theme::Dark);
    assert!(state.bars[0].missing);
}

#[test]
fn pointer_events_resolve_bar_grab_menu_and_cancelled_palette_drop() {
    let mut editor = EditorState::default();
    editor.timeline.bars.push(Bar {
        start: 100,
        duration: 1000,
        lane: 1,
        label: "A".into(),
        missing: false,
    });
    let bounds = Rectangle::new(Point::new(100.0, 100.0), Size::new(800.0, 240.0));
    let pointer = mouse::Cursor::Available(Point::new(130.0, 170.0));
    let press = iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    assert_eq!(
        Timeline { editor: &editor }.pointer_event(&press, bounds, pointer),
        Some(EditorMessage::MoveStart(0, 20.0))
    );
    let menu = iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
    assert_eq!(
        Timeline { editor: &editor }.pointer_event(&menu, bounds, pointer),
        Some(EditorMessage::Menu(0))
    );
    editor.dragging = Some(Drag::Sound("/a.wav".into()));
    let release = iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
    assert_eq!(
        Timeline { editor: &editor }.pointer_event(&release, bounds, mouse::Cursor::Unavailable),
        Some(EditorMessage::Release(None))
    );
}
