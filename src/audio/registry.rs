use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::mpsc;

use super::engine::AudioEvent;
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
    output_sinks: Vec<(u32, String, String)>,
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
) -> bool {
    let props = match global.props {
        Some(p) => p,
        None => return false,
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
            } else if class == "Audio/Sink" && name != SINK_NODE_NAME && name != SOURCE_NODE_NAME {
                let description = props.get("node.description").unwrap_or(name);
                state
                    .output_sinks
                    .push((global.id, name.to_owned(), description.to_owned()));
                return true;
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
    false
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
            let mut s = self.state.borrow_mut();
            let mut links = self.mic_links.borrow_mut();
            let pairs: Vec<(u32, u32)> = s
                .mic_output_ports
                .iter()
                .zip(s.sink_input_ports.iter())
                .map(|(&m, &k)| (m, k))
                .collect();
            links.clear(); // drop PipeWire link objects first
            for pair in pairs {
                s.linked_pairs.remove(&pair);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make_state_with_ports() -> RegistryState {
        let mut state = RegistryState {
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
}

fn sink_names(state: &RegistryState) -> Vec<(String, String)> {
    state
        .output_sinks
        .iter()
        .map(|(_, n, d)| (n.clone(), d.clone()))
        .collect()
}

/// Configuration bundle for `setup_registry_listener`.
///
/// Bundles the arguments that exceed the `too-many-arguments-threshold = 5`
/// clippy lint threshold so the function signature stays within limits.
pub struct RegistryConfig {
    pub shared_sink_id: Rc<Cell<Option<u32>>>,
    pub default_source_name: Option<String>,
    pub mic_passthrough: Rc<Cell<bool>>,
    pub evt_tx: mpsc::Sender<AudioEvent>,
    /// Updated by the registry whenever the virtual sink's input ports are seen,
    /// so the Router can read them reactively on `SourceAdded` events.
    pub shared_sink_ports: Rc<RefCell<Vec<u32>>>,
}

pub fn setup_registry_listener(
    core: &pipewire::core::CoreRc,
    cfg: RegistryConfig,
) -> Result<RegistryGuard, AudioError> {
    let RegistryConfig {
        shared_sink_id,
        default_source_name,
        mic_passthrough,
        evt_tx,
        shared_sink_ports,
    } = cfg;
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
        output_sinks: Vec::new(),
    }));
    let mic_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));
    let other_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));

    let registry = core
        .get_registry_rc()
        .map_err(|e| AudioError::PipeWireInit(format!("registry: {e}")))?;

    let state_ref = state.clone();
    let state_remove_ref = state.clone();
    let mic_links_ref = mic_links.clone();
    let other_links_ref = other_links.clone();
    let mic_passthrough_ref = mic_passthrough.clone();
    let core_ref = core.clone();
    let evt_tx_remove = evt_tx.clone();
    let listener = registry
        .add_listener_local()
        .global(move |global| {
            let mut s = state_ref.borrow_mut();
            let sinks_changed = handle_registry_global(global, &mut s);
            if let Some(id) = s.sink_node_id {
                shared_sink_id.set(Some(id));
            }
            // Update shared_sink_ports whenever the registry sees new sink ports,
            // so the Router can read them reactively on SourceAdded events.
            if !s.sink_input_ports.is_empty() {
                *shared_sink_ports.borrow_mut() = s.sink_input_ports.clone();
            }
            if mic_passthrough_ref.get() {
                let mut ml = mic_links_ref.borrow_mut();
                try_create_mic_links(&mut s, &core_ref, &mut ml);
            }
            let mut ol = other_links_ref.borrow_mut();
            try_create_monitor_links(&mut s, &core_ref, &mut ol);
            if sinks_changed {
                let sinks = sink_names(&s);
                let _ = evt_tx.send(AudioEvent::OutputDevicesChanged(sinks));
            }
        })
        .global_remove(move |id| {
            let mut s = state_remove_ref.borrow_mut();
            let before = s.output_sinks.len();
            s.output_sinks.retain(|(sink_id, _, _)| *sink_id != id);
            if s.output_sinks.len() != before {
                let sinks = sink_names(&s);
                let _ = evt_tx_remove.send(AudioEvent::OutputDevicesChanged(sinks));
            }
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
