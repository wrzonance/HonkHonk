use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use super::error::AudioError;

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SOURCE_NODE_NAME: &str = "honkhonk-mic";

struct RegistryState {
    preferred_source_name: Option<String>,
    sink_node_id: Option<u32>,
    sink_input_ports: Vec<u32>,
    sink_output_ports: Vec<u32>,
    vsource_node_id: Option<u32>,
    vsource_input_ports: Vec<u32>,
    mic_node_id: Option<u32>,
    mic_output_ports: Vec<u32>,
    linked_pairs: HashSet<(u32, u32)>,
}

fn try_create_mic_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
) {
    let mic_node = match state.mic_node_id {
        Some(id) => id,
        None => return,
    };
    let sink_node = match state.sink_node_id {
        Some(id) => id,
        None => return,
    };

    for (mic_port, sink_port) in state
        .mic_output_ports
        .iter()
        .zip(state.sink_input_ports.iter())
    {
        if state.linked_pairs.contains(&(*mic_port, *sink_port)) {
            continue;
        }
        let link_props = pipewire::properties::properties! {
            "link.output.node" => mic_node.to_string(),
            "link.output.port" => mic_port.to_string(),
            "link.input.node" => sink_node.to_string(),
            "link.input.port" => sink_port.to_string(),
            "object.linger" => "false",
        };
        match core.create_object::<pipewire::link::Link>("link-factory", &link_props) {
            Ok(link) => {
                state.linked_pairs.insert((*mic_port, *sink_port));
                links.push(link);
            }
            Err(e) => {
                eprintln!("honkhonk: failed to create mic passthrough link: {e}");
            }
        }
    }
}

fn try_create_monitor_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
) {
    let sink_node = match state.sink_node_id {
        Some(id) => id,
        None => return,
    };
    let vsource_node = match state.vsource_node_id {
        Some(id) => id,
        None => return,
    };

    for (sink_out, vsource_in) in state
        .sink_output_ports
        .iter()
        .zip(state.vsource_input_ports.iter())
    {
        if state.linked_pairs.contains(&(*sink_out, *vsource_in)) {
            continue;
        }
        let link_props = pipewire::properties::properties! {
            "link.output.node" => sink_node.to_string(),
            "link.output.port" => sink_out.to_string(),
            "link.input.node" => vsource_node.to_string(),
            "link.input.port" => vsource_in.to_string(),
            "object.linger" => "false",
        };
        match core.create_object::<pipewire::link::Link>("link-factory", &link_props) {
            Ok(link) => {
                state.linked_pairs.insert((*sink_out, *vsource_in));
                links.push(link);
            }
            Err(e) => {
                eprintln!("honkhonk: failed to create monitor→source link: {e}");
            }
        }
    }
}

fn handle_registry_global(
    global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
    state: &mut RegistryState,
) {
    let props = match global.props {
        Some(p) => p,
        None => return,
    };

    match global.type_ {
        pipewire::types::ObjectType::Node => {
            let name = props.get("node.name").unwrap_or("");
            let class = props.get("media.class").unwrap_or("");

            if name == SINK_NODE_NAME {
                state.sink_node_id = Some(global.id);
            } else if name == SOURCE_NODE_NAME {
                state.vsource_node_id = Some(global.id);
            } else if class == "Audio/Source" && name != SOURCE_NODE_NAME {
                match &state.preferred_source_name {
                    Some(pref) if pref == name => {
                        state.mic_node_id = Some(global.id);
                    }
                    Some(_) => {}
                    None if state.mic_node_id.is_none() => {
                        state.mic_node_id = Some(global.id);
                    }
                    None => {}
                }
            }
        }
        pipewire::types::ObjectType::Port => {
            let node_id: u32 = props
                .get("node.id")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let direction = props.get("port.direction").unwrap_or("");

            if Some(node_id) == state.sink_node_id {
                if direction == "in" {
                    state.sink_input_ports.push(global.id);
                } else if direction == "out" {
                    state.sink_output_ports.push(global.id);
                }
            } else if Some(node_id) == state.vsource_node_id && direction == "in" {
                state.vsource_input_ports.push(global.id);
            } else if Some(node_id) == state.mic_node_id && direction == "out" {
                state.mic_output_ports.push(global.id);
            }
        }
        _ => {}
    }
}

pub struct RegistryGuard {
    _registry: pipewire::registry::RegistryRc,
    _listener: pipewire::registry::Listener,
    _other_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
    mic_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
    state: Rc<RefCell<RegistryState>>,
    mic_passthrough: Rc<Cell<bool>>,
    core: pipewire::core::CoreRc,
}

impl RegistryGuard {
    pub fn apply_passthrough(&self, enabled: bool) {
        let core = &self.core;
        self.mic_passthrough.set(enabled);
        if enabled {
            let mut s = self.state.borrow_mut();
            let mut links = self.mic_links.borrow_mut();
            try_create_mic_links(&mut s, core, &mut links);
        } else {
            {
                let mut s = self.state.borrow_mut();
                let pairs: Vec<(u32, u32)> = s
                    .mic_output_ports
                    .iter()
                    .zip(s.sink_input_ports.iter())
                    .map(|(&m, &k)| (m, k))
                    .collect();
                for pair in pairs {
                    s.linked_pairs.remove(&pair);
                }
            }
            self.mic_links.borrow_mut().clear();
        }
    }
}

pub fn setup_registry_listener(
    core: &pipewire::core::CoreRc,
    shared_sink_id: Rc<Cell<Option<u32>>>,
    default_source_name: Option<String>,
    mic_passthrough: Rc<Cell<bool>>,
) -> Result<RegistryGuard, AudioError> {
    let state = Rc::new(RefCell::new(RegistryState {
        preferred_source_name: default_source_name,
        sink_node_id: None,
        sink_input_ports: Vec::new(),
        sink_output_ports: Vec::new(),
        vsource_node_id: None,
        vsource_input_ports: Vec::new(),
        mic_node_id: None,
        mic_output_ports: Vec::new(),
        linked_pairs: HashSet::new(),
    }));
    let mic_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));
    let other_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));

    let registry = core
        .get_registry_rc()
        .map_err(|e| AudioError::PipeWireInit(format!("registry: {e}")))?;

    let state_ref = state.clone();
    let mic_links_ref = mic_links.clone();
    let other_links_ref = other_links.clone();
    let mic_passthrough_ref = mic_passthrough.clone();
    let core_ref = core.clone();
    let listener = registry
        .add_listener_local()
        .global(move |global| {
            let mut s = state_ref.borrow_mut();
            handle_registry_global(global, &mut s);
            if let Some(id) = s.sink_node_id {
                shared_sink_id.set(Some(id));
            }
            if mic_passthrough_ref.get() {
                let mut ml = mic_links_ref.borrow_mut();
                try_create_mic_links(&mut s, &core_ref, &mut ml);
            }
            let mut ol = other_links_ref.borrow_mut();
            try_create_monitor_links(&mut s, &core_ref, &mut ol);
        })
        .register();

    Ok(RegistryGuard {
        _registry: registry,
        _listener: listener,
        _other_links: other_links,
        mic_links,
        state,
        mic_passthrough,
        core: core.clone(),
    })
}
