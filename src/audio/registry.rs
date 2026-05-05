use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::error::AudioError;

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SOURCE_NODE_NAME: &str = "honkhonk-mic";

#[derive(Default)]
struct RegistryState {
    sink_node_id: Option<u32>,
    sink_input_ports: Vec<u32>,
    sink_output_ports: Vec<u32>,
    vsource_node_id: Option<u32>,
    vsource_input_ports: Vec<u32>,
    mic_node_id: Option<u32>,
    mic_output_ports: Vec<u32>,
    mic_links_created: bool,
    monitor_links_created: bool,
}

fn try_create_mic_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
) {
    if state.mic_links_created {
        return;
    }
    let mic_node = match state.mic_node_id {
        Some(id) => id,
        None => return,
    };
    let sink_node = match state.sink_node_id {
        Some(id) => id,
        None => return,
    };
    if state.mic_output_ports.is_empty() || state.sink_input_ports.is_empty() {
        return;
    }

    let mut all_ok = true;
    for (mic_port, sink_port) in state
        .mic_output_ports
        .iter()
        .zip(state.sink_input_ports.iter())
    {
        let link_props = pipewire::properties::properties! {
            "link.output.node" => mic_node.to_string(),
            "link.output.port" => mic_port.to_string(),
            "link.input.node" => sink_node.to_string(),
            "link.input.port" => sink_port.to_string(),
            "object.linger" => "false",
        };
        match core.create_object::<pipewire::link::Link>("link-factory", &link_props) {
            Ok(link) => links.push(link),
            Err(e) => {
                eprintln!("honkhonk: failed to create mic passthrough link: {e}");
                all_ok = false;
            }
        }
    }

    state.mic_links_created = all_ok;
}

fn try_create_monitor_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
) {
    if state.monitor_links_created {
        return;
    }
    let sink_node = match state.sink_node_id {
        Some(id) => id,
        None => return,
    };
    let vsource_node = match state.vsource_node_id {
        Some(id) => id,
        None => return,
    };
    if state.sink_output_ports.is_empty() || state.vsource_input_ports.is_empty() {
        return;
    }

    let mut all_ok = true;
    for (sink_out, vsource_in) in state
        .sink_output_ports
        .iter()
        .zip(state.vsource_input_ports.iter())
    {
        let link_props = pipewire::properties::properties! {
            "link.output.node" => sink_node.to_string(),
            "link.output.port" => sink_out.to_string(),
            "link.input.node" => vsource_node.to_string(),
            "link.input.port" => vsource_in.to_string(),
            "object.linger" => "false",
        };
        match core.create_object::<pipewire::link::Link>("link-factory", &link_props) {
            Ok(link) => links.push(link),
            Err(e) => {
                eprintln!("honkhonk: failed to create monitor→source link: {e}");
                all_ok = false;
            }
        }
    }

    state.monitor_links_created = all_ok;
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
            } else if class == "Audio/Source" && state.mic_node_id.is_none() {
                state.mic_node_id = Some(global.id);
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

pub struct RegistryGuard<'a> {
    _registry: pipewire::registry::RegistryBox<'a>,
    _listener: pipewire::registry::Listener,
    _links: Rc<RefCell<Vec<pipewire::link::Link>>>,
}

pub fn setup_registry_listener(
    core: &pipewire::core::CoreRc,
    shared_sink_id: Rc<Cell<Option<u32>>>,
) -> Result<RegistryGuard<'_>, AudioError> {
    let state = Rc::new(RefCell::new(RegistryState::default()));
    let all_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));

    let registry = core
        .get_registry()
        .map_err(|e| AudioError::PipeWireInit(format!("registry: {e}")))?;

    let state_ref = state.clone();
    let links_ref = all_links.clone();
    let core_ref = core.clone();
    let listener = registry
        .add_listener_local()
        .global(move |global| {
            let mut s = state_ref.borrow_mut();
            handle_registry_global(global, &mut s);
            if let Some(id) = s.sink_node_id {
                shared_sink_id.set(Some(id));
            }
            let mut link_store = links_ref.borrow_mut();
            try_create_mic_links(&mut s, &core_ref, &mut link_store);
            try_create_monitor_links(&mut s, &core_ref, &mut link_store);
        })
        .register();

    Ok(RegistryGuard {
        _registry: registry,
        _listener: listener,
        _links: all_links,
    })
}
