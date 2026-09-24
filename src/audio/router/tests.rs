use super::*;
use std::sync::mpsc;

fn make_router() -> (Router, mpsc::Receiver<RouterEvent>) {
    let (tx, rx) = mpsc::channel();
    let router = Router::new(tx);
    (router, rx)
}

// ── Router state management tests ────────────────────────────────────────

#[test]
fn router_new_has_empty_state() {
    let (router, _rx) = make_router();
    assert!(router.intents.is_empty());
    assert!(router.active_links.is_empty());
    assert!(router.known_sources.is_empty());
    assert!(router.sink_input_ports.is_empty());
}

#[test]
fn update_sink_ports_stores_ports() {
    let (mut router, _rx) = make_router();
    router.update_sink_ports(vec![10, 11]);
    assert_eq!(router.sink_input_ports, vec![10, 11]);
}

#[test]
fn source_added_event_stores_identity_and_ports() {
    let (mut router, _rx) = make_router();
    router.on_source_added(
        42,
        Some("Spotify".into()),
        Some("spotify".into()),
        Some(1234),
    );
    assert!(router.known_sources.contains_key(&42));
    let info = &router.known_sources[&42];
    assert_eq!(info.identity.app_name.as_deref(), Some("Spotify"));
}

#[test]
fn port_added_for_tracked_source_accumulates_ports() {
    let (mut router, _rx) = make_router();
    router.on_source_added(42, Some("Spotify".into()), None, None);
    router.on_port_added(100, 42, "FL".into(), Direction::Output);
    router.on_port_added(101, 42, "FR".into(), Direction::Output);
    let info = &router.known_sources[&42];
    assert_eq!(info.output_ports, vec![100, 101]);
}

#[test]
fn port_added_for_untracked_source_is_buffered_not_in_known_sources() {
    let (mut router, _rx) = make_router();
    router.on_port_added(100, 999, "FL".into(), Direction::Output);
    // Does not fabricate a known_sources entry (which would have an empty
    // identity)...
    assert!(router.known_sources.is_empty());
    // ...but is retained for reconciliation when SourceAdded arrives.
    assert_eq!(router.pending_ports[&999], vec![100]);
}

#[test]
fn ports_before_source_are_buffered_and_reconciled() {
    // Regression (PR #103 verification): PipeWire registry replay delivers a
    // pre-existing app's output ports BEFORE the async SourceAdded event, so
    // ports arrive while the node is still untracked. They must be buffered
    // and reconciled when the source arrives — not dropped — otherwise an
    // already-playing app (e.g. Firefox playing on launch) can never route.
    let (mut router, _rx) = make_router();
    router.on_port_added(131, 109, "FL".into(), Direction::Output);
    router.on_port_added(134, 109, "FR".into(), Direction::Output);
    router.on_source_added(
        109,
        Some("Firefox".into()),
        Some("firefox".into()),
        Some(12529),
    );
    let info = &router.known_sources[&109];
    assert_eq!(info.output_ports, vec![131, 134]);
}

#[test]
fn already_playing_source_routes_after_ports_then_source() {
    // End-to-end of the same regression: with ports delivered before the
    // source (the replay ordering for an already-playing app), a subsequent
    // RouteSource command must succeed, not error SourcePortsUnavailable.
    let (mut router, rx) = make_router();
    router.update_sink_ports(vec![10, 11]);
    router.on_port_added(131, 109, "FL".into(), Direction::Output);
    router.on_port_added(134, 109, "FR".into(), Direction::Output);
    router.on_source_added(
        109,
        Some("Firefox".into()),
        Some("firefox".into()),
        Some(12529),
    );

    router.route_source_test(109);

    match rx.try_recv().expect("expected a RouterEvent") {
        RouterEvent::RouteCreated { node_id, .. } => assert_eq!(node_id, 109),
        other => panic!("expected RouteCreated, got {other:?}"),
    }
}

#[test]
fn source_removed_discards_buffered_ports() {
    // Buffered ports for a node that is removed before its source resolves
    // must not leak into a later, unrelated source on the same node id.
    let (mut router, _rx) = make_router();
    router.on_port_added(131, 109, "FL".into(), Direction::Output);
    router.on_source_removed(109);
    router.on_source_added(109, Some("Firefox".into()), None, None);
    assert!(router.known_sources[&109].output_ports.is_empty());
}

#[test]
fn source_removed_cleans_active_links_but_preserves_intent() {
    let (mut router, rx) = make_router();
    let id = AppIdentity::from_stream(Some("Spotify".into()), None, None);
    router.intents.push(RouteIntent {
        identity: id,
        enabled: true,
    });
    router.active_links.insert(42, vec![]);
    router.on_source_added(42, Some("Spotify".into()), None, None);

    router.on_source_removed(42);

    assert!(!router.active_links.contains_key(&42));
    assert_eq!(router.intents.len(), 1);
    assert!(!router.known_sources.contains_key(&42));

    let events: Vec<RouterEvent> = rx.try_iter().collect();
    let disconnected = events
        .iter()
        .any(|e| matches!(e, RouterEvent::SourceDisconnected { .. }));
    assert!(disconnected, "expected SourceDisconnected in {events:?}");
}

#[test]
fn unroute_all_clears_intents_and_emits_route_destroyed() {
    let (mut router, rx) = make_router();
    router.active_links.insert(1, vec![]);
    router.active_links.insert(2, vec![]);
    router.intents.push(RouteIntent {
        identity: AppIdentity::from_stream(Some("A".into()), None, None),
        enabled: true,
    });

    router.handle_command_unroute_all();

    assert!(router.intents.is_empty());
    assert!(router.active_links.is_empty());

    let events: Vec<RouterEvent> = rx.try_iter().collect();
    let destroyed_ids: Vec<u32> = events
        .iter()
        .filter_map(|e| match e {
            RouterEvent::RouteDestroyed { node_id } => Some(*node_id),
            _ => None,
        })
        .collect();
    assert!(destroyed_ids.contains(&1));
    assert!(destroyed_ids.contains(&2));
}

#[test]
fn unroute_source_removes_intent_and_active_link() {
    let (mut router, rx) = make_router();
    let id = AppIdentity::from_stream(Some("Spotify".into()), None, None);
    router.intents.push(RouteIntent {
        identity: id.clone(),
        enabled: true,
    });
    router.active_links.insert(42, vec![]);
    router.on_source_added(42, Some("Spotify".into()), None, None);

    router.handle_command_unroute_source(42);

    assert!(!router.active_links.contains_key(&42));
    assert!(!router.intents[0].enabled);

    let events: Vec<RouterEvent> = rx.try_iter().collect();
    let destroyed = events
        .iter()
        .any(|e| matches!(e, RouterEvent::RouteDestroyed { node_id: 42 }));
    assert!(destroyed, "expected RouteDestroyed(42) in {events:?}");
}

#[test]
fn route_source_adds_intent_and_sends_route_created() {
    let (mut router, rx) = make_router();
    router.update_sink_ports(vec![10, 11]);
    router.on_source_added(42, Some("Spotify".into()), None, None);
    router.on_port_added(100, 42, "FL".into(), Direction::Output);
    router.on_port_added(101, 42, "FR".into(), Direction::Output);

    router.route_source_test(42);

    assert_eq!(router.intents.len(), 1);
    assert!(router.intents[0].enabled);

    let events: Vec<RouterEvent> = rx.try_iter().collect();
    let created = events
        .iter()
        .any(|e| matches!(e, RouterEvent::RouteCreated { node_id: 42, .. }));
    assert!(created, "expected RouteCreated(42) in {events:?}");
}

#[test]
fn auto_reconnect_fires_when_source_added_matches_intent() {
    let (mut router, rx) = make_router();
    router.update_sink_ports(vec![10, 11]);
    router.intents.push(RouteIntent {
        identity: AppIdentity::from_stream(Some("Spotify".into()), None, None),
        enabled: true,
    });
    router.on_source_added(99, Some("Spotify".into()), None, None);
    router.on_port_added(200, 99, "FL".into(), Direction::Output);
    router.on_port_added(201, 99, "FR".into(), Direction::Output);
    router.try_auto_reconnect_test(99);

    assert!(router.active_links.contains_key(&99));

    let events: Vec<RouterEvent> = rx.try_iter().collect();
    let reconnected = events
        .iter()
        .any(|e| matches!(e, RouterEvent::AutoReconnected { node_id: 99, .. }));
    assert!(reconnected, "expected AutoReconnected(99) in {events:?}");
}

#[test]
fn auto_reconnect_does_not_fire_for_disabled_intent() {
    let (mut router, rx) = make_router();
    router.update_sink_ports(vec![10, 11]);
    router.intents.push(RouteIntent {
        identity: AppIdentity::from_stream(Some("Spotify".into()), None, None),
        enabled: false,
    });
    router.on_source_added(99, Some("Spotify".into()), None, None);
    router.on_port_added(200, 99, "FL".into(), Direction::Output);
    router.on_port_added(201, 99, "FR".into(), Direction::Output);
    router.try_auto_reconnect_test(99);

    assert!(!router.active_links.contains_key(&99));
    let events: Vec<RouterEvent> = rx.try_iter().collect();
    let reconnected = events
        .iter()
        .any(|e| matches!(e, RouterEvent::AutoReconnected { .. }));
    assert!(!reconnected, "unexpected AutoReconnected in {events:?}");
}

#[test]
fn route_source_with_no_sink_ports_sends_error() {
    let (mut router, rx) = make_router();
    router.on_source_added(42, Some("Spotify".into()), None, None);
    router.on_port_added(100, 42, "FL".into(), Direction::Output);

    router.route_source_test(42);

    let events: Vec<RouterEvent> = rx.try_iter().collect();
    let has_error = events
        .iter()
        .any(|e| matches!(e, RouterEvent::Error(RouterError::SinkPortsUnavailable)));
    assert!(
        has_error,
        "expected SinkPortsUnavailable error in {events:?}"
    );
}

#[test]
fn route_source_with_no_source_ports_sends_error() {
    let (mut router, rx) = make_router();
    router.update_sink_ports(vec![10, 11]);
    router.on_source_added(42, Some("Spotify".into()), None, None);

    router.route_source_test(42);

    let events: Vec<RouterEvent> = rx.try_iter().collect();
    let has_error = events.iter().any(|e| {
        matches!(
            e,
            RouterEvent::Error(RouterError::SourcePortsUnavailable { node_id: 42 })
        )
    });
    assert!(
        has_error,
        "expected SourcePortsUnavailable(42) in {events:?}"
    );
}

/// Integration test: requires a live PipeWire session.
/// Run with: cargo test --features pipewire-test route_integration
#[cfg(feature = "pipewire-test")]
#[test]
fn route_creates_visible_link_in_pipewire() {
    // Implementation deferred — requires PipeWire test harness setup (#27 scope).
    // This stub ensures the test infrastructure compiles and the feature gate works.
    let _is_integration_test = true;
    println!("SKIP: pipewire-test integration not yet implemented");
}
