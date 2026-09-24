use super::*;
use crate::audio::{FeedbackSource, RouteRejection};
use std::collections::VecDeque;
use std::time::Instant;

pub(super) struct RoutingSafety {
    pub safe_mode: bool,
    pub activated: HashMap<u32, Instant>,
    pub cooldowns: Vec<(u32, AppIdentity, Instant)>,
    pub undo: VecDeque<Vec<RouteIntent>>,
}

impl Default for RoutingSafety {
    fn default() -> Self {
        Self {
            safe_mode: true,
            activated: HashMap::new(),
            cooldowns: Vec::new(),
            undo: VecDeque::new(),
        }
    }
}

impl Router {
    /// Safe mode starts enabled and never overrides a graph rejection.
    pub fn set_safe_mode(&mut self, enabled: bool) {
        self.safety.safe_mode = enabled;
        if enabled {
            let ids: Vec<_> = self.active_links.keys().copied().collect();
            for id in ids {
                self.unroute(id);
            }
        }
    }

    pub(super) fn remember_change(&mut self) {
        if self.safety.undo.len() == 3 {
            self.safety.undo.pop_front();
        }
        self.safety.undo.push_back(self.intents.clone());
    }

    /// Restores intent only; the engine must recreate links through the normal checks.
    pub fn restore_last_change(&mut self) {
        if let Some(intents) = self.safety.undo.pop_back() {
            let ids: Vec<_> = self.active_links.keys().copied().collect();
            for id in ids {
                self.unroute(id);
            }
            self.intents = intents;
        }
    }

    pub(crate) fn latest_source(&self) -> Option<(Instant, u32)> {
        self.safety
            .activated
            .iter()
            .filter(|(id, _)| self.active_links.contains_key(id))
            .map(|(&id, &time)| (time, id))
            .max()
    }

    pub(crate) fn trip_feedback(&mut self, node_id: u32, now: Instant) -> Option<FeedbackSource> {
        let mut identity = self.known_sources.get(&node_id)?.identity.clone();
        let name = identity
            .app_name
            .clone()
            .or_else(|| identity.process_binary.clone())
            .unwrap_or_else(|| format!("Source {node_id}"));
        // PID changes on restart must not bypass the cooldown.
        identity.process_id = None;
        self.safety.cooldowns.retain(|(_, _, until)| *until > now);
        self.safety.cooldowns.push((
            node_id,
            identity.clone(),
            now + std::time::Duration::from_secs(2),
        ));
        let ids: Vec<_> = self
            .known_sources
            .iter()
            .filter(|(_, info)| identity.matches(&info.identity))
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.unroute(id);
        }
        self.unroute(node_id);
        tracing::warn!(node_id, ?identity, graph = ?self.graph.borrow(), "feedback: route disconnected");
        Some(FeedbackSource { node_id, name })
    }

    pub(super) fn safety_check(&self, node_id: u32, now: Instant) -> Result<(), RouteRejection> {
        let info = self
            .known_sources
            .get(&node_id)
            .ok_or(RouteRejection::Unknown)?;
        if self
            .safety
            .cooldowns
            .iter()
            .any(|(original_node, id, until)| {
                *until > now && (*original_node == node_id || id.matches(&info.identity))
            })
        {
            return Err(RouteRejection::Cooldown);
        }
        if self.safety.safe_mode {
            return Err(RouteRejection::SafeMode);
        }
        Ok(())
    }
}
