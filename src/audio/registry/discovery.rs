use super::*;

/// Decide whether a discovered real `Audio/Source` node should become the
/// selected microphone for passthrough.
///
/// An exact match against the user's `preferred` source always wins; otherwise
/// the first real source seen (`!mic_selected`) is taken as a fallback. The
/// virtual source is filtered out by the caller (`name != SOURCE_NODE_NAME`) and
/// `preferred` is expected to be sanitized via [`sanitize_preferred_source`], so
/// HonkHonk's own mic can never be chosen here.
pub(super) fn select_mic_node(
    preferred: Option<&str>,
    node_name: &str,
    mic_selected: bool,
) -> bool {
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
pub(super) fn sanitize_preferred_source(name: Option<String>) -> Option<String> {
    name.filter(|n| n != SOURCE_NODE_NAME)
}

/// Re-pick `mic_node_id` (and its cached output ports) from the currently known
/// `input_sources` under the current `preferred_source_name`. Used when the user
/// switches input devices at runtime. Selection follows [`select_mic_node`]: an
/// exact preferred match wins, otherwise the first real source.
pub(super) fn reselect_mic(state: &mut RegistryState) {
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
pub(super) enum DeviceChange {
    None,
    Outputs,
    Inputs,
}

pub(super) fn handle_registry_global(
    global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
    state: &mut RegistryState,
) -> DeviceChange {
    let Some(props) = global.props else {
        return DeviceChange::None;
    };
    match global.type_ {
        pipewire::types::ObjectType::Node => node(global.id, props, state),
        pipewire::types::ObjectType::Port => {
            port(global.id, props, state);
            DeviceChange::None
        }
        _ => DeviceChange::None,
    }
}

fn node(
    id: u32,
    props: &pipewire::spa::utils::dict::DictRef,
    state: &mut RegistryState,
) -> DeviceChange {
    let name = props.get("node.name").unwrap_or("");
    let class = props.get("media.class").unwrap_or("");
    if name == SINK_NODE_NAME {
        state.sink_node_id = Some(id);
    } else if name == SOURCE_NODE_NAME {
        state.vsource_node_id = Some(id);
    } else if class == "Audio/Source" {
        if select_mic_node(
            state.preferred_source_name.as_deref(),
            name,
            state.mic_node_id.is_some(),
        ) {
            state.mic_node_id = Some(id);
        }
        let description = props.get("node.description").unwrap_or(name);
        state
            .input_sources
            .push((id, name.into(), description.into()));
        return DeviceChange::Inputs;
    } else if class == "Audio/Sink" {
        let description = props.get("node.description").unwrap_or(name);
        state
            .output_sinks
            .push((id, name.into(), description.into()));
        return DeviceChange::Outputs;
    }
    DeviceChange::None
}

fn port(id: u32, props: &pipewire::spa::utils::dict::DictRef, state: &mut RegistryState) {
    let Some(node_id) = props.get("node.id").and_then(|v| v.parse().ok()) else {
        return;
    };
    let direction = props.get("port.direction").unwrap_or("");
    if Some(node_id) == state.sink_node_id {
        if direction == "in" {
            state.sink_input_ports.push(id);
        } else if direction == "out" {
            state.sink_output_ports.push(id);
        }
    } else if Some(node_id) == state.vsource_node_id && direction == "in" {
        state.vsource_input_ports.push(id);
    } else if Some(node_id) == state.mic_node_id && direction == "out" {
        state.mic_output_ports.push(id);
    }
    if direction == "out" && state.input_sources.iter().any(|(id, _, _)| *id == node_id) {
        state.source_ports.entry(node_id).or_default().push(id);
    }
}
