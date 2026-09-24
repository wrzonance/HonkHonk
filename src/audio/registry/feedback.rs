use super::*;
use crate::audio::{FeedbackSource, RouteRejection};
use std::time::{Duration, Instant};

impl RegistryGuard {
    pub(crate) fn latest_source(&self) -> Option<(Instant, u32)> {
        if self.mic_links.borrow().is_empty() {
            return None;
        }
        let state = self.state.borrow();
        Some((state.mic_activated?, state.mic_node_id?))
    }

    pub(crate) fn trip_feedback(&self, now: Instant) -> Option<FeedbackSource> {
        let source = {
            let state = self.state.borrow();
            let id = state.mic_node_id?;
            let name = state
                .input_sources
                .iter()
                .find(|(node, _, _)| *node == id)
                .map(|(_, _, name)| name.clone())
                .unwrap_or_else(|| format!("Microphone {id}"));
            tracing::warn!(node_id = id, graph = ?state.graph.borrow(), "feedback: microphone disconnected");
            FeedbackSource { node_id: id, name }
        };
        self.apply_passthrough(false);
        let _ = self.evt_tx.send(AudioEvent::MicrophoneFeedbackMuted);
        self.state.borrow_mut().mic_cooldown = Some(now + Duration::from_secs(2));
        Some(source)
    }

    pub(super) fn cooling_down(&self) -> bool {
        let state = self.state.borrow();
        if state
            .mic_cooldown
            .is_some_and(|until| until > Instant::now())
        {
            if let Some(node_id) = state.mic_node_id {
                let _ = self.evt_tx.send(AudioEvent::RouteRejected {
                    node_id,
                    reason: RouteRejection::Cooldown,
                });
            }
            return true;
        }
        false
    }
}
