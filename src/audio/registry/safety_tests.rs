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
