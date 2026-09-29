//! PipeWire link router with persistent route intent (issue #27).
//!
//! Maintains stereo links from external app output ports to the HonkHonk
//! virtual sink input ports. Route intent is keyed by `AppIdentity` so
//! routes survive PipeWire stream destroy/recreate cycles.

use super::routing_graph::RoutingGraph;
use std::collections::HashMap;
use std::sync::mpsc;
use std::{cell::RefCell, rc::Rc};

use super::error::RouterError;
use super::streams::Direction;

// ── Public types ─────────────────────────────────────────────────────────────

/// Stable identity for an application across stream lifecycle events.
///
/// A stream destroyed and recreated by the same app gets a new PipeWire
/// `object.id` but the same `app_name`, `process_binary`, and `process_id`.
/// Matching by identity (not id) makes routes persistent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub app_name: Option<String>,
    pub process_binary: Option<String>,
    pub process_id: Option<u32>,
}

/// A persisted user routing preference.
#[derive(Debug, Clone)]
pub struct RouteIntent {
    pub identity: AppIdentity,
    pub enabled: bool,
}

/// Commands sent from the application layer to the Router.
#[derive(Debug, Clone)]
pub enum RouterCommand {
    /// Route the source node with the given PipeWire node ID to the virtual sink.
    RouteSource {
        source_node_id: u32,
    },
    /// Stop routing the source node with the given PipeWire node ID.
    UnrouteSource {
        source_node_id: u32,
    },
    /// Remove all active routes and all route intents.
    UnrouteAll,
    SetSourceLevel {
        source_node_id: u32,
        level: super::streams::SourceLevel,
    },
    SetSafeMode(bool),
    UndoRoutingChange,
}

/// Events emitted by the Router back to the application layer.
///
/// Does not implement `Clone` because `RouterError::LinkCreation` carries a
/// `pipewire::Error` source which is not `Clone`. Events are sent once via
/// `mpsc::Sender` and consumed by the drain thread.
#[derive(Debug)]
pub enum RouterEvent {
    RouteCreated { node_id: u32, identity: AppIdentity },
    RouteDestroyed { node_id: u32 },
    AutoReconnected { identity: AppIdentity, node_id: u32 },
    SourceDisconnected { identity: AppIdentity },
    Error(RouterError),
}

// ── Internal state ────────────────────────────────────────────────────────────

/// Per-node identity snapshot captured from `StreamEvent::SourceAdded`.
#[derive(Debug, Clone)]
pub(super) struct SourceInfo {
    identity: AppIdentity,
    /// Output port IDs (FL, FR) for this node in order received.
    output_ports: Vec<u32>,
}

/// Router state — all fields are owned by the PipeWire engine thread.
pub struct Router {
    /// User's routing preferences — persists across stream lifecycle events.
    pub(super) intents: Vec<RouteIntent>,
    /// Live PipeWire link objects keyed by source node ID.
    /// Dropping the Vec destroys the links.
    pub(super) active_links: HashMap<u32, Vec<pipewire::link::Link>>,
    /// Cached sink input ports (FL, FR) from the virtual sink.
    pub(super) sink_input_ports: Vec<u32>,
    /// Per-node identity + port info accumulated from `StreamEvent`s.
    pub(super) known_sources: HashMap<u32, SourceInfo>,
    /// Output ports seen before their owning source's `SourceAdded` arrived.
    ///
    /// PipeWire registry replay delivers an already-playing app's ports before
    /// the async node-`info` callback that produces `SourceAdded`, so ports can
    /// arrive while the node is still untracked. Buffered here keyed by node ID
    /// and drained into `known_sources` by `on_source_added`.
    pub(super) pending_ports: HashMap<u32, Vec<u32>>,
    /// Channel to send RouterEvents back to the app layer.
    evt_tx: mpsc::Sender<RouterEvent>,
    graph: Rc<RefCell<RoutingGraph>>,
    safety: safety::RoutingSafety,
}

mod identity;
mod safety;

// ── Router implementation ─────────────────────────────────────────────────────

impl Router {
    /// Create a new Router. Call `update_sink_ports` once the virtual sink's
    /// input port IDs are known (after registry enumeration completes).
    pub fn new(evt_tx: mpsc::Sender<RouterEvent>) -> Self {
        Self {
            intents: Vec::new(),
            active_links: HashMap::new(),
            sink_input_ports: Vec::new(),
            known_sources: HashMap::new(),
            pending_ports: HashMap::new(),
            evt_tx,
            graph: Rc::new(RefCell::new(RoutingGraph::default())),
            safety: safety::RoutingSafety::default(),
        }
    }

    pub(crate) fn use_graph(&mut self, graph: Rc<RefCell<RoutingGraph>>) {
        self.graph = graph;
    }

    pub(crate) fn recheck_routes(&mut self) {
        use super::routing_graph::RouteRejection;
        let invalid: Vec<_> = self
            .active_links
            .keys()
            .filter_map(|&id| {
                let reason = self
                    .port_pairs(id)
                    .map_err(|_| RouteRejection::Unknown)
                    .and_then(|pairs| self.graph.borrow().check(id, &pairs))
                    .err()?;
                Some((id, reason))
            })
            .collect();
        for (node_id, reason) in invalid {
            if reason == RouteRejection::Unknown {
                self.drop_route(node_id);
            } else {
                self.unroute(node_id);
                let _ = self
                    .evt_tx
                    .send(RouterEvent::Error(RouterError::UnsafeRoute {
                        node_id,
                        reason,
                    }));
            }
        }
    }

    fn drop_route(&mut self, node_id: u32) {
        self.safety.activated.remove(&node_id);
        if self.active_links.remove(&node_id).is_some() {
            let _ = self.evt_tx.send(RouterEvent::RouteDestroyed { node_id });
        }
    }

    /// Set the virtual sink input port IDs. Called reactively whenever the
    /// shared_sink_ports Rc is updated by the registry listener.
    pub fn update_sink_ports(&mut self, ports: Vec<u32>) {
        self.sink_input_ports = ports;
    }

    /// Handle a `StreamEvent::SourceAdded` from the stream watcher.
    pub fn on_source_added(
        &mut self,
        node_id: u32,
        app_name: Option<String>,
        process_binary: Option<String>,
        process_id: Option<u32>,
    ) {
        let identity = AppIdentity::from_stream(app_name, process_binary, process_id);
        // Reconcile any ports that arrived before this SourceAdded (registry
        // replay ordering for an already-playing app).
        let output_ports = self.pending_ports.remove(&node_id).unwrap_or_default();
        self.known_sources.insert(
            node_id,
            SourceInfo {
                identity,
                output_ports,
            },
        );
    }

    /// Handle a `StreamEvent::PortAdded` for a tracked source node.
    pub fn on_port_added(
        &mut self,
        port_id: u32,
        node_id: u32,
        _channel: String,
        direction: Direction,
    ) {
        if direction != Direction::Output {
            return;
        }
        if let Some(info) = self.known_sources.get_mut(&node_id) {
            info.output_ports.push(port_id);
        } else {
            // Source not tracked yet — buffer until its SourceAdded arrives.
            self.pending_ports.entry(node_id).or_default().push(port_id);
        }
    }

    pub(crate) fn on_port_removed(&mut self, port_id: u32) {
        for info in self.known_sources.values_mut() {
            info.output_ports.retain(|id| *id != port_id);
        }
        for ports in self.pending_ports.values_mut() {
            ports.retain(|id| *id != port_id);
        }
    }

    /// Handle a `StreamEvent::SourceRemoved`. Drops active links (destroying them
    /// in PipeWire), preserves intent for future reconnect, emits `SourceDisconnected`.
    pub fn on_source_removed(&mut self, node_id: u32) {
        self.drop_route(node_id);
        self.pending_ports.remove(&node_id); // discard un-reconciled buffered ports
        let identity = self
            .known_sources
            .remove(&node_id)
            .map(|info| info.identity);
        if let Some(identity) = identity {
            let _ = self
                .evt_tx
                .send(RouterEvent::SourceDisconnected { identity });
        }
    }

    /// Handle `RouterCommand::UnrouteAll` — clear every intent and every active link.
    pub fn handle_command_unroute_all(&mut self) {
        self.remember_change();
        let node_ids: Vec<u32> = self.active_links.keys().copied().collect();
        self.active_links.clear(); // drop all links
        self.safety.activated.clear();
        for node_id in node_ids {
            let _ = self.evt_tx.send(RouterEvent::RouteDestroyed { node_id });
        }
        self.intents.clear();
    }

    /// Handle `RouterCommand::UnrouteSource`. Removes active link (drops it),
    /// disables the intent (preserves for UX memory), emits `RouteDestroyed`.
    pub fn handle_command_unroute_source(&mut self, node_id: u32) {
        self.remember_change();
        self.unroute(node_id);
    }

    fn unroute(&mut self, node_id: u32) {
        self.safety.activated.remove(&node_id);
        self.active_links.remove(&node_id); // drop = PW link destruction
        if let Some(info) = self.known_sources.get(&node_id) {
            let identity = info.identity.clone();
            for intent in &mut self.intents {
                if intent.identity == identity || intent.identity.matches(&identity) {
                    intent.enabled = false;
                }
            }
        }
        let _ = self.evt_tx.send(RouterEvent::RouteDestroyed { node_id });
    }

    /// Create or update an intent for the given identity.
    fn upsert_intent(&mut self, identity: AppIdentity, enabled: bool) {
        if let Some(existing) = self.intents.iter_mut().find(|i| i.identity == identity) {
            existing.enabled = enabled;
        } else {
            self.intents.push(RouteIntent { identity, enabled });
        }
    }
}

#[cfg(test)]
mod identity_tests;
mod routing;
#[cfg(test)]
mod safety_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
