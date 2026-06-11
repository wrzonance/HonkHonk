use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
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
    /// Real microphone (`Audio/Source`) devices for the input picker:
    /// (node_id, node_name, display_name). Excludes HonkHonk's own virtual mic.
    input_sources: Vec<(u32, String, String)>,
    /// Output ports of each real source node, cached so a runtime device switch
    /// can re-link a mic whose ports were already enumerated.
    source_ports: HashMap<u32, Vec<u32>>,
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

/// Decide whether a discovered real `Audio/Source` node should become the
/// selected microphone for passthrough.
///
/// An exact match against the user's `preferred` source always wins; otherwise
/// the first real source seen (`!mic_selected`) is taken as a fallback. The
/// virtual source is filtered out by the caller (`name != SOURCE_NODE_NAME`) and
/// `preferred` is expected to be sanitized via [`sanitize_preferred_source`], so
/// HonkHonk's own mic can never be chosen here.
fn select_mic_node(preferred: Option<&str>, node_name: &str, mic_selected: bool) -> bool {
    match preferred {
        Some(pref) if pref == node_name => true,
        Some(_) => false,
        None => !mic_selected,
    }
}

/// Strip HonkHonk's own virtual source from the preferred-mic name.
///
/// When `honkhonk-mic` is PipeWire's `default.audio.source` (the bootstrap
/// self-reference: our persistent virtual mic becomes the system default),
/// `query_default_source_name` returns our own node name. Used as the preferred
/// source it would match no real device, so [`select_mic_node`] would never pick
/// a mic and passthrough would be permanently silent. Treat that value as "no
/// preference" so the first real source is chosen instead.
fn sanitize_preferred_source(name: Option<String>) -> Option<String> {
    name.filter(|n| n != SOURCE_NODE_NAME)
}

/// Re-pick `mic_node_id` (and its cached output ports) from the currently known
/// `input_sources` under the current `preferred_source_name`. Used when the user
/// switches input devices at runtime. Selection follows [`select_mic_node`]: an
/// exact preferred match wins, otherwise the first real source.
fn reselect_mic(state: &mut RegistryState) {
    state.mic_node_id = None;
    state.mic_output_ports.clear();
    let mut selected = false;
    let sources: Vec<(u32, String)> = state
        .input_sources
        .iter()
        .map(|(id, name, _)| (*id, name.clone()))
        .collect();
    for (id, name) in sources {
        if select_mic_node(state.preferred_source_name.as_deref(), &name, selected) {
            state.mic_node_id = Some(id);
            state.mic_output_ports = state.source_ports.get(&id).cloned().unwrap_or_default();
            selected = true;
        }
    }
}

/// Which device list (if any) changed when processing a registry global, so the
/// listener emits the matching `*DevicesChanged` event.
enum DeviceChange {
    None,
    Outputs,
    Inputs,
}

fn handle_registry_global(
    global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
    state: &mut RegistryState,
) -> DeviceChange {
    let props = match global.props {
        Some(p) => p,
        None => return DeviceChange::None,
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
                if select_mic_node(
                    state.preferred_source_name.as_deref(),
                    name,
                    state.mic_node_id.is_some(),
                ) {
                    state.mic_node_id = Some(global.id);
                }
                let description = props.get("node.description").unwrap_or(name);
                state
                    .input_sources
                    .push((global.id, name.to_owned(), description.to_owned()));
                return DeviceChange::Inputs;
            } else if class == "Audio/Sink" && name != SINK_NODE_NAME && name != SOURCE_NODE_NAME {
                let description = props.get("node.description").unwrap_or(name);
                state
                    .output_sinks
                    .push((global.id, name.to_owned(), description.to_owned()));
                return DeviceChange::Outputs;
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

            // Cache output ports of every real source (including the current mic)
            // so a runtime input-device switch can re-link without waiting for
            // ports that PipeWire already enumerated.
            if direction == "out" && state.input_sources.iter().any(|(id, _, _)| *id == node_id) {
                state
                    .source_ports
                    .entry(node_id)
                    .or_default()
                    .push(global.id);
            }
        }
        _ => {}
    }
    DeviceChange::None
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

    /// Switch the microphone (input) source at runtime. Mirrors the monitor
    /// device switch, but for the link-based mic path: tear down the current mic
    /// links, update the preferred source (sanitized so HonkHonk's own mic is
    /// never chosen), re-select a real source, and rebuild links if passthrough
    /// is enabled. `preferred_name` = `None` means Auto (first real source).
    pub fn set_input_device(&self, preferred_name: Option<String>) {
        let core = &self.core;
        let mut s = self.state.borrow_mut();
        let mut links = self.mic_links.borrow_mut();

        // Tear down current mic links: drop the link objects, then forget pairs.
        let pairs: Vec<(u32, u32)> = s
            .mic_output_ports
            .iter()
            .zip(s.sink_input_ports.iter())
            .map(|(&m, &k)| (m, k))
            .collect();
        links.clear();
        for pair in pairs {
            s.linked_pairs.remove(&pair);
        }

        s.preferred_source_name = sanitize_preferred_source(preferred_name);
        reselect_mic(&mut s);

        if self.mic_passthrough.get() {
            try_create_mic_links(&mut s, core, &mut links);
        }
    }
}

fn sink_names(state: &RegistryState) -> Vec<(String, String)> {
    state
        .output_sinks
        .iter()
        .map(|(_, n, d)| (n.clone(), d.clone()))
        .collect()
}

fn source_names(state: &RegistryState) -> Vec<(String, String)> {
    state
        .input_sources
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
        preferred_source_name: sanitize_preferred_source(default_source_name),
        sink_node_id: None,
        sink_input_ports: Vec::new(),
        sink_output_ports: Vec::new(),
        vsource_node_id: None,
        vsource_input_ports: Vec::new(),
        mic_node_id: None,
        mic_output_ports: Vec::new(),
        linked_pairs: HashSet::new(),
        output_sinks: Vec::new(),
        input_sources: Vec::new(),
        source_ports: HashMap::new(),
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
            let change = handle_registry_global(global, &mut s);
            if let Some(id) = s.sink_node_id {
                shared_sink_id.set(Some(id));
            }
            // Update shared_sink_ports whenever the registry sees sink port changes
            // (including empty) so the Router never holds stale port IDs.
            *shared_sink_ports.borrow_mut() = s.sink_input_ports.clone();
            if mic_passthrough_ref.get() {
                let mut ml = mic_links_ref.borrow_mut();
                try_create_mic_links(&mut s, &core_ref, &mut ml);
            }
            let mut ol = other_links_ref.borrow_mut();
            try_create_monitor_links(&mut s, &core_ref, &mut ol);
            match change {
                DeviceChange::Outputs => {
                    let sinks = sink_names(&s);
                    let _ = evt_tx.send(AudioEvent::OutputDevicesChanged(sinks));
                }
                DeviceChange::Inputs => {
                    let sources = source_names(&s);
                    let _ = evt_tx.send(AudioEvent::InputDevicesChanged(sources));
                }
                DeviceChange::None => {}
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
            let before_sources = s.input_sources.len();
            s.input_sources.retain(|(source_id, _, _)| *source_id != id);
            s.source_ports.remove(&id);
            if s.input_sources.len() != before_sources {
                let sources = source_names(&s);
                let _ = evt_tx_remove.send(AudioEvent::InputDevicesChanged(sources));
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
}
