use super::*;
use std::collections::HashSet;

fn make_state_with_ports() -> RegistryState {
    let mut state = RegistryState {
        last_mic_rejection: None,
        mic_activated: None,
        mic_cooldown: None,
        graph: Rc::new(RefCell::new(RoutingGraph::default())),
        preferred_source_name: None,
        sink_node_id: Some(1),
        sink_input_ports: vec![10, 11],
        sink_output_ports: vec![],
        vsource_node_id: None,
        vsource_input_ports: vec![],
        mic_node_id: Some(2),
        mic_output_ports: vec![20, 21],
        linked_pairs: HashSet::new(),
        output_sinks: Vec::<(u32, String, String)>::new(),
        input_sources: Vec::<(u32, String, String)>::new(),
        source_ports: HashMap::new(),
    };
    // Simulate what try_create_mic_links would do (without PipeWire):
    // manually insert the pairs that would be created
    state.linked_pairs.insert((20, 10));
    state.linked_pairs.insert((21, 11));
    state
}

#[test]
fn linked_pairs_removal_uses_zip_not_cross_product() {
    // After disable, only the zipped pairs should be removed, not the full cross-product
    let mut state = make_state_with_ports();
    // Manually remove as apply_passthrough(false) would
    let pairs: Vec<(u32, u32)> = state
        .mic_output_ports
        .iter()
        .zip(state.sink_input_ports.iter())
        .map(|(&m, &k)| (m, k))
        .collect();
    for pair in pairs {
        state.linked_pairs.remove(&pair);
    }
    // All linked pairs should be gone
    assert!(state.linked_pairs.is_empty());
}

#[test]
fn linked_pairs_removal_clears_only_mic_sink_pairs() {
    // Monitor pairs (sink_output → vsource_input) should not be touched
    let mut state = make_state_with_ports();
    // Add a monitor pair that should not be removed
    state.linked_pairs.insert((30, 40)); // sink_out → vsource_in

    let pairs: Vec<(u32, u32)> = state
        .mic_output_ports
        .iter()
        .zip(state.sink_input_ports.iter())
        .map(|(&m, &k)| (m, k))
        .collect();
    for pair in pairs {
        state.linked_pairs.remove(&pair);
    }
    // Monitor pair should still be present
    assert!(state.linked_pairs.contains(&(30, 40)));
    // Mic pairs should be gone
    assert!(!state.linked_pairs.contains(&(20, 10)));
    assert!(!state.linked_pairs.contains(&(21, 11)));
}

#[test]
fn linked_pairs_empty_state_removal_does_not_panic() {
    let state = RegistryState {
        last_mic_rejection: None,
        mic_activated: None,
        mic_cooldown: None,
        graph: Rc::new(RefCell::new(RoutingGraph::default())),
        preferred_source_name: None,
        sink_node_id: None,
        sink_input_ports: vec![],
        sink_output_ports: vec![],
        vsource_node_id: None,
        vsource_input_ports: vec![],
        mic_node_id: None,
        mic_output_ports: vec![],
        linked_pairs: HashSet::new(),
        output_sinks: Vec::<(u32, String, String)>::new(),
        input_sources: Vec::<(u32, String, String)>::new(),
        source_ports: HashMap::new(),
    };
    // zip of empty vecs should produce no iterations — should not panic
    let pairs: Vec<(u32, u32)> = state
        .mic_output_ports
        .iter()
        .zip(state.sink_input_ports.iter())
        .map(|(&m, &k)| (m, k))
        .collect();
    assert!(pairs.is_empty());
}

#[test]
fn sanitize_preferred_source_drops_virtual_mic() {
    // The bootstrap self-reference: PipeWire's default.audio.source is our
    // own virtual mic. It must not survive as a preferred source.
    assert_eq!(
        sanitize_preferred_source(Some(SOURCE_NODE_NAME.to_string())),
        None
    );
}

#[test]
fn sanitize_preferred_source_keeps_real_device() {
    let real = Some("alsa_input.usb-OBSBOT_Meet_2".to_string());
    assert_eq!(sanitize_preferred_source(real.clone()), real);
}

#[test]
fn sanitize_preferred_source_passes_through_none() {
    assert_eq!(sanitize_preferred_source(None), None);
}

#[test]
fn select_mic_node_picks_first_real_when_no_preference() {
    assert!(select_mic_node(None, "alsa_input.usb-OBSBOT_Meet_2", false));
}

#[test]
fn select_mic_node_skips_further_sources_once_selected() {
    assert!(!select_mic_node(None, "alsa_input.second_mic", true));
}

#[test]
fn select_mic_node_honors_exact_preferred_even_if_already_selected() {
    assert!(select_mic_node(Some("micA"), "micA", true));
}

#[test]
fn select_mic_node_skips_non_preferred_when_preference_set() {
    assert!(!select_mic_node(Some("micA"), "micB", false));
}

#[test]
fn virtual_mic_as_system_default_still_selects_real_mic() {
    // Regression: mic passthrough was silent because honkhonk-mic registers
    // as PipeWire's default.audio.source, so the queried preferred source is
    // our own virtual mic. Sanitizing it to None must let the first real
    // Audio/Source be selected — otherwise mic_node_id stays None forever.
    let preferred = sanitize_preferred_source(Some(SOURCE_NODE_NAME.to_string()));
    assert_eq!(
        preferred, None,
        "virtual mic must not be a preferred source"
    );
    assert!(
        select_mic_node(preferred.as_deref(), "alsa_input.usb-OBSBOT_Meet_2", false),
        "a real mic must be selected when the only 'preference' was our own virtual mic"
    );
}

fn state_for_reselect(
    preferred: Option<&str>,
    sources: Vec<(u32, &str)>,
    ports: &[(u32, Vec<u32>)],
) -> RegistryState {
    RegistryState {
        last_mic_rejection: None,
        mic_activated: None,
        mic_cooldown: None,
        graph: Rc::new(RefCell::new(RoutingGraph::default())),
        preferred_source_name: preferred.map(String::from),
        sink_node_id: Some(1),
        sink_input_ports: vec![10, 11],
        sink_output_ports: vec![],
        vsource_node_id: None,
        vsource_input_ports: vec![],
        mic_node_id: None,
        mic_output_ports: vec![],
        linked_pairs: HashSet::new(),
        output_sinks: Vec::new(),
        input_sources: sources
            .into_iter()
            .map(|(id, n)| (id, n.to_string(), n.to_string()))
            .collect(),
        source_ports: ports.iter().cloned().collect(),
    }
}

#[test]
fn source_names_extracts_name_and_description() {
    let mut s = state_for_reselect(None, vec![(7, "alsa_input.usb-OBSBOT")], &[]);
    s.input_sources[0].2 = "OBSBOT Meet 2".to_string();
    assert_eq!(
        source_names(&s),
        vec![(
            "alsa_input.usb-OBSBOT".to_string(),
            "OBSBOT Meet 2".to_string()
        )]
    );
}

#[test]
fn reselect_mic_auto_picks_first_real_source() {
    let mut s = state_for_reselect(
        None,
        vec![(7, "alsa_input.first"), (8, "alsa_input.second")],
        &[(7, vec![70, 71]), (8, vec![80, 81])],
    );
    reselect_mic(&mut s);
    assert_eq!(s.mic_node_id, Some(7));
    assert_eq!(s.mic_output_ports, vec![70, 71]);
}

#[test]
fn reselect_mic_honors_explicit_preference() {
    let mut s = state_for_reselect(
        Some("alsa_input.second"),
        vec![(7, "alsa_input.first"), (8, "alsa_input.second")],
        &[(7, vec![70, 71]), (8, vec![80, 81])],
    );
    reselect_mic(&mut s);
    assert_eq!(s.mic_node_id, Some(8));
    assert_eq!(s.mic_output_ports, vec![80, 81]);
}

#[test]
fn reselect_mic_clears_selection_when_no_sources() {
    let mut s = state_for_reselect(None, vec![], &[]);
    s.mic_node_id = Some(99);
    s.mic_output_ports = vec![1, 2];
    reselect_mic(&mut s);
    assert_eq!(s.mic_node_id, None);
    assert!(s.mic_output_ports.is_empty());
}

#[test]
fn reselect_mic_absent_preference_selects_nothing() {
    // A device the user picked that isn't currently present yields no mic
    // (no silent fallback) until it reappears.
    let mut s = state_for_reselect(
        Some("alsa_input.unplugged"),
        vec![(7, "alsa_input.present")],
        &[(7, vec![70, 71])],
    );
    reselect_mic(&mut s);
    assert_eq!(s.mic_node_id, None);
}
