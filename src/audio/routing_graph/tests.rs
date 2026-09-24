use super::*;

fn graph() -> RoutingGraph {
    let mut graph = RoutingGraph::default();
    for (id, name, class) in [
        (1, "honkhonk-mix", "Audio/Sink/Virtual"),
        (2, "honkhonk-mic", "Audio/Source/Virtual"),
        (3, "browser", "Stream/Output/Audio"),
        (4, "capture", "Stream/Input/Audio"),
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
    graph.ports.insert(
        10,
        PortInfo {
            node: 1,
            input: true,
            monitor: false,
        },
    );
    graph.ports.insert(
        30,
        PortInfo {
            node: 3,
            input: false,
            monitor: false,
        },
    );
    graph
}

#[test]
fn permits_external_playback_and_real_microphones() {
    let mut graph = graph();
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
    graph.nodes.get_mut(&3).unwrap().class = "Audio/Source".into();
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
}

#[test]
fn rejects_self_by_name_or_application_and_disallowed_classes() {
    let mut graph = graph();
    graph.nodes.get_mut(&3).unwrap().name = "honkhonk-mix".into();
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::SelfRoute));
    graph.nodes.get_mut(&3).unwrap().name = "other".into();
    graph.nodes.get_mut(&3).unwrap().app = "HonkHonk".into();
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::SelfRoute));
    graph.nodes.get_mut(&3).unwrap().app.clear();
    for class in [
        "Audio/Sink",
        "Stream/Input/Audio",
        "",
        "Audio/Source/Virtual",
    ] {
        graph.nodes.get_mut(&3).unwrap().class = class.into();
        assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::MediaClass));
    }
}

#[test]
fn rejects_monitor_ports_wrong_endpoints_and_missing_metadata() {
    let mut graph = graph();
    graph.ports.get_mut(&30).unwrap().monitor = true;
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::Monitor));
    graph.ports.get_mut(&30).unwrap().monitor = false;
    graph.ports.get_mut(&10).unwrap().node = 4;
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::Unknown));
    assert_eq!(graph.check(3, &[(99, 10)]), Err(RouteRejection::Unknown));
    assert_eq!(graph.check(99, &[(30, 10)]), Err(RouteRejection::Unknown));
}

#[test]
fn rejects_direct_and_application_round_trip_cycles() {
    let mut graph = graph();
    graph.links.insert(50, (2, 3));
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::Cycle));
    graph.links.insert(50, (2, 4));
    graph.nodes.get_mut(&3).unwrap().process = Some(77);
    graph.nodes.get_mut(&4).unwrap().process = Some(77);
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::Cycle));
    graph.nodes.get_mut(&3).unwrap().process = None;
    graph.nodes.get_mut(&4).unwrap().process = None;
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
    graph.nodes.get_mut(&3).unwrap().client = Some(8);
    graph.nodes.get_mut(&4).unwrap().client = Some(8);
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::Cycle));
}

#[test]
fn graph_walk_terminates_on_unrelated_cycles() {
    let mut graph = graph();
    graph.links.insert(50, (2, 4));
    graph.links.insert(51, (4, 2));
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
}

fn set_group(graph: &mut RoutingGraph, node: u32, group: &str) {
    let props = pipewire::properties::properties! { "node.link-group" => group };
    graph.update_node(node, props.dict());
}

#[test]
fn rejects_implicit_link_group_path_without_shared_application_identity() {
    let mut graph = graph();
    graph.links.insert(50, (2, 4));
    // Coupled nodes need not have the capture/playback media classes.
    graph.nodes.get_mut(&4).unwrap().class = "Audio/Sink".into();
    set_group(&mut graph, 4, "loopback-123");
    set_group(&mut graph, 3, "loopback-123");
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::Cycle));
}

#[test]
fn unrelated_empty_and_absent_link_groups_do_not_create_paths() {
    let mut graph = graph();
    graph.links.insert(50, (2, 4));
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
    for (capture, playback) in [("one", "two"), ("", ""), ("one", "")] {
        set_group(&mut graph, 4, capture);
        set_group(&mut graph, 3, playback);
        assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
    }
}

#[test]
fn group_updates_and_node_removal_clear_implicit_paths() {
    let mut graph = graph();
    graph.links.insert(50, (2, 4));
    set_group(&mut graph, 4, "coupled");
    set_group(&mut graph, 3, "coupled");
    assert_eq!(graph.check(3, &[(30, 10)]), Err(RouteRejection::Cycle));
    set_group(&mut graph, 3, "different");
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
    set_group(&mut graph, 3, "coupled");
    set_group(&mut graph, 4, "");
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
    set_group(&mut graph, 4, "coupled");
    graph.remove(4);
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
    graph.nodes.insert(4, NodeInfo::default());
    graph.links.insert(50, (2, 4));
    assert_eq!(graph.check(3, &[(30, 10)]), Ok(()));
}
