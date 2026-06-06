//! PipeWire link router with persistent route intent (issue #27).
//!
//! Maintains stereo links from external app output ports to the HonkHonk
//! virtual sink input ports. Route intent is keyed by `AppIdentity` so
//! routes survive PipeWire stream destroy/recreate cycles.

use std::collections::HashMap;
use std::sync::mpsc;

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
    RouteSource { source_node_id: u32 },
    /// Stop routing the source node with the given PipeWire node ID.
    UnrouteSource { source_node_id: u32 },
    /// Remove all active routes and all route intents.
    UnrouteAll,
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
}

// ── Identity matching ─────────────────────────────────────────────────────────

impl AppIdentity {
    /// Returns true if `self` (the intent identity) matches `candidate` (a live source).
    ///
    /// Match rules (in priority order):
    /// 1. If both have `app_name`, match on `app_name`.
    /// 2. If either `app_name` is None, fall back to `process_binary`.
    /// 3. If `self.process_id` is Some, also require PID match.
    pub fn matches(&self, candidate: &AppIdentity) -> bool {
        let name_matches = match (&self.app_name, &candidate.app_name) {
            (Some(a), Some(b)) => a == b,
            _ => match (&self.process_binary, &candidate.process_binary) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
        };
        if !name_matches {
            return false;
        }
        match self.process_id {
            Some(pid) => candidate.process_id == Some(pid),
            None => true,
        }
    }

    /// Build an AppIdentity from raw stream event fields.
    pub fn from_stream(
        app_name: Option<String>,
        process_binary: Option<String>,
        process_id: Option<u32>,
    ) -> Self {
        Self {
            app_name,
            process_binary,
            process_id,
        }
    }
}

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

    /// Handle a `StreamEvent::SourceRemoved`. Drops active links (destroying them
    /// in PipeWire), preserves intent for future reconnect, emits `SourceDisconnected`.
    pub fn on_source_removed(&mut self, node_id: u32) {
        self.active_links.remove(&node_id); // drop = PW link destruction
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
        let node_ids: Vec<u32> = self.active_links.keys().copied().collect();
        self.active_links.clear(); // drop all links
        for node_id in node_ids {
            let _ = self.evt_tx.send(RouterEvent::RouteDestroyed { node_id });
        }
        self.intents.clear();
    }

    /// Handle `RouterCommand::UnrouteSource`. Removes active link (drops it),
    /// disables the intent (preserves for UX memory), emits `RouteDestroyed`.
    pub fn handle_command_unroute_source(&mut self, node_id: u32) {
        self.active_links.remove(&node_id); // drop = PW link destruction
        if let Some(info) = self.known_sources.get(&node_id) {
            let identity = info.identity.clone();
            for intent in &mut self.intents {
                if intent.identity.matches(&identity) {
                    intent.enabled = false;
                }
            }
        }
        let _ = self.evt_tx.send(RouterEvent::RouteDestroyed { node_id });
    }

    /// Check if a newly-added source matches any enabled intent and auto-link.
    /// Called after `on_source_added` + port accumulation is complete (i.e.
    /// on each `PortAdded` event for the node — succeeds once enough ports exist).
    pub fn try_auto_reconnect(&mut self, node_id: u32, core: &pipewire::core::CoreRc) {
        let identity = match self.known_sources.get(&node_id).map(|i| i.identity.clone()) {
            Some(id) => id,
            None => return,
        };
        let should_route = self
            .intents
            .iter()
            .any(|intent| intent.enabled && intent.identity.matches(&identity));
        if !should_route {
            return;
        }
        // Only attempt once both stereo ports (FL + FR) are available.
        // Attempting with < 2 ports would create a partial mono link on first
        // PortAdded, then the already-linked guard (below) blocks the FR link.
        if self.sink_input_ports.len() < 2
            || self
                .known_sources
                .get(&node_id)
                .map_or(0, |i| i.output_ports.len())
                < 2
        {
            return;
        }
        // Skip if already linked
        if self.active_links.contains_key(&node_id) {
            return;
        }
        match self.create_stereo_links(node_id, core) {
            Ok(links) => {
                self.active_links.insert(node_id, links);
                let _ = self
                    .evt_tx
                    .send(RouterEvent::AutoReconnected { identity, node_id });
            }
            Err(e) => {
                let _ = self.evt_tx.send(RouterEvent::Error(e));
            }
        }
    }

    /// Route a source node by ID. Adds/updates intent, creates stereo links.
    pub fn route_source(&mut self, node_id: u32, core: &pipewire::core::CoreRc) {
        let identity = match self.known_sources.get(&node_id).map(|i| i.identity.clone()) {
            Some(id) => id,
            None => {
                let _ = self
                    .evt_tx
                    .send(RouterEvent::Error(RouterError::SourcePortsUnavailable {
                        node_id,
                    }));
                return;
            }
        };
        self.upsert_intent(identity.clone(), true);
        match self.create_stereo_links(node_id, core) {
            Ok(links) => {
                self.active_links.insert(node_id, links);
                let _ = self
                    .evt_tx
                    .send(RouterEvent::RouteCreated { node_id, identity });
            }
            Err(e) => {
                let _ = self.evt_tx.send(RouterEvent::Error(e));
            }
        }
    }

    /// Create or update an intent for the given identity.
    fn upsert_intent(&mut self, identity: AppIdentity, enabled: bool) {
        if let Some(existing) = self.intents.iter_mut().find(|i| i.identity == identity) {
            existing.enabled = enabled;
        } else {
            self.intents.push(RouteIntent { identity, enabled });
        }
    }

    /// Create stereo (FL + FR) PipeWire links from source output ports to
    /// virtual sink input ports. Returns the link objects; caller stores them.
    /// Dropping the Vec destroys the links.
    fn create_stereo_links(
        &self,
        node_id: u32,
        core: &pipewire::core::CoreRc,
    ) -> Result<Vec<pipewire::link::Link>, RouterError> {
        // Require both stereo ports on the sink side before creating any links.
        if self.sink_input_ports.len() < 2 {
            return Err(RouterError::SinkPortsUnavailable);
        }
        let source_ports = match self.known_sources.get(&node_id) {
            Some(info) if info.output_ports.len() >= 2 => &info.output_ports,
            _ => return Err(RouterError::SourcePortsUnavailable { node_id }),
        };
        let mut links = Vec::new();
        // Zip with explicit take(2) so we never create more than 2 (FL+FR) links
        // even if either side somehow advertises more ports than expected.
        for (src_port, sink_port) in source_ports
            .iter()
            .take(2)
            .zip(self.sink_input_ports.iter().take(2))
        {
            let link_props = pipewire::properties::properties! {
                "link.output.port" => src_port.to_string(),
                "link.input.port"  => sink_port.to_string(),
                "object.linger"    => "false",
            };
            let link = core
                .create_object::<pipewire::link::Link>("link-factory", &link_props)
                .map_err(|e| RouterError::LinkCreation {
                    src_port: *src_port,
                    sink_port: *sink_port,
                    source: e,
                })?;
            links.push(link);
        }
        Ok(links)
    }

    // ── Test-only helpers ─────────────────────────────────────────────────────

    /// Test-only: route source without creating real PipeWire links.
    #[cfg(test)]
    pub fn route_source_test(&mut self, node_id: u32) {
        if self.sink_input_ports.len() < 2 {
            let _ = self
                .evt_tx
                .send(RouterEvent::Error(RouterError::SinkPortsUnavailable));
            return;
        }
        let identity = match self.known_sources.get(&node_id).map(|i| i.identity.clone()) {
            Some(id) => id,
            None => {
                let _ = self
                    .evt_tx
                    .send(RouterEvent::Error(RouterError::SourcePortsUnavailable {
                        node_id,
                    }));
                return;
            }
        };
        if self
            .known_sources
            .get(&node_id)
            .map_or(0, |i| i.output_ports.len())
            < 2
        {
            let _ = self
                .evt_tx
                .send(RouterEvent::Error(RouterError::SourcePortsUnavailable {
                    node_id,
                }));
            return;
        }
        self.upsert_intent(identity.clone(), true);
        self.active_links.insert(node_id, vec![]);
        let _ = self
            .evt_tx
            .send(RouterEvent::RouteCreated { node_id, identity });
    }

    /// Test-only: try auto-reconnect without creating real PipeWire links.
    #[cfg(test)]
    pub fn try_auto_reconnect_test(&mut self, node_id: u32) {
        let identity = match self.known_sources.get(&node_id).map(|i| i.identity.clone()) {
            Some(id) => id,
            None => return,
        };
        let should_route = self
            .intents
            .iter()
            .any(|intent| intent.enabled && intent.identity.matches(&identity));
        if !should_route {
            return;
        }
        if self.sink_input_ports.len() < 2
            || self
                .known_sources
                .get(&node_id)
                .map_or(0, |i| i.output_ports.len())
                < 2
        {
            return;
        }
        if self.active_links.contains_key(&node_id) {
            return;
        }
        self.active_links.insert(node_id, vec![]);
        let _ = self
            .evt_tx
            .send(RouterEvent::AutoReconnected { identity, node_id });
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn identity(name: Option<&str>, binary: Option<&str>, pid: Option<u32>) -> AppIdentity {
        AppIdentity {
            app_name: name.map(str::to_owned),
            process_binary: binary.map(str::to_owned),
            process_id: pid,
        }
    }

    fn make_router() -> (Router, mpsc::Receiver<RouterEvent>) {
        let (tx, rx) = mpsc::channel();
        let router = Router::new(tx);
        (router, rx)
    }

    // ── AppIdentity::matches tests ────────────────────────────────────────────

    #[test]
    fn identity_matches_by_app_name() {
        let intent = identity(Some("Spotify"), None, None);
        let candidate = identity(Some("Spotify"), Some("spotify"), Some(1234));
        assert!(intent.matches(&candidate));
    }

    #[test]
    fn identity_no_match_different_app_name() {
        let intent = identity(Some("Spotify"), None, None);
        let candidate = identity(Some("Firefox"), None, None);
        assert!(!intent.matches(&candidate));
    }

    #[test]
    fn identity_falls_back_to_binary_when_app_name_none() {
        let intent = identity(None, Some("spotify"), None);
        let candidate = identity(None, Some("spotify"), Some(999));
        assert!(intent.matches(&candidate));
    }

    #[test]
    fn identity_no_match_binary_differs() {
        let intent = identity(None, Some("spotify"), None);
        let candidate = identity(None, Some("firefox"), None);
        assert!(!intent.matches(&candidate));
    }

    #[test]
    fn identity_both_none_returns_false() {
        let intent = identity(None, None, None);
        let candidate = identity(None, None, None);
        assert!(!intent.matches(&candidate));
    }

    #[test]
    fn identity_pid_required_when_set_in_intent() {
        let intent = identity(Some("Spotify"), None, Some(1234));
        let same_pid = identity(Some("Spotify"), None, Some(1234));
        let diff_pid = identity(Some("Spotify"), None, Some(9999));
        assert!(intent.matches(&same_pid));
        assert!(!intent.matches(&diff_pid));
    }

    #[test]
    fn identity_no_pid_in_intent_matches_all_instances() {
        let intent = identity(Some("Spotify"), None, None);
        let inst1 = identity(Some("Spotify"), None, Some(1111));
        let inst2 = identity(Some("Spotify"), None, Some(2222));
        assert!(intent.matches(&inst1));
        assert!(intent.matches(&inst2));
    }

    #[test]
    fn identity_app_name_takes_priority_over_binary_mismatch() {
        let intent = identity(Some("Spotify"), Some("different-binary"), None);
        let candidate = identity(Some("Spotify"), Some("spotify"), None);
        assert!(intent.matches(&candidate));
    }

    #[test]
    fn identity_intent_app_name_none_candidate_has_name_falls_to_binary() {
        let intent = identity(None, Some("spotify"), None);
        let candidate = identity(Some("Spotify"), Some("spotify"), None);
        assert!(intent.matches(&candidate));
    }

    #[test]
    fn from_stream_round_trips_fields() {
        let id =
            AppIdentity::from_stream(Some("Firefox".into()), Some("firefox".into()), Some(4242));
        assert_eq!(id.app_name.as_deref(), Some("Firefox"));
        assert_eq!(id.process_binary.as_deref(), Some("firefox"));
        assert_eq!(id.process_id, Some(4242));
    }

    #[test]
    fn router_command_variants_constructible() {
        let _ = RouterCommand::RouteSource { source_node_id: 1 };
        let _ = RouterCommand::UnrouteSource { source_node_id: 1 };
        let _ = RouterCommand::UnrouteAll;
    }

    #[test]
    fn router_event_variants_constructible() {
        let id = AppIdentity::from_stream(None, Some("test".into()), None);
        let _ = RouterEvent::RouteCreated {
            node_id: 1,
            identity: id.clone(),
        };
        let _ = RouterEvent::RouteDestroyed { node_id: 1 };
        let _ = RouterEvent::AutoReconnected {
            identity: id.clone(),
            node_id: 1,
        };
        let _ = RouterEvent::SourceDisconnected { identity: id };
        let _ = RouterEvent::Error(RouterError::SinkPortsUnavailable);
    }

    // ── Router state management tests ────────────────────────────────────────

    #[test]
    fn router_new_has_empty_state() {
        let (router, _rx) = make_router();
        assert!(router.intents.is_empty());
        assert!(router.active_links.is_empty());
        assert!(router.known_sources.is_empty());
        assert!(router.sink_input_ports.is_empty());
    }

    #[test]
    fn update_sink_ports_stores_ports() {
        let (mut router, _rx) = make_router();
        router.update_sink_ports(vec![10, 11]);
        assert_eq!(router.sink_input_ports, vec![10, 11]);
    }

    #[test]
    fn source_added_event_stores_identity_and_ports() {
        let (mut router, _rx) = make_router();
        router.on_source_added(
            42,
            Some("Spotify".into()),
            Some("spotify".into()),
            Some(1234),
        );
        assert!(router.known_sources.contains_key(&42));
        let info = &router.known_sources[&42];
        assert_eq!(info.identity.app_name.as_deref(), Some("Spotify"));
    }

    #[test]
    fn port_added_for_tracked_source_accumulates_ports() {
        let (mut router, _rx) = make_router();
        router.on_source_added(42, Some("Spotify".into()), None, None);
        router.on_port_added(100, 42, "FL".into(), Direction::Output);
        router.on_port_added(101, 42, "FR".into(), Direction::Output);
        let info = &router.known_sources[&42];
        assert_eq!(info.output_ports, vec![100, 101]);
    }

    #[test]
    fn port_added_for_untracked_source_is_buffered_not_in_known_sources() {
        let (mut router, _rx) = make_router();
        router.on_port_added(100, 999, "FL".into(), Direction::Output);
        // Does not fabricate a known_sources entry (which would have an empty
        // identity)...
        assert!(router.known_sources.is_empty());
        // ...but is retained for reconciliation when SourceAdded arrives.
        assert_eq!(router.pending_ports[&999], vec![100]);
    }

    #[test]
    fn ports_before_source_are_buffered_and_reconciled() {
        // Regression (PR #103 verification): PipeWire registry replay delivers a
        // pre-existing app's output ports BEFORE the async SourceAdded event, so
        // ports arrive while the node is still untracked. They must be buffered
        // and reconciled when the source arrives — not dropped — otherwise an
        // already-playing app (e.g. Firefox playing on launch) can never route.
        let (mut router, _rx) = make_router();
        router.on_port_added(131, 109, "FL".into(), Direction::Output);
        router.on_port_added(134, 109, "FR".into(), Direction::Output);
        router.on_source_added(
            109,
            Some("Firefox".into()),
            Some("firefox".into()),
            Some(12529),
        );
        let info = &router.known_sources[&109];
        assert_eq!(info.output_ports, vec![131, 134]);
    }

    #[test]
    fn already_playing_source_routes_after_ports_then_source() {
        // End-to-end of the same regression: with ports delivered before the
        // source (the replay ordering for an already-playing app), a subsequent
        // RouteSource command must succeed, not error SourcePortsUnavailable.
        let (mut router, rx) = make_router();
        router.update_sink_ports(vec![10, 11]);
        router.on_port_added(131, 109, "FL".into(), Direction::Output);
        router.on_port_added(134, 109, "FR".into(), Direction::Output);
        router.on_source_added(
            109,
            Some("Firefox".into()),
            Some("firefox".into()),
            Some(12529),
        );

        router.route_source_test(109);

        match rx.try_recv().expect("expected a RouterEvent") {
            RouterEvent::RouteCreated { node_id, .. } => assert_eq!(node_id, 109),
            other => panic!("expected RouteCreated, got {other:?}"),
        }
    }

    #[test]
    fn source_removed_discards_buffered_ports() {
        // Buffered ports for a node that is removed before its source resolves
        // must not leak into a later, unrelated source on the same node id.
        let (mut router, _rx) = make_router();
        router.on_port_added(131, 109, "FL".into(), Direction::Output);
        router.on_source_removed(109);
        router.on_source_added(109, Some("Firefox".into()), None, None);
        assert!(router.known_sources[&109].output_ports.is_empty());
    }

    #[test]
    fn source_removed_cleans_active_links_but_preserves_intent() {
        let (mut router, rx) = make_router();
        let id = AppIdentity::from_stream(Some("Spotify".into()), None, None);
        router.intents.push(RouteIntent {
            identity: id,
            enabled: true,
        });
        router.active_links.insert(42, vec![]);
        router.on_source_added(42, Some("Spotify".into()), None, None);

        router.on_source_removed(42);

        assert!(!router.active_links.contains_key(&42));
        assert_eq!(router.intents.len(), 1);
        assert!(!router.known_sources.contains_key(&42));

        let events: Vec<RouterEvent> = rx.try_iter().collect();
        let disconnected = events
            .iter()
            .any(|e| matches!(e, RouterEvent::SourceDisconnected { .. }));
        assert!(disconnected, "expected SourceDisconnected in {events:?}");
    }

    #[test]
    fn unroute_all_clears_intents_and_emits_route_destroyed() {
        let (mut router, rx) = make_router();
        router.active_links.insert(1, vec![]);
        router.active_links.insert(2, vec![]);
        router.intents.push(RouteIntent {
            identity: AppIdentity::from_stream(Some("A".into()), None, None),
            enabled: true,
        });

        router.handle_command_unroute_all();

        assert!(router.intents.is_empty());
        assert!(router.active_links.is_empty());

        let events: Vec<RouterEvent> = rx.try_iter().collect();
        let destroyed_ids: Vec<u32> = events
            .iter()
            .filter_map(|e| match e {
                RouterEvent::RouteDestroyed { node_id } => Some(*node_id),
                _ => None,
            })
            .collect();
        assert!(destroyed_ids.contains(&1));
        assert!(destroyed_ids.contains(&2));
    }

    #[test]
    fn unroute_source_removes_intent_and_active_link() {
        let (mut router, rx) = make_router();
        let id = AppIdentity::from_stream(Some("Spotify".into()), None, None);
        router.intents.push(RouteIntent {
            identity: id.clone(),
            enabled: true,
        });
        router.active_links.insert(42, vec![]);
        router.on_source_added(42, Some("Spotify".into()), None, None);

        router.handle_command_unroute_source(42);

        assert!(!router.active_links.contains_key(&42));
        assert!(!router.intents[0].enabled);

        let events: Vec<RouterEvent> = rx.try_iter().collect();
        let destroyed = events
            .iter()
            .any(|e| matches!(e, RouterEvent::RouteDestroyed { node_id: 42 }));
        assert!(destroyed, "expected RouteDestroyed(42) in {events:?}");
    }

    #[test]
    fn route_source_adds_intent_and_sends_route_created() {
        let (mut router, rx) = make_router();
        router.update_sink_ports(vec![10, 11]);
        router.on_source_added(42, Some("Spotify".into()), None, None);
        router.on_port_added(100, 42, "FL".into(), Direction::Output);
        router.on_port_added(101, 42, "FR".into(), Direction::Output);

        router.route_source_test(42);

        assert_eq!(router.intents.len(), 1);
        assert!(router.intents[0].enabled);

        let events: Vec<RouterEvent> = rx.try_iter().collect();
        let created = events
            .iter()
            .any(|e| matches!(e, RouterEvent::RouteCreated { node_id: 42, .. }));
        assert!(created, "expected RouteCreated(42) in {events:?}");
    }

    #[test]
    fn auto_reconnect_fires_when_source_added_matches_intent() {
        let (mut router, rx) = make_router();
        router.update_sink_ports(vec![10, 11]);
        router.intents.push(RouteIntent {
            identity: AppIdentity::from_stream(Some("Spotify".into()), None, None),
            enabled: true,
        });
        router.on_source_added(99, Some("Spotify".into()), None, None);
        router.on_port_added(200, 99, "FL".into(), Direction::Output);
        router.on_port_added(201, 99, "FR".into(), Direction::Output);
        router.try_auto_reconnect_test(99);

        assert!(router.active_links.contains_key(&99));

        let events: Vec<RouterEvent> = rx.try_iter().collect();
        let reconnected = events
            .iter()
            .any(|e| matches!(e, RouterEvent::AutoReconnected { node_id: 99, .. }));
        assert!(reconnected, "expected AutoReconnected(99) in {events:?}");
    }

    #[test]
    fn auto_reconnect_does_not_fire_for_disabled_intent() {
        let (mut router, rx) = make_router();
        router.update_sink_ports(vec![10, 11]);
        router.intents.push(RouteIntent {
            identity: AppIdentity::from_stream(Some("Spotify".into()), None, None),
            enabled: false,
        });
        router.on_source_added(99, Some("Spotify".into()), None, None);
        router.on_port_added(200, 99, "FL".into(), Direction::Output);
        router.on_port_added(201, 99, "FR".into(), Direction::Output);
        router.try_auto_reconnect_test(99);

        assert!(!router.active_links.contains_key(&99));
        let events: Vec<RouterEvent> = rx.try_iter().collect();
        let reconnected = events
            .iter()
            .any(|e| matches!(e, RouterEvent::AutoReconnected { .. }));
        assert!(!reconnected, "unexpected AutoReconnected in {events:?}");
    }

    #[test]
    fn route_source_with_no_sink_ports_sends_error() {
        let (mut router, rx) = make_router();
        router.on_source_added(42, Some("Spotify".into()), None, None);
        router.on_port_added(100, 42, "FL".into(), Direction::Output);

        router.route_source_test(42);

        let events: Vec<RouterEvent> = rx.try_iter().collect();
        let has_error = events
            .iter()
            .any(|e| matches!(e, RouterEvent::Error(RouterError::SinkPortsUnavailable)));
        assert!(
            has_error,
            "expected SinkPortsUnavailable error in {events:?}"
        );
    }

    #[test]
    fn route_source_with_no_source_ports_sends_error() {
        let (mut router, rx) = make_router();
        router.update_sink_ports(vec![10, 11]);
        router.on_source_added(42, Some("Spotify".into()), None, None);

        router.route_source_test(42);

        let events: Vec<RouterEvent> = rx.try_iter().collect();
        let has_error = events.iter().any(|e| {
            matches!(
                e,
                RouterEvent::Error(RouterError::SourcePortsUnavailable { node_id: 42 })
            )
        });
        assert!(
            has_error,
            "expected SourcePortsUnavailable(42) in {events:?}"
        );
    }

    /// Integration test: requires a live PipeWire session.
    /// Run with: cargo test --features pipewire-test route_integration
    #[cfg(feature = "pipewire-test")]
    #[test]
    fn route_creates_visible_link_in_pipewire() {
        // Implementation deferred — requires PipeWire test harness setup (#27 scope).
        // This stub ensures the test infrastructure compiles and the feature gate works.
        let _is_integration_test = true;
        eprintln!("SKIP: pipewire-test integration not yet implemented");
    }
}
