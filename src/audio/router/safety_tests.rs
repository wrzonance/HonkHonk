use super::*;
use crate::audio::routing_graph::{NodeInfo, PortInfo};

fn router() -> Router {
    let (tx, _) = mpsc::channel();
    let mut router = Router::new(tx);
    router.set_safe_mode(false);
    router.on_source_added(3, Some("Browser".into()), None, Some(33));
    router.update_sink_ports(vec![10, 11]);
    router.on_port_added(30, 3, "FL".into(), Direction::Output);
    router.on_port_added(31, 3, "FR".into(), Direction::Output);
    {
        let mut graph = router.graph.borrow_mut();
        graph.nodes.insert(
            1,
            NodeInfo {
                name: "honkhonk-mix".into(),
                ..Default::default()
            },
        );
        graph.nodes.insert(
            3,
            NodeInfo {
                class: "Stream/Output/Audio".into(),
                ..Default::default()
            },
        );
        for (id, node, input) in [(10, 1, true), (11, 1, true), (30, 3, false), (31, 3, false)] {
            graph.ports.insert(
                id,
                PortInfo {
                    node,
                    input,
                    monitor: false,
                },
            );
        }
    }
    router
}

#[test]
fn manual_and_automatic_routes_check_graph_before_calling_link_factory() {
    let mut router = router();
    router
        .graph
        .borrow_mut()
        .ports
        .get_mut(&30)
        .unwrap()
        .monitor = true;
    for automatic in [false, true] {
        if automatic {
            router.intents.push(RouteIntent {
                identity: router.known_sources[&3].identity.clone(),
                enabled: true,
            });
        }
        let mut called = false;
        router.route_with(3, automatic, |_| {
            called = true;
            Ok(vec![])
        });
        assert!(!called, "dangerous route reached PipeWire link factory");
        assert!(!router.active_links.contains_key(&3));
    }
}

#[test]
fn failed_link_creation_does_not_enable_intent() {
    let mut router = router();
    router.route_with(3, false, |_| Err(RouterError::SinkPortsUnavailable));
    assert!(router.intents.is_empty());
}

#[test]
fn safe_mode_defaults_on_and_disabling_routes_when_reenabled() {
    let (tx, _) = mpsc::channel();
    assert!(Router::new(tx).safety.safe_mode);
    let mut router = router();
    router.set_safe_mode(true);
    assert_eq!(
        router.safety_check(3, std::time::Instant::now()),
        Err(crate::audio::RouteRejection::SafeMode)
    );
    router.set_safe_mode(false);
    router.route_with(3, false, |_| Ok(vec![]));
    router.set_safe_mode(true);
    assert!(router.active_links.is_empty());
}

#[test]
fn feedback_disables_intent_and_blocks_recreated_identity_for_two_seconds() {
    use std::time::{Duration, Instant};
    let mut router = router();
    router.route_with(3, false, |_| Ok(vec![]));
    let now = Instant::now();
    let source = router.trip_feedback(3, now).expect("suspected source");
    assert_eq!(source.node_id, 3);
    assert!(!router.intents[0].enabled);
    assert!(!router.active_links.contains_key(&3));
    router.on_source_added(9, Some("Browser".into()), None, Some(99));
    assert_eq!(
        router.safety_check(9, now + Duration::from_millis(1999)),
        Err(crate::audio::RouteRejection::Cooldown)
    );
    assert_eq!(router.safety_check(9, now + Duration::from_secs(2)), Ok(()));
}

#[test]
fn undo_records_only_last_three_user_changes() {
    let mut router = router();
    for _ in 0..3 {
        router.route_with(3, false, |_| Ok(vec![]));
        router.handle_command_unroute_source(3);
    }
    assert_eq!(router.safety.undo.len(), 3);
    router.restore_last_change();
    assert!(router.intents[0].enabled);
    assert!(
        router.active_links.is_empty(),
        "undo must recreate links through safety checks"
    );
}

#[test]
fn graph_disappearance_drops_links_but_preserves_reconnect_intent() {
    let mut router = router();
    router.route_with(3, false, |_| Ok(vec![]));
    router.graph.borrow_mut().remove(3);
    router.recheck_routes();
    assert!(router.active_links.is_empty());
    assert!(router.intents[0].enabled);
    router.on_source_removed(3);
    assert!(router.intents[0].enabled);
}

#[test]
fn anonymous_source_feedback_still_blocks_same_node() {
    let mut router = router();
    router.known_sources.get_mut(&3).unwrap().identity = AppIdentity::from_stream(None, None, None);
    router.route_with(3, false, |_| Ok(vec![]));
    let now = std::time::Instant::now();
    router.trip_feedback(3, now).expect("anonymous source");
    assert_eq!(
        router.safety_check(3, now),
        Err(crate::audio::RouteRejection::Cooldown)
    );
    assert!(!router.intents[0].enabled);
}

#[test]
fn confirmed_cycle_disables_intent_and_reports_reason() {
    let mut router = router();
    let (tx, rx) = mpsc::channel();
    router.evt_tx = tx;
    router.route_with(3, false, |_| Ok(vec![]));
    router.graph.borrow_mut().links.insert(70, (1, 3));
    router.recheck_routes();
    assert!(!router.intents[0].enabled);
    assert!(rx.try_iter().any(|e| matches!(
        e,
        RouterEvent::Error(RouterError::UnsafeRoute {
            node_id: 3,
            reason: crate::audio::RouteRejection::Cycle,
        })
    )));
}

#[test]
fn volume_control_requires_active_safe_route() {
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut router = Router::new(tx);
    assert!(router.check_source_control(7).is_err());
    router.on_source_added(7, Some("Browser".into()), None, Some(123));
    router.update_sink_ports(vec![20, 22]);
    router.on_port_added(21, 7, "FL".into(), Direction::Output);
    router.on_port_added(23, 7, "FR".into(), Direction::Output);
    assert!(router.check_source_control(7).is_err());
    router.route_source_test(7);
    assert!(router.check_source_control(7).is_ok());
    router.set_safe_mode(true);
    assert!(router.check_source_control(7).is_err());
}

#[test]
fn all_route_teardowns_emit_the_volume_restoration_event() {
    for teardown in 0..4 {
        let mut router = router();
        let (tx, rx) = mpsc::channel();
        router.evt_tx = tx;
        router.route_source_test(3);
        let _ = rx.try_iter().collect::<Vec<_>>();
        match teardown {
            0 => router.handle_command_unroute_source(3),
            1 => router.set_safe_mode(true),
            2 => {
                router.trip_feedback(3, std::time::Instant::now());
            }
            _ => router.restore_last_change(),
        }
        assert!(!router.active_links.contains_key(&3));
        assert!(
            rx.try_iter()
                .any(|event| matches!(event, RouterEvent::RouteDestroyed { node_id: 3 }))
        );
    }
}
