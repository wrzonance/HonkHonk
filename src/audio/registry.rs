use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc;

use super::engine::AudioEvent;
use super::error::AudioError;
use super::routing_graph::RoutingGraph;
mod discovery;
mod feedback;
mod lifecycle;
mod listener;
pub use listener::setup as setup_registry_listener;
mod links;
use discovery::*;
use links::*;

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SOURCE_NODE_NAME: &str = "honkhonk-mic";

#[derive(Default)]
struct RegistryState {
    last_mic_rejection: Option<(u32, super::RouteRejection)>,
    mic_activated: Option<std::time::Instant>,
    mic_cooldown: Option<std::time::Instant>,
    graph: Rc<RefCell<RoutingGraph>>,
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

pub struct RegistryGuard {
    _registry: pipewire::registry::RegistryRc,
    _listener: pipewire::registry::Listener,
    _other_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
    mic_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
    state: Rc<RefCell<RegistryState>>,
    mic_passthrough: Rc<Cell<bool>>,
    core: pipewire::core::CoreRc,
    evt_tx: mpsc::Sender<AudioEvent>,
}

impl RegistryGuard {
    pub fn recheck_routes(&self) {
        if self.mic_passthrough.get() {
            try_create_mic_links(
                &mut self.state.borrow_mut(),
                &self.core,
                &mut self.mic_links.borrow_mut(),
                &self.evt_tx,
            );
        }
    }

    pub fn apply_passthrough(&self, enabled: bool) {
        if enabled && self.cooling_down() {
            return;
        }
        let core = &self.core;
        self.mic_passthrough.set(enabled);
        if enabled {
            let mut s = self.state.borrow_mut();
            let mut links = self.mic_links.borrow_mut();
            try_create_mic_links(&mut s, core, &mut links, &self.evt_tx);
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
            try_create_mic_links(&mut s, core, &mut links, &self.evt_tx);
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
    pub graph: Rc<RefCell<RoutingGraph>>,
    pub shared_sink_id: Rc<Cell<Option<u32>>>,
    pub default_source_name: Option<String>,
    pub mic_passthrough: Rc<Cell<bool>>,
    pub evt_tx: mpsc::Sender<AudioEvent>,
    /// Updated by the registry whenever the virtual sink's input ports are seen,
    /// so the Router can read them reactively on `SourceAdded` events.
    pub shared_sink_ports: Rc<RefCell<Vec<u32>>>,
}

#[cfg(test)]
mod safety_tests;
#[cfg(test)]
mod tests;
