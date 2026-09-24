use super::*;
use crate::audio::RouteRejection;

pub(super) fn drop_mic_links(state: &mut RegistryState, links: &mut Vec<pipewire::link::Link>) {
    links.clear();
    for pair in state
        .mic_output_ports
        .iter()
        .copied()
        .zip(state.sink_input_ports.iter().copied())
    {
        state.linked_pairs.remove(&pair);
    }
    state.mic_activated = None;
}

pub(super) fn mic_pairs(
    state: &RegistryState,
    now: std::time::Instant,
) -> Result<Vec<(u32, u32)>, RouteRejection> {
    let node = state.mic_node_id.ok_or(RouteRejection::Unknown)?;
    if state.mic_cooldown.is_some_and(|until| until > now) {
        return Err(RouteRejection::Cooldown);
    }
    let pairs: Vec<_> = state
        .mic_output_ports
        .iter()
        .copied()
        .zip(state.sink_input_ports.iter().copied())
        .collect();
    state.graph.borrow().check(node, &pairs)?;
    Ok(pairs)
}

pub(super) fn try_create_mic_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
    events: &mpsc::Sender<AudioEvent>,
) {
    let pairs = match mic_pairs(state, std::time::Instant::now()) {
        Ok(pairs) => pairs,
        Err(reason) => {
            drop_mic_links(state, links);
            state.report_mic_rejection(reason, events);
            return;
        }
    };
    state.last_mic_rejection = None;
    for (from, to) in pairs {
        if state.linked_pairs.contains(&(from, to)) {
            continue;
        }
        match create_link(core, from, to) {
            Ok(link) => {
                state.linked_pairs.insert((from, to));
                if links.is_empty() {
                    state.mic_activated = Some(std::time::Instant::now());
                }
                links.push(link);
            }
            Err(error) => {
                let _ = events.send(AudioEvent::Error(super::super::EngineErrorEvent::Routing {
                    detail: format!("create mic passthrough link {from} -> {to}: {error}"),
                }));
            }
        }
    }
}

pub(super) fn try_create_monitor_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
) {
    if state.sink_node_id.is_none() || state.vsource_node_id.is_none() {
        return;
    }
    let pairs: Vec<_> = state
        .sink_output_ports
        .iter()
        .copied()
        .zip(state.vsource_input_ports.iter().copied())
        .filter(|pair| !state.linked_pairs.contains(pair))
        .collect();
    for (from, to) in pairs {
        add_monitor_link(state, core, links, (from, to));
    }
}

fn add_monitor_link(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
    pair: (u32, u32),
) {
    match create_link(core, pair.0, pair.1) {
        Ok(link) => {
            state.linked_pairs.insert(pair);
            links.push(link);
        }
        Err(error) => tracing::warn!(%error, "failed to create monitor->source link"),
    }
}

fn create_link(
    core: &pipewire::core::Core,
    from: u32,
    to: u32,
) -> Result<pipewire::link::Link, pipewire::Error> {
    core.create_object(
        "link-factory",
        &pipewire::properties::properties! {
            "link.output.port" => from.to_string(),
            "link.input.port" => to.to_string(),
            "object.linger" => "false",
        },
    )
}
