use super::*;
use std::fs;

#[test]
fn default_config_has_expected_values() {
    let config = AppConfig::default();
    assert_eq!(config.volume, 0.85);
    assert_eq!(config.window_width, 900);
    assert_eq!(config.window_height, 600);
    assert!(config.mic_passthrough);
    let eps = 1e-6_f32;
    assert!((config.mic_passthrough_level - 1.0).abs() < eps);
}

#[test]
fn default_density_is_regular() {
    assert_eq!(AppConfig::default().density, Density::Regular);
}

#[test]
fn default_overlap_mode_is_concurrent() {
    assert_eq!(AppConfig::default().overlap_mode, OverlapMode::Concurrent);
}

#[test]
fn density_columns_compact_is_6() {
    assert_eq!(Density::Compact.columns(), 6);
}

#[test]
fn density_columns_regular_is_5() {
    assert_eq!(Density::Regular.columns(), 5);
}

#[test]
fn density_columns_comfy_is_4() {
    assert_eq!(Density::Comfy.columns(), 4);
}

#[test]
fn density_round_trips_through_json() {
    for d in [Density::Compact, Density::Regular, Density::Comfy] {
        let json = serde_json::to_string(&d).unwrap();
        let back: Density = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}

#[test]
fn round_trip_serialize_deserialize() {
    let config = AppConfig {
        processing: Default::default(),
        sound_directories: vec![PathBuf::from("/tmp/sounds")],
        volume: 0.5,
        window_width: 1024,
        window_height: 768,
        theme: Theme::Dark,
        density: Density::Compact,
        mic_passthrough: true,
        mic_passthrough_level: 0.75,
        renderer: Renderer::Wgpu,
        monitor_device: None,
        input_device: None,
        overlap_mode: OverlapMode::Concurrent,
        panel_animations: true,
        sort_prefs: BTreeMap::new(),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config, deserialized);
}

#[test]
fn save_and_load_from_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");

    let config = AppConfig {
        processing: Default::default(),
        sound_directories: vec![PathBuf::from("/home/user/sounds")],
        volume: 0.7,
        window_width: 800,
        window_height: 500,
        theme: Theme::Dark,
        density: Density::Comfy,
        mic_passthrough: false,
        mic_passthrough_level: 0.5,
        renderer: Renderer::Wgpu,
        monitor_device: None,
        input_device: None,
        overlap_mode: OverlapMode::Concurrent,
        panel_animations: true,
        sort_prefs: BTreeMap::new(),
    };

    config.save_to(&path).unwrap();
    let loaded = AppConfig::load_from(&path).unwrap();

    assert_eq!(config, loaded);
}

#[test]
fn load_missing_creates_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subdir/config.json");

    let loaded = AppConfig::load_from(&path).unwrap();
    assert_eq!(loaded, AppConfig::default());
    assert!(path.exists());
}

#[test]
fn default_mic_passthrough_is_true() {
    assert!(AppConfig::default().mic_passthrough);
}

#[test]
fn default_mic_passthrough_level_is_one() {
    let eps = 1e-6_f32;
    assert!((AppConfig::default().mic_passthrough_level - 1.0).abs() < eps);
}

#[test]
fn mic_passthrough_false_round_trips_json() {
    let config = AppConfig {
        mic_passthrough: false,
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(!back.mic_passthrough);
}

#[test]
fn mic_passthrough_level_round_trips_json() {
    let config = AppConfig {
        mic_passthrough_level: 0.42,
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    let eps = 1e-5_f32;
    assert!((back.mic_passthrough_level - 0.42).abs() < eps);
}

#[test]
fn missing_mic_passthrough_field_deserializes_to_default() {
    // Simulates loading a config written before this field existed.
    let json = r#"{"sound_directories":[],"volume":0.85,"window_width":900,"window_height":600,"theme":"Dark","density":"regular"}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert!(config.mic_passthrough);
    let eps = 1e-6_f32;
    assert!((config.mic_passthrough_level - 1.0).abs() < eps);
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a/b/c/config.json");

    let config = AppConfig::default();
    config.save_to(&path).unwrap();

    assert!(path.exists());
    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains("volume"));
}

#[test]
fn renderer_default_is_wgpu() {
    assert_eq!(AppConfig::default().renderer, Renderer::Wgpu);
}

#[test]
fn renderer_round_trips_json() {
    for (variant, expected_str) in [
        (Renderer::Wgpu, "\"wgpu\""),
        (Renderer::TinySkia, "\"tiny-skia\""),
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected_str, "Renderer::{variant:?} serialized wrong");
        let back: Renderer = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn missing_renderer_field_deserializes_to_wgpu() {
    let json = r#"{"sound_directories":[],"volume":0.85,"window_width":900,"window_height":600,"theme":"Dark","density":"regular","mic_passthrough":true,"mic_passthrough_level":1.0}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.renderer, Renderer::Wgpu);
}

#[test]
fn missing_monitor_device_field_deserializes_to_none() {
    let json = r#"{"sound_directories":[],"volume":0.85,"window_width":900,"window_height":600,"theme":"Dark","density":"regular","mic_passthrough":true,"mic_passthrough_level":1.0,"renderer":"wgpu"}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert!(config.monitor_device.is_none());
}

#[test]
fn monitor_device_none_round_trips_json() {
    let config = AppConfig {
        monitor_device: None,
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(back.monitor_device.is_none());
}

#[test]
fn monitor_device_some_round_trips_json() {
    let config = AppConfig {
        monitor_device: Some("alsa_output.pci-0000_00_1f.3.analog-stereo".into()),
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.monitor_device.as_deref(),
        Some("alsa_output.pci-0000_00_1f.3.analog-stereo")
    );
}

#[test]
fn missing_input_device_field_deserializes_to_none() {
    // Simulates loading a config written before the input_device field existed.
    let json = r#"{"sound_directories":[],"volume":0.85,"window_width":900,"window_height":600,"theme":"Dark","density":"regular","mic_passthrough":true,"mic_passthrough_level":1.0,"renderer":"wgpu","monitor_device":null}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert!(config.input_device.is_none());
}

#[test]
fn input_device_none_round_trips_json() {
    let config = AppConfig {
        input_device: None,
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(back.input_device.is_none());
}

#[test]
fn input_device_some_round_trips_json() {
    let config = AppConfig {
        input_device: Some("alsa_input.usb-OBSBOT_Meet_2-00.analog-stereo".into()),
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.input_device.as_deref(),
        Some("alsa_input.usb-OBSBOT_Meet_2-00.analog-stereo")
    );
}

#[test]
fn missing_overlap_mode_field_deserializes_to_concurrent() {
    let json = r#"{"sound_directories":[],"volume":0.85,"window_width":900,"window_height":600,"theme":"Dark","density":"regular","mic_passthrough":true,"mic_passthrough_level":1.0,"renderer":"wgpu","monitor_device":null,"input_device":null}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.overlap_mode, OverlapMode::Concurrent);
}

#[test]
fn overlap_mode_interrupt_round_trips_json() {
    let config = AppConfig {
        overlap_mode: OverlapMode::Interrupt,
        ..AppConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.overlap_mode, OverlapMode::Interrupt);
}
