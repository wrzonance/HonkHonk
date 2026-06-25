use honkhonk::state::{AppConfig, OverlapMode};

#[test]
fn overlap_mode_interrupt_persists_as_interrupt() {
    let config = AppConfig {
        overlap_mode: OverlapMode::Interrupt,
        ..AppConfig::default()
    };

    let value = serde_json::to_value(config).unwrap();

    assert_eq!(
        value
            .get("overlap_mode")
            .and_then(serde_json::Value::as_str),
        Some("interrupt")
    );
}
