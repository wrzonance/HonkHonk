//! Safety policy over a snapshot of PipeWire's directed graph.
use std::collections::{HashMap, HashSet};

mod observer;
pub(crate) use observer::GraphWatcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RouteRejection {
    #[error("routing metadata is not available yet")]
    Unknown,
    #[error("HonkHonk cannot route its own output back into its mix")]
    SelfRoute,
    #[error("only application playback and microphone sources can be routed")]
    MediaClass,
    #[error("monitor ports cannot be routed into the mix")]
    Monitor,
    #[error("this source is downstream of HonkHonk's output")]
    Cycle,
    #[error("safe mode disables application passthrough")]
    SafeMode,
    #[error("source is cooling down after feedback")]
    Cooldown,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct NodeInfo {
    pub name: String,
    pub class: String,
    pub app: String,
    pub process: Option<u32>,
    pub client: Option<u32>,
    pub link_group: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PortInfo {
    pub node: u32,
    pub input: bool,
    pub monitor: bool,
}

#[derive(Debug, Default)]
pub(crate) struct RoutingGraph {
    pub nodes: HashMap<u32, NodeInfo>,
    pub ports: HashMap<u32, PortInfo>,
    pub links: HashMap<u32, (u32, u32)>,
}

impl NodeInfo {
    fn own(&self) -> bool {
        self.name.starts_with("honkhonk-") || self.app.eq_ignore_ascii_case("honkhonk")
    }

    fn same_application(&self, other: &Self) -> bool {
        match (self.process, other.process) {
            (Some(a), Some(b)) => a == b,
            _ => {
                (self.client.is_some() && self.client == other.client)
                    || (!self.app.is_empty() && self.app == other.app)
            }
        }
    }

    fn internally_connected(&self, other: &Self) -> bool {
        // PipeWire's scalar node.link-group identifies coupled streams. Treat
        // matching nonempty IDs as possible signal paths even across classes.
        (!self.link_group.is_empty() && self.link_group == other.link_group)
            || (self.class == "Stream/Input/Audio"
                && other.class == "Stream/Output/Audio"
                && self.same_application(other))
    }
}

impl RoutingGraph {
    pub fn check(&self, source: u32, ports: &[(u32, u32)]) -> Result<(), RouteRejection> {
        let node = self.nodes.get(&source).ok_or(RouteRejection::Unknown)?;
        if node.own() {
            return Err(RouteRejection::SelfRoute);
        }
        if !matches!(node.class.as_str(), "Stream/Output/Audio" | "Audio/Source") {
            return Err(RouteRejection::MediaClass);
        }
        if ports.is_empty() {
            return Err(RouteRejection::Unknown);
        }
        for &(output, input) in ports {
            self.check_ports(source, output, input)?;
        }
        if self.downstream(source) {
            return Err(RouteRejection::Cycle);
        }
        Ok(())
    }

    fn check_ports(&self, source: u32, output: u32, input: u32) -> Result<(), RouteRejection> {
        let out = self.ports.get(&output).ok_or(RouteRejection::Unknown)?;
        let dst = self.ports.get(&input).ok_or(RouteRejection::Unknown)?;
        let sink = self.nodes.get(&dst.node).ok_or(RouteRejection::Unknown)?;
        if out.monitor {
            return Err(RouteRejection::Monitor);
        }
        if out.node != source || out.input || !dst.input || sink.name != "honkhonk-mix" {
            return Err(RouteRejection::Unknown);
        }
        Ok(())
    }

    fn downstream(&self, target: u32) -> bool {
        let mut pending: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, node)| matches!(node.name.as_str(), "honkhonk-mix" | "honkhonk-mic"))
            .map(|(&id, _)| id)
            .collect();
        let mut seen = HashSet::new();
        while let Some(id) = pending.pop() {
            if id == target {
                return true;
            }
            if !seen.insert(id) {
                continue;
            }
            pending.extend(
                self.links
                    .values()
                    .filter(|(from, _)| *from == id)
                    .map(|(_, to)| *to),
            );
            if let Some(node) = self.nodes.get(&id) {
                pending.extend(
                    self.nodes
                        .iter()
                        .filter(|(_, n)| node.internally_connected(n))
                        .map(|(&id, _)| id),
                );
            }
        }
        false
    }

    pub fn remove(&mut self, id: u32) {
        self.nodes.remove(&id);
        self.ports.remove(&id);
        self.links.remove(&id);
        self.ports.retain(|_, port| port.node != id);
        self.links.retain(|_, (from, to)| *from != id && *to != id);
    }
}

#[cfg(test)]
mod tests;
