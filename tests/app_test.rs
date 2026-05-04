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
