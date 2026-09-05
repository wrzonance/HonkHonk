use super::*;

#[test]
fn show_settings_sets_view_mode() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::Settings(SettingsMessage::Show));
    assert!(matches!(app.view_mode, ViewMode::Settings));
}

#[test]
fn show_settings_defaults_section_to_audio() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::Settings(SettingsMessage::Show));
    assert_eq!(app.settings_ui.section(), SettingsSection::Audio);
}

#[test]
fn show_settings_section_updates_active_section() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::Settings(SettingsMessage::ShowSection(
        SettingsSection::Library,
    )));
    assert_eq!(app.settings_ui.section(), SettingsSection::Library);
}

#[test]
fn show_main_from_settings_resets_view_mode() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::Settings(SettingsMessage::Show));
    let _ = app.update(Message::ShowMain);
    assert!(matches!(app.view_mode, ViewMode::Main));
}

#[test]
fn theme_changed_updates_config() {
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.config.theme, crate::ui::theme::Theme::Dark);
    let _ = app.update(Message::ThemeChanged(crate::ui::theme::Theme::Light));
    assert_eq!(app.config.theme, crate::ui::theme::Theme::Light);
    let _ = app.update(Message::ThemeChanged(crate::ui::theme::Theme::System));
    assert_eq!(app.config.theme, crate::ui::theme::Theme::System);
}

#[test]
fn density_changed_updates_config() {
    let mut app = HonkHonk::new_for_test();
    assert_eq!(app.config.density, crate::state::config::Density::Regular);
    let _ = app.update(Message::DensityChanged(
        crate::state::config::Density::Compact,
    ));
    assert_eq!(app.config.density, crate::state::config::Density::Compact);
    let _ = app.update(Message::DensityChanged(
        crate::state::config::Density::Comfy,
    ));
    assert_eq!(app.config.density, crate::state::config::Density::Comfy);
}

#[test]
fn mic_passthrough_changed_message_carries_bool() {
    let msg = Message::MicPassthroughChanged(false);
    assert!(matches!(msg, Message::MicPassthroughChanged(false)));
}

#[test]
fn mic_passthrough_level_changed_message_carries_f32() {
    let msg = Message::MicPassthroughLevelChanged(0.5);
    assert!(matches!(msg, Message::MicPassthroughLevelChanged(_)));
}

#[test]
fn renderer_changed_dispatches_to_update() {
    use crate::state::Renderer;
    let mut app = HonkHonk::new_for_test();
    // default is Wgpu
    let _ = app.update(Message::RendererChanged(Renderer::TinySkia));
    assert_eq!(app.config.renderer, Renderer::TinySkia);
    let _ = app.update(Message::RendererChanged(Renderer::Wgpu));
    assert_eq!(app.config.renderer, Renderer::Wgpu);
}

#[test]
fn renderer_changed_no_op_when_value_unchanged() {
    use crate::state::Renderer;
    let mut app = HonkHonk::new_for_test();
    // start: Wgpu (default). Send TinySkia, verify change.
    let _ = app.update(Message::RendererChanged(Renderer::TinySkia));
    assert_eq!(app.config.renderer, Renderer::TinySkia);
    // send TinySkia again — state must not corrupt
    let _ = app.update(Message::RendererChanged(Renderer::TinySkia));
    assert_eq!(app.config.renderer, Renderer::TinySkia);
}

#[test]
fn monitor_device_changed_to_none_clears_config() {
    let mut app = HonkHonk::new_for_test();
    app.config = AppConfig {
        monitor_device: Some("alsa_output.pci-test".into()),
        ..AppConfig::default()
    };
    let _ = app.update(Message::MonitorDeviceChanged(None));
    assert!(app.config.monitor_device.is_none());
}

#[test]
fn monitor_device_changed_to_some_sets_config() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::MonitorDeviceChanged(Some(
        "alsa_output.pci-test".into(),
    )));
    assert_eq!(
        app.config.monitor_device.as_deref(),
        Some("alsa_output.pci-test")
    );
}

#[test]
fn monitor_device_changed_same_value_is_idempotent() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::MonitorDeviceChanged(None));
    let _ = app.update(Message::MonitorDeviceChanged(None));
    assert!(app.config.monitor_device.is_none());
}

#[test]
fn audio_event_output_devices_changed_updates_monitor_devices() {
    let mut app = HonkHonk::new_for_test();
    let devices = vec![("alsa_output.pci".into(), "Built-in Audio".into())];
    let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(
        devices.clone(),
    )));
    assert_eq!(app.monitor_devices, devices);
}

#[test]
fn output_devices_changed_does_not_clear_device_before_it_is_first_seen() {
    // Startup race: saved device not yet enumerated — must NOT clear config
    let mut app = HonkHonk::new_for_test();
    app.config = AppConfig {
        monitor_device: Some("alsa_output.usb-headset".into()),
        ..AppConfig::default()
    };
    // monitor_devices is empty (startup) — first event only contains a different sink
    let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
        ("alsa_output.hdmi".into(), "HDMI Audio".into()),
    ])));
    assert_eq!(
        app.config.monitor_device.as_deref(),
        Some("alsa_output.usb-headset"),
        "must not clear saved device before it has been enumerated"
    );
}

#[test]
fn output_devices_changed_clears_device_after_it_disappears() {
    // Runtime removal: device was known, then removed — clear config
    let mut app = HonkHonk::new_for_test();
    app.config = AppConfig {
        monitor_device: Some("alsa_output.usb-headset".into()),
        ..AppConfig::default()
    };
    // First: device appears in list (now it's "seen")
    let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
        ("alsa_output.usb-headset".into(), "USB Headset".into()),
    ])));
    assert_eq!(
        app.config.monitor_device.as_deref(),
        Some("alsa_output.usb-headset")
    );
    // Then: device disappears (unplugged)
    let _ = app.update(Message::AudioEvent(AudioEvent::OutputDevicesChanged(vec![
        ("alsa_output.pci".into(), "Built-in Audio".into()),
    ])));
    assert!(
        app.config.monitor_device.is_none(),
        "must clear config when device was visible and then removed"
    );
}
