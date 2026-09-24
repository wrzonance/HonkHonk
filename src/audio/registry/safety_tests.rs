use super::*;
use crate::audio::RouteRejection;

fn state() -> RegistryState {
    RegistryState {
        mic_node_id: Some(2),
        sink_node_id: Some(1),
        mic_output_ports: vec![20, 21],
        sink_input_ports: vec![10, 11],
        input_sources: vec![(2, "mic".into(), "Microphone".into())],
        source_ports: HashMap::from([(2, vec![20, 21])]),
        linked_pairs: HashSet::from([(20, 10), (21, 11)]),
        ..Default::default()
    }
}

fn routable_state() -> RegistryState {
    use crate::audio::routing_graph::{NodeInfo, PortInfo};
    let state = state();
    let mut graph = state.graph.borrow_mut();
    for (id, name, class) in [
        (1, "honkhonk-mix", "Audio/Sink"),
        (2, "mic", "Audio/Source"),
    ] {
        graph.nodes.insert(
            id,
            NodeInfo {
                name: name.into(),
                class: class.into(),
                ..Default::default()
            },
        );
    }
    for (id, node, input) in [(10, 1, true), (11, 1, true), (20, 2, false), (21, 2, false)] {
        graph.ports.insert(
            id,
            PortInfo {
                node,
                input,
                monitor: false,
            },
        );
    }
    drop(graph);
    state
}

#[test]
fn microphone_enable_during_cooldown_retains_intent_for_legal_retry() {
    let now = std::time::Instant::now();
    let until = now + std::time::Duration::from_secs(2);
    let mut state = routable_state();
    state.mic_cooldown = Some(until);
    let intent = Cell::new(false);
    assert!(!request_passthrough(&intent, true, || true));
    assert!(intent.get());
    assert_eq!(mic_pairs(&state, now), Err(RouteRejection::Cooldown));
    assert!(intent.get());
    assert_eq!(mic_pairs(&state, until), Ok(vec![(20, 10), (21, 11)]));
}

#[test]
fn removed_mic_ports_do_not_poison_recreated_routing() {
    let mut state = state();
    state.remove_global(20);
    assert_eq!(state.mic_output_ports, [21]);
    assert_eq!(state.source_ports[&2], [21]);
    assert!(!state.linked_pairs.contains(&(20, 10)));
    state.remove_global(2);
    assert!(state.mic_node_id.is_none());
    assert!(state.mic_output_ports.is_empty());
    assert!(state.input_sources.is_empty());
}

#[test]
fn removed_sink_clears_stale_identifiers() {
    let mut state = state();
    state.remove_global(1);
    assert!(state.sink_node_id.is_none());
    assert!(state.sink_input_ports.is_empty());
    assert!(state.linked_pairs.is_empty());
}

#[test]
fn unsafe_mic_rejection_is_reported_once_but_transient_metadata_is_quiet() {
    let mut state = state();
    let (tx, rx) = mpsc::channel();
    state.report_mic_rejection(RouteRejection::Unknown, &tx);
    assert!(rx.try_recv().is_err());
    for _ in 0..2 {
        state.report_mic_rejection(RouteRejection::Cycle, &tx);
    }
    assert!(matches!(
        rx.try_recv(),
        Ok(AudioEvent::RouteRejected {
            node_id: 2,
            reason: RouteRejection::Cycle
        })
    ));
    assert!(rx.try_recv().is_err());
}
