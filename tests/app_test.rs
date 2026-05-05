use honkhonk::app::{HonkHonk, Message};

#[test]
fn quit_message_sets_should_exit() {
    let mut app = HonkHonk::new_for_test();
    let _task = app.update(Message::Quit);
    assert!(app.should_exit());
}

#[test]
fn toggle_visibility_flips_visible_state() {
    let mut app = HonkHonk::new_for_test();
    assert!(app.is_visible());
    let _task = app.update(Message::ToggleVisibility);
    assert!(!app.is_visible());
    let _task = app.update(Message::ToggleVisibility);
    assert!(app.is_visible());
}

#[test]
fn tray_event_quit_maps_to_quit_message() {
    let msg = Message::from_tray_event(honkhonk::tray::TrayEvent::Quit);
    assert_eq!(msg, Message::Quit);
}

#[test]
fn tray_event_toggle_maps_to_toggle_message() {
    let msg = Message::from_tray_event(honkhonk::tray::TrayEvent::ToggleVisibility);
    assert_eq!(msg, Message::ToggleVisibility);
}

#[test]
fn toggle_visibility_does_not_exit() {
    let mut app = HonkHonk::new_for_test();
    let _task = app.update(Message::ToggleVisibility);
    assert!(!app.should_exit());
    assert!(!app.is_visible());
}

#[test]
fn audio_playback_started_sets_playing() {
    let mut app = HonkHonk::new_for_test();
    let event = honkhonk::audio::AudioEvent::PlaybackStarted {
        sound_id: "test-id".into(),
    };
    let _task = app.update(Message::AudioEvent(event));
    assert_eq!(app.playing(), Some("test-id"));
}

#[test]
fn audio_playback_finished_clears_playing() {
    let mut app = HonkHonk::new_for_test();
    let started = honkhonk::audio::AudioEvent::PlaybackStarted {
        sound_id: "test-id".into(),
    };
    let _task = app.update(Message::AudioEvent(started));
    assert_eq!(app.playing(), Some("test-id"));

    let finished = honkhonk::audio::AudioEvent::PlaybackFinished {
        sound_id: "test-id".into(),
    };
    let _task = app.update(Message::AudioEvent(finished));
    assert_eq!(app.playing(), None);
}

#[test]
fn stop_all_clears_playing() {
    let mut app = HonkHonk::new_for_test();
    let started = honkhonk::audio::AudioEvent::PlaybackStarted {
        sound_id: "x".into(),
    };
    let _task = app.update(Message::AudioEvent(started));
    assert!(app.playing().is_some());

    let _task = app.update(Message::StopAll);
    assert_eq!(app.playing(), None);
}

#[test]
fn select_category_updates_state() {
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.active_category(), None);

    let _task = app.update(Message::SelectCategory(Some("Memes".into())));
    assert_eq!(app.active_category(), Some("Memes"));

    let _task = app.update(Message::SelectCategory(None));
    assert_eq!(app.active_category(), None);
}

#[test]
fn play_sound_with_no_matching_id_does_not_crash() {
    let mut app = HonkHonk::new_for_test();
    let _task = app.update(Message::PlaySound("nonexistent".into()));
    assert_eq!(app.playing(), None);
}
