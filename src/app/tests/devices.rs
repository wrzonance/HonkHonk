use super::*;

#[test]
fn input_device_changed_to_none_clears_config() {
    let mut app = HonkHonk::new_for_test();
    app.config = AppConfig {
        input_device: Some("alsa_input.pci-test".into()),
        ..AppConfig::default()
    };
    let _ = app.update(Message::InputDeviceChanged(None));
    assert!(app.config.input_device.is_none());
}

#[test]
fn input_device_changed_to_some_sets_config() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::InputDeviceChanged(Some(
        "alsa_input.usb-mic".into(),
    )));
    assert_eq!(
        app.config.input_device.as_deref(),
        Some("alsa_input.usb-mic")
    );
}

#[test]
fn input_device_changed_same_value_is_idempotent() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::InputDeviceChanged(None));
    let _ = app.update(Message::InputDeviceChanged(None));
    assert!(app.config.input_device.is_none());
}

#[test]
fn audio_event_input_devices_changed_updates_input_devices() {
    let mut app = HonkHonk::new_for_test();
    let devices = vec![("alsa_input.usb-mic".into(), "USB Microphone".into())];
    let _ = app.update(Message::AudioEvent(AudioEvent::InputDevicesChanged(
        devices.clone(),
    )));
    assert_eq!(app.input_devices, devices);
}

#[test]
fn input_devices_changed_does_not_clear_device_before_it_is_first_seen() {
    // Startup race: saved mic not yet enumerated — must NOT clear config.
    let mut app = HonkHonk::new_for_test();
    app.config = AppConfig {
        input_device: Some("alsa_input.usb-mic".into()),
        ..AppConfig::default()
    };
    let _ = app.update(Message::AudioEvent(AudioEvent::InputDevicesChanged(vec![
        ("alsa_input.onboard".into(), "Onboard Mic".into()),
    ])));
    assert_eq!(
        app.config.input_device.as_deref(),
        Some("alsa_input.usb-mic"),
        "must not clear saved mic before it has been enumerated"
    );
}

#[test]
fn input_devices_changed_clears_device_after_it_disappears() {
    // Runtime removal: mic was known, then unplugged — clear config.
    let mut app = HonkHonk::new_for_test();
    app.config = AppConfig {
        input_device: Some("alsa_input.usb-mic".into()),
        ..AppConfig::default()
    };
    let _ = app.update(Message::AudioEvent(AudioEvent::InputDevicesChanged(vec![
        ("alsa_input.usb-mic".into(), "USB Microphone".into()),
    ])));
    assert_eq!(
        app.config.input_device.as_deref(),
        Some("alsa_input.usb-mic")
    );
    let _ = app.update(Message::AudioEvent(AudioEvent::InputDevicesChanged(vec![
        ("alsa_input.onboard".into(), "Onboard Mic".into()),
    ])));
    assert!(
        app.config.input_device.is_none(),
        "must clear config when mic was visible and then removed"
    );
}

#[test]
fn output_devices_changed_keeps_valid_monitor_device() {
    let mut app = HonkHonk::new_for_test();
    app.config = AppConfig {
        monitor_device: Some("alsa_output.pci".into()),
        ..AppConfig::default()
    };
    // Device appears, then stays in subsequent updates
    let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
        ("alsa_output.pci".into(), "Built-in Audio".into()),
    ])));
    let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
        ("alsa_output.pci".into(), "Built-in Audio".into()),
        ("alsa_output.usb".into(), "USB Headset".into()),
    ])));
    assert_eq!(
        app.config.monitor_device.as_deref(),
        Some("alsa_output.pci")
    );
}

#[test]
fn audio_event_output_devices_changed_replaces_previous_list() {
    let mut app = HonkHonk::new_for_test();
    let first = vec![("alsa_output.pci".into(), "Built-in Audio".into())];
    let second = vec![
        ("alsa_output.pci".into(), "Built-in Audio".into()),
        ("alsa_output.usb".into(), "USB Headset".into()),
    ];
    let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(first)));
    let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(
        second.clone(),
    )));
    assert_eq!(app.monitor_devices, second);
}

#[test]
fn open_shortcut_config_sends_command_when_handle_present() {
    use tokio::sync::mpsc;
    let mut app = HonkHonk::new_for_test();
    let (tx, mut rx) = mpsc::channel(8);
    app.shortcut_config.set_portal_sender(tx);
    app.shortcut_config.set_portal_v2_available(true);
    let _ = app.update(Message::OpenShortcutConfig);
    assert!(rx.try_recv().is_ok());
}

#[test]
fn open_shortcut_config_is_noop_when_no_handle() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::OpenShortcutConfig);
}

#[test]
fn escape_first_press_consumes_search_focus_flag_without_clearing_query() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SearchChanged("honk".into()));
    assert!(app.filter.had_focus());
    let _ = app.update(Message::EscapePressed);
    assert!(!app.filter.had_focus());
    assert_eq!(app.search_query(), "honk");
}

#[test]
fn escape_second_press_clears_query() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SearchChanged("honk".into()));
    let _ = app.update(Message::EscapePressed); // consume focus flag
    let _ = app.update(Message::EscapePressed); // clear query
    assert_eq!(app.search_query(), "");
}

#[test]
fn escape_closes_context_menu_without_consuming_search_focus() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::SearchChanged("honk".into()));
    let _ = app.update(Message::OpenContextMenu("test-id".into()));
    let _ = app.update(Message::EscapePressed);
    assert!(app.context_menu().is_none());
    assert!(app.filter.had_focus()); // not consumed — menu took priority
}

#[test]
fn search_changed_sets_filter_focus_stage() {
    let mut app = HonkHonk::new_for_test();
    assert!(!app.filter.had_focus());
    let _ = app.update(Message::SearchChanged("test".into()));
    assert!(app.filter.had_focus());
}

// Per-sound metadata tests
