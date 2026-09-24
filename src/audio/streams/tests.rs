use super::*;
use pipewire::properties::properties;

#[test]
fn monitor_flag_and_monitor_port_names_are_diagnostic_only() {
    for props in [
        properties! { "port.monitor" => "true" },
        properties! { "port.name" => "monitor_FL" },
        properties! { "port.name" => "device:monitor_FR" },
    ] {
        assert!(dict(&props, is_monitor));
    }
    assert!(!dict(
        &properties! { "port.name" => "output_FL" },
        is_monitor
    ));
}

fn dict<F: FnOnce(&DictRef) -> R, R>(props: &pipewire::properties::PropertiesBox, f: F) -> R {
    let d: &DictRef = props.as_ref();
    f(d)
}

#[test]
fn is_own_node_matches_self_pid() {
    let props = properties! { "application.process.id" => "4242" };
    assert!(dict(&props, |d| is_own_node(d, 4242)));
}

#[test]
fn is_own_node_skips_when_pid_differs() {
    let props = properties! { "application.process.id" => "4242" };
    assert!(!dict(&props, |d| is_own_node(d, 9999)));
}

#[test]
fn is_own_node_fails_open_when_pid_missing() {
    let props = properties! { "node.name" => "spotify" };
    assert!(!dict(&props, |d| is_own_node(d, 4242)));
}

#[test]
fn is_own_node_fails_open_on_non_numeric_pid() {
    let props = properties! { "application.process.id" => "not-a-number" };
    assert!(!dict(&props, |d| is_own_node(d, 4242)));
}

#[test]
fn extract_name_uses_description_first() {
    let props = properties! {
        "node.description" => "Spotify Premium",
        "node.nick" => "Spotify",
        "node.name" => "spotify",
    };
    assert_eq!(dict(&props, extract_name), "Spotify Premium");
}

#[test]
fn extract_name_falls_back_to_nick() {
    let props = properties! {
        "node.nick" => "Spotify",
        "node.name" => "spotify",
    };
    assert_eq!(dict(&props, extract_name), "Spotify");
}

#[test]
fn extract_name_falls_back_to_node_name() {
    let props = properties! { "node.name" => "spotify" };
    assert_eq!(dict(&props, extract_name), "spotify");
}

#[test]
fn extract_name_defaults_to_unknown_when_all_missing() {
    let props = properties! { "media.class" => "Stream/Output/Audio" };
    assert_eq!(dict(&props, extract_name), "unknown");
}

#[test]
fn extract_pid_parses_numeric() {
    let props = properties! { "application.process.id" => "1234" };
    assert_eq!(dict(&props, extract_pid), Some(1234));
}

#[test]
fn extract_pid_returns_none_when_missing() {
    let props = properties! { "node.name" => "spotify" };
    assert_eq!(dict(&props, extract_pid), None);
}

#[test]
fn extract_pid_returns_none_on_non_numeric() {
    let props = properties! { "application.process.id" => "abc" };
    assert_eq!(dict(&props, extract_pid), None);
}

#[test]
fn extract_opt_returns_value_when_present() {
    let props = properties! { "application.name" => "Firefox" };
    assert_eq!(
        dict(&props, |d| extract_opt(d, "application.name")),
        Some("Firefox".to_owned())
    );
}

#[test]
fn extract_opt_returns_none_when_absent() {
    let props = properties! { "node.name" => "x" };
    assert_eq!(dict(&props, |d| extract_opt(d, "application.name")), None);
}

#[test]
fn direction_variants_are_distinct() {
    // Pin the public enum surface so a future enum-shuffle review
    // catches accidental variant reorder/removal.
    assert_ne!(Direction::Input, Direction::Output);
}
