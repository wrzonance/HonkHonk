use honkhonk::app::{HonkHonk, Message};
use honkhonk::settings::{
    ControlType, SETTINGS_REGISTRY, SettingCategory, SettingId, SettingValue,
};
use honkhonk::state::AppConfig;
use honkhonk::ui::settings::{get_setting_value, setting_message};

#[test]
fn panel_animations_default_enabled_and_backward_compatible() {
    assert!(AppConfig::default().panel_animations);

    let old_json = r#"{
        "sound_directories": [],
        "volume": 0.85,
        "window_width": 900,
        "window_height": 600,
        "theme": "Dark",
        "density": "regular",
        "mic_passthrough": true,
        "mic_passthrough_level": 1.0,
        "renderer": "wgpu",
        "monitor_device": null,
        "input_device": null,
        "overlap_mode": "concurrent"
    }"#;
    let config: AppConfig = serde_json::from_str(old_json).expect("old config should load");
    assert!(config.panel_animations);
}

#[test]
fn panel_animations_round_trip_when_disabled() {
    let config = AppConfig {
        panel_animations: false,
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).expect("config should serialize");
    let back: AppConfig = serde_json::from_str(&json).expect("config should deserialize");
    assert!(!back.panel_animations);
}

#[test]
fn panel_animation_setting_is_toggle_in_appearance() {
    let def = SETTINGS_REGISTRY
        .iter()
        .find(|d| matches!(d.id, SettingId::PanelAnimations))
        .expect("PanelAnimations must be in SETTINGS_REGISTRY");
    assert!(matches!(def.category, SettingCategory::Appearance));
    assert!(matches!(def.control, ControlType::Toggle));
}

#[test]
fn panel_animation_setting_value_reads_config() {
    let app = HonkHonk::new_for_test();
    assert!(matches!(
        get_setting_value(SettingId::PanelAnimations, &app),
        SettingValue::Bool(true)
    ));
}

#[test]
fn panel_animation_setting_message_updates_toggle() {
    let msg = setting_message(SettingId::PanelAnimations, SettingValue::Bool(false));
    assert!(matches!(msg, Message::PanelAnimationsChanged(false)));
}
