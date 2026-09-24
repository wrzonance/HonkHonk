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
