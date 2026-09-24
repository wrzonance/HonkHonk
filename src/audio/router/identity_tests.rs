fn identity(name: Option<&str>, binary: Option<&str>, pid: Option<u32>) -> AppIdentity {
    AppIdentity {
        app_name: name.map(str::to_owned),
        process_binary: binary.map(str::to_owned),
        process_id: pid,
    }
}

use super::*;

// ── AppIdentity::matches tests ────────────────────────────────────────────

#[test]
fn identity_matches_by_app_name() {
    let intent = identity(Some("Spotify"), None, None);
    let candidate = identity(Some("Spotify"), Some("spotify"), Some(1234));
    assert!(intent.matches(&candidate));
}

#[test]
fn identity_no_match_different_app_name() {
    let intent = identity(Some("Spotify"), None, None);
    let candidate = identity(Some("Firefox"), None, None);
    assert!(!intent.matches(&candidate));
}

#[test]
fn identity_falls_back_to_binary_when_app_name_none() {
    let intent = identity(None, Some("spotify"), None);
    let candidate = identity(None, Some("spotify"), Some(999));
    assert!(intent.matches(&candidate));
}

#[test]
fn identity_no_match_binary_differs() {
    let intent = identity(None, Some("spotify"), None);
    let candidate = identity(None, Some("firefox"), None);
    assert!(!intent.matches(&candidate));
}

#[test]
fn identity_both_none_returns_false() {
    let intent = identity(None, None, None);
    let candidate = identity(None, None, None);
    assert!(!intent.matches(&candidate));
}

#[test]
fn identity_pid_required_when_set_in_intent() {
    let intent = identity(Some("Spotify"), None, Some(1234));
    let same_pid = identity(Some("Spotify"), None, Some(1234));
    let diff_pid = identity(Some("Spotify"), None, Some(9999));
    assert!(intent.matches(&same_pid));
    assert!(!intent.matches(&diff_pid));
}

#[test]
fn identity_no_pid_in_intent_matches_all_instances() {
    let intent = identity(Some("Spotify"), None, None);
    let inst1 = identity(Some("Spotify"), None, Some(1111));
    let inst2 = identity(Some("Spotify"), None, Some(2222));
    assert!(intent.matches(&inst1));
    assert!(intent.matches(&inst2));
}

#[test]
fn identity_app_name_takes_priority_over_binary_mismatch() {
    let intent = identity(Some("Spotify"), Some("different-binary"), None);
    let candidate = identity(Some("Spotify"), Some("spotify"), None);
    assert!(intent.matches(&candidate));
}

#[test]
fn identity_intent_app_name_none_candidate_has_name_falls_to_binary() {
    let intent = identity(None, Some("spotify"), None);
    let candidate = identity(Some("Spotify"), Some("spotify"), None);
    assert!(intent.matches(&candidate));
}

#[test]
fn from_stream_round_trips_fields() {
    let id = AppIdentity::from_stream(Some("Firefox".into()), Some("firefox".into()), Some(4242));
    assert_eq!(id.app_name.as_deref(), Some("Firefox"));
    assert_eq!(id.process_binary.as_deref(), Some("firefox"));
    assert_eq!(id.process_id, Some(4242));
}

#[test]
fn router_command_variants_constructible() {
    let _ = RouterCommand::RouteSource { source_node_id: 1 };
    let _ = RouterCommand::UnrouteSource { source_node_id: 1 };
    let _ = RouterCommand::UnrouteAll;
}

#[test]
fn router_event_variants_constructible() {
    let id = AppIdentity::from_stream(None, Some("test".into()), None);
    let _ = RouterEvent::RouteCreated {
        node_id: 1,
        identity: id.clone(),
    };
    let _ = RouterEvent::RouteDestroyed { node_id: 1 };
    let _ = RouterEvent::AutoReconnected {
        identity: id.clone(),
        node_id: 1,
    };
    let _ = RouterEvent::SourceDisconnected { identity: id };
    let _ = RouterEvent::Error(RouterError::SinkPortsUnavailable);
}
