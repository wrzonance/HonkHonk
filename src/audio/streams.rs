//! External audio stream watcher (issue #26).
//!
//! Observes PipeWire registry for `Stream/Output/Audio` nodes belonging to
//! OTHER applications (Spotify, Firefox, paplay, ...). Captures stable
//! identity properties (`application.name`, `application.process.binary`,
//! `application.process.id`) so future route-restoration work (#27) and
//! per-stream volume work (#29) can reconnect intent across stream
//! destroy/recreate cycles.
//!
//! This module is observation-only. It does NOT:
//! - render any UI (issue #28),
//! - create links / route audio (issue #27),
//! - manipulate stream volume / mute (issue #29).
//!
//! It owns an independent registry listener attached to the same
//! `pipewire::core::CoreRc` already managed by `audio::engine`.
//! Sibling pattern to `audio::registry` — registry helpers stay separate
//! per CLAUDE.md "self-contained unit, one clear purpose".

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc;

use pipewire::spa::utils::dict::DictRef;

use super::error::{AudioError, WatcherError};

const STREAM_OUTPUT_AUDIO: &str = "Stream/Output/Audio";

/// Direction of a port belonging to a tracked stream node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

/// Live updates emitted as external audio streams come and go.
///
/// All variants carry the PipeWire object id (`u32`) as primary key.
/// Optional `app_*` props are captured at first-seen time and remain
/// stable for the lifetime of the producing process (per issue #26
/// "Stable Identity Properties" rationale).
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A new `Stream/Output/Audio` node belonging to an external app
    /// has appeared. Emitted ONCE per node, after props are extracted.
    SourceAdded {
        id: u32,
        /// Display name: `node.description` → `node.nick` → `node.name` → `"unknown"`.
        name: String,
        app_name: Option<String>,
        app_binary: Option<String>,
        app_pid: Option<u32>,
        icon: Option<String>,
        media_name: Option<String>,
    },
    /// A previously tracked node was destroyed.
    SourceRemoved { id: u32 },
    /// Tracked node's transient state changed (e.g. current track title).
    /// Reserved for future `node.info` change events (#28 will consume).
    SourceUpdated { id: u32, media_name: Option<String> },
    /// A port belonging to a tracked stream node appeared.
    PortAdded {
        id: u32,
        node_id: u32,
        channel: String,
        direction: Direction,
    },
    /// A previously seen port was destroyed.
    PortRemoved { id: u32 },
}

/// RAII guard owning the registry listener and any tracked node proxies.
///
/// Drop semantics: dropping the watcher detaches the registry listener
/// and frees every stashed node proxy, which lets PipeWire release the
/// associated server-side state.
pub struct StreamWatcher {
    _registry: pipewire::registry::RegistryRc,
    _listener: pipewire::registry::Listener,
    _tracked_nodes: Rc<RefCell<HashMap<u32, TrackedNode>>>,
    _tracked_ports: Rc<RefCell<HashSet<u32>>>,
}

/// Shared state needed by the registry `global` callback. Wrapping these
/// in one struct keeps `handle_global`'s arg count within
/// `too-many-arguments-threshold = 5` (clippy.toml).
struct HandleGlobalCtx {
    self_pid: u32,
    registry: pipewire::registry::RegistryRc,
    tracked_nodes: Rc<RefCell<HashMap<u32, TrackedNode>>>,
    tracked_ports: Rc<RefCell<HashSet<u32>>>,
    tx: mpsc::Sender<StreamEvent>,
}

/// Per-node bookkeeping: proxy keeps the bind alive, listener fires
/// `node.info` once props arrive in full.
struct TrackedNode {
    _node: pipewire::node::Node,
    _listener: pipewire::node::NodeListener,
}

/// Read `application.process.id` and compare against `self_pid`.
///
/// Fail-open: if the property is missing or non-numeric, returns `false`
/// (node is NOT treated as our own — we'd rather report a stray external
/// node than silently drop a legitimate one).
fn is_own_node(props: &DictRef, self_pid: u32) -> bool {
    props
        .get("application.process.id")
        .and_then(|s| s.parse::<u32>().ok())
        .map(|pid| pid == self_pid)
        .unwrap_or(false)
}

/// Extract human-readable name with documented fallback chain.
fn extract_name(props: &DictRef) -> String {
    props
        .get("node.description")
        .or_else(|| props.get("node.nick"))
        .or_else(|| props.get("node.name"))
        .unwrap_or("unknown")
        .to_owned()
}

/// Extract the producing process PID, if PipeWire exposed it.
fn extract_pid(props: &DictRef) -> Option<u32> {
    props
        .get("application.process.id")
        .and_then(|s| s.parse().ok())
}

fn extract_opt(props: &DictRef, key: &str) -> Option<String> {
    props.get(key).map(str::to_owned)
}

/// Start a PipeWire registry watcher.
///
/// On success returns the watcher (hold until shutdown — its `Drop`
/// detaches the listener) and a receiver yielding `StreamEvent`s as
/// they happen.
///
/// `self_pid` is compared against each incoming node's
/// `application.process.id` to exclude HonkHonk's own playback nodes.
pub fn start(
    core: &pipewire::core::CoreRc,
    self_pid: u32,
) -> Result<(StreamWatcher, mpsc::Receiver<StreamEvent>), AudioError> {
    let (tx, rx) = mpsc::channel::<StreamEvent>();

    let registry = core
        .get_registry_rc()
        .map_err(|e| AudioError::StreamWatcherInit(WatcherError::RegistryAcquire(e)))?;

    let tracked_nodes: Rc<RefCell<HashMap<u32, TrackedNode>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let tracked_ports: Rc<RefCell<HashSet<u32>>> = Rc::new(RefCell::new(HashSet::new()));

    let ctx = Rc::new(HandleGlobalCtx {
        self_pid,
        registry: registry.clone(),
        tracked_nodes: tracked_nodes.clone(),
        tracked_ports: tracked_ports.clone(),
        tx: tx.clone(),
    });
    let ctx_global = ctx.clone();

    let tracked_nodes_remove = tracked_nodes.clone();
    let tracked_ports_remove = tracked_ports.clone();
    let tx_remove = tx;

    let listener = registry
        .add_listener_local()
        .global(move |global| {
            handle_global(global, &ctx_global);
        })
        .global_remove(move |id| {
            // Distinguish node vs port lifecycle so consumers can reconcile
            // each independently. A removed ID belongs to exactly one of:
            // a tracked stream node, a tracked port, or something we never
            // observed (silently ignored).
            if tracked_nodes_remove.borrow_mut().remove(&id).is_some() {
                let _ = tx_remove.send(StreamEvent::SourceRemoved { id });
                return;
            }
            if tracked_ports_remove.borrow_mut().remove(&id) {
                let _ = tx_remove.send(StreamEvent::PortRemoved { id });
            }
        })
        .register();

    Ok((
        StreamWatcher {
            _registry: registry,
            _listener: listener,
            _tracked_nodes: tracked_nodes,
            _tracked_ports: tracked_ports,
        },
        rx,
    ))
}

fn handle_global(global: &pipewire::registry::GlobalObject<&DictRef>, ctx: &HandleGlobalCtx) {
    let props = match global.props {
        Some(p) => p,
        None => return,
    };

    match global.type_ {
        pipewire::types::ObjectType::Node => {
            if props.get("media.class") != Some(STREAM_OUTPUT_AUDIO) {
                return;
            }
            if is_own_node(props, ctx.self_pid) {
                return;
            }
            bind_and_track_node(global, &ctx.registry, &ctx.tracked_nodes, &ctx.tx);
        }
        pipewire::types::ObjectType::Port => {
            forward_port_event(
                global,
                props,
                &ctx.tracked_nodes,
                &ctx.tracked_ports,
                &ctx.tx,
            );
        }
        _ => {}
    }
}

fn bind_and_track_node(
    global: &pipewire::registry::GlobalObject<&DictRef>,
    registry: &pipewire::registry::RegistryRc,
    tracked: &Rc<RefCell<HashMap<u32, TrackedNode>>>,
    tx: &mpsc::Sender<StreamEvent>,
) {
    let node: pipewire::node::Node = match registry.bind(global) {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(node = global.id, error = %e, "failed to bind stream node");
            return;
        }
    };

    let id = global.id;
    let tx_info = tx.clone();
    let emitted_added = Rc::new(RefCell::new(false));
    let emitted_added_clone = emitted_added.clone();

    let listener = node
        .add_listener_local()
        .info(move |info| {
            on_node_info(id, info, &emitted_added_clone, &tx_info);
        })
        .register();

    tracked.borrow_mut().insert(
        id,
        TrackedNode {
            _node: node,
            _listener: listener,
        },
    );
}

fn on_node_info(
    id: u32,
    info: &pipewire::node::NodeInfoRef,
    emitted_added: &Rc<RefCell<bool>>,
    tx: &mpsc::Sender<StreamEvent>,
) {
    let info_props = match info.props() {
        Some(p) => p,
        None => return,
    };

    if !*emitted_added.borrow() {
        let event = StreamEvent::SourceAdded {
            id,
            name: extract_name(info_props),
            app_name: extract_opt(info_props, "application.name"),
            app_binary: extract_opt(info_props, "application.process.binary"),
            app_pid: extract_pid(info_props),
            icon: extract_opt(info_props, "application.icon_name"),
            media_name: extract_opt(info_props, "media.name"),
        };
        *emitted_added.borrow_mut() = true;
        let _ = tx.send(event);
    } else {
        let _ = tx.send(StreamEvent::SourceUpdated {
            id,
            media_name: extract_opt(info_props, "media.name"),
        });
    }
}

fn forward_port_event(
    global: &pipewire::registry::GlobalObject<&DictRef>,
    props: &DictRef,
    tracked_nodes: &Rc<RefCell<HashMap<u32, TrackedNode>>>,
    tracked_ports: &Rc<RefCell<HashSet<u32>>>,
    tx: &mpsc::Sender<StreamEvent>,
) {
    let node_id: u32 = match props.get("node.id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return,
    };
    if !tracked_nodes.borrow().contains_key(&node_id) {
        return;
    }
    let channel = props.get("audio.channel").unwrap_or("UNKNOWN").to_owned();
    let direction = match props.get("port.direction") {
        Some("in") => Direction::Input,
        Some("out") => Direction::Output,
        _ => return,
    };
    // Record the port so global_remove can emit the matching PortRemoved.
    tracked_ports.borrow_mut().insert(global.id);
    let _ = tx.send(StreamEvent::PortAdded {
        id: global.id,
        node_id,
        channel,
        direction,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use pipewire::properties::properties;

    fn dict<F: FnOnce(&DictRef) -> R, R>(props: &pipewire::properties::PropertiesBox, f: F) -> R {
        let d: &DictRef = props.as_ref();
        f(d)
    }

    #[test]
    fn is_own_node_matches_self_pid() {
        let props = properties! { "application.process.id" => "4242" };
        assert!(dict(&props, |d| is_own_node(d, 4242)));
    }

    #[test]
    fn is_own_node_skips_when_pid_differs() {
        let props = properties! { "application.process.id" => "4242" };
        assert!(!dict(&props, |d| is_own_node(d, 9999)));
    }

    #[test]
    fn is_own_node_fails_open_when_pid_missing() {
        let props = properties! { "node.name" => "spotify" };
        assert!(!dict(&props, |d| is_own_node(d, 4242)));
    }

    #[test]
    fn is_own_node_fails_open_on_non_numeric_pid() {
        let props = properties! { "application.process.id" => "not-a-number" };
        assert!(!dict(&props, |d| is_own_node(d, 4242)));
    }

    #[test]
    fn extract_name_uses_description_first() {
        let props = properties! {
            "node.description" => "Spotify Premium",
            "node.nick" => "Spotify",
            "node.name" => "spotify",
        };
        assert_eq!(dict(&props, extract_name), "Spotify Premium");
    }

    #[test]
    fn extract_name_falls_back_to_nick() {
        let props = properties! {
            "node.nick" => "Spotify",
            "node.name" => "spotify",
        };
        assert_eq!(dict(&props, extract_name), "Spotify");
    }

    #[test]
    fn extract_name_falls_back_to_node_name() {
        let props = properties! { "node.name" => "spotify" };
        assert_eq!(dict(&props, extract_name), "spotify");
    }

    #[test]
    fn extract_name_defaults_to_unknown_when_all_missing() {
        let props = properties! { "media.class" => "Stream/Output/Audio" };
        assert_eq!(dict(&props, extract_name), "unknown");
    }

    #[test]
    fn extract_pid_parses_numeric() {
        let props = properties! { "application.process.id" => "1234" };
        assert_eq!(dict(&props, extract_pid), Some(1234));
    }

    #[test]
    fn extract_pid_returns_none_when_missing() {
        let props = properties! { "node.name" => "spotify" };
        assert_eq!(dict(&props, extract_pid), None);
    }

    #[test]
    fn extract_pid_returns_none_on_non_numeric() {
        let props = properties! { "application.process.id" => "abc" };
        assert_eq!(dict(&props, extract_pid), None);
    }

    #[test]
    fn extract_opt_returns_value_when_present() {
        let props = properties! { "application.name" => "Firefox" };
        assert_eq!(
            dict(&props, |d| extract_opt(d, "application.name")),
            Some("Firefox".to_owned())
        );
    }

    #[test]
    fn extract_opt_returns_none_when_absent() {
        let props = properties! { "node.name" => "x" };
        assert_eq!(dict(&props, |d| extract_opt(d, "application.name")), None);
    }

    #[test]
    fn direction_variants_are_distinct() {
        // Pin the public enum surface so a future enum-shuffle review
        // catches accidental variant reorder/removal.
        assert_ne!(Direction::Input, Direction::Output);
    }
}
