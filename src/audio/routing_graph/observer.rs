//! Observe all graph objects, including capture streams omitted by the app list.
use super::{PortInfo, RoutingGraph};
use crate::audio::AudioError;
use pipewire::{registry::GlobalObject, spa::utils::dict::DictRef, types::ObjectType};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

type NodeWatch = (pipewire::node::Node, pipewire::node::NodeListener);

pub(crate) struct GraphWatcher {
    _registry: pipewire::registry::RegistryRc,
    _listener: pipewire::registry::Listener,
    _nodes: Rc<RefCell<HashMap<u32, NodeWatch>>>,
}

impl GraphWatcher {
    pub fn start(
        core: &pipewire::core::CoreRc,
        graph: Rc<RefCell<RoutingGraph>>,
    ) -> Result<Self, AudioError> {
        let registry = core
            .get_registry_rc()
            .map_err(|e| AudioError::PipeWireInit(format!("feedback graph registry: {e}")))?;
        let nodes = Rc::new(RefCell::new(HashMap::new()));
        let reg = registry.clone();
        let tracked = nodes.clone();
        let removed = nodes.clone();
        let remove_graph = graph.clone();
        let listener = registry
            .add_listener_local()
            .global(move |global| {
                graph.borrow_mut().observe(global);
                if global.type_ == ObjectType::Node {
                    bind_node(&reg, global, &graph, &tracked);
                }
            })
            .global_remove(move |id| {
                remove_graph.borrow_mut().remove(id);
                removed.borrow_mut().remove(&id);
            })
            .register();
        Ok(Self {
            _registry: registry,
            _listener: listener,
            _nodes: nodes,
        })
    }
}

fn bind_node(
    registry: &pipewire::registry::RegistryRc,
    global: &GlobalObject<&DictRef>,
    graph: &Rc<RefCell<RoutingGraph>>,
    nodes: &Rc<RefCell<HashMap<u32, NodeWatch>>>,
) {
    let node: pipewire::node::Node = match registry.bind(global) {
        Ok(node) => node,
        Err(error) => {
            tracing::warn!(node = global.id, %error, "cannot observe routing identity");
            return;
        }
    };
    let id = global.id;
    let graph = graph.clone();
    let listener = node
        .add_listener_local()
        .info(move |info| {
            if let Some(props) = info.props() {
                graph.borrow_mut().update_node(id, props);
            }
        })
        .register();
    nodes.borrow_mut().insert(id, (node, listener));
}

impl RoutingGraph {
    pub(super) fn update_node(&mut self, id: u32, props: &DictRef) {
        let node = self.nodes.entry(id).or_default();
        for (key, value) in [
            ("node.name", &mut node.name),
            ("media.class", &mut node.class),
            ("application.name", &mut node.app),
            ("node.link-group", &mut node.link_group),
        ] {
            if let Some(text) = props.get(key) {
                *value = text.into();
            }
        }
        if let Some(pid) = props.get("application.process.id") {
            node.process = pid.parse().ok();
        }
        if let Some(client) = props.get("client.id") {
            node.client = client.parse().ok();
        }
    }

    fn observe(&mut self, global: &GlobalObject<&DictRef>) {
        let Some(props) = global.props else {
            return;
        };
        let id = |key| props.get(key).and_then(|v| v.parse::<u32>().ok());
        match global.type_ {
            ObjectType::Node => self.update_node(global.id, props),
            ObjectType::Port => {
                if let Some(node) = id("node.id") {
                    let name = props.get("port.name").unwrap_or("");
                    self.ports.insert(
                        global.id,
                        PortInfo {
                            node,
                            input: props.get("port.direction") == Some("in"),
                            monitor: props.get("port.monitor") == Some("true")
                                || name
                                    .split(['.', ':'])
                                    .any(|part| part.starts_with("monitor_")),
                        },
                    );
                }
            }
            ObjectType::Link => {
                if let (Some(from), Some(to)) = (id("link.output.node"), id("link.input.node")) {
                    self.links.insert(global.id, (from, to));
                }
            }
            _ => {}
        }
    }
}
