use super::*;
use crate::audio::RouteRejection;

impl RegistryState {
    pub(super) fn remove_global(&mut self, id: u32) {
        self.input_sources.retain(|(node, _, _)| *node != id);
        self.output_sinks.retain(|(node, _, _)| *node != id);
        self.source_ports.remove(&id);
        for ports in self.source_ports.values_mut() {
            ports.retain(|port| *port != id);
        }
        for ports in [
            &mut self.mic_output_ports,
            &mut self.sink_input_ports,
            &mut self.sink_output_ports,
            &mut self.vsource_input_ports,
        ] {
            ports.retain(|port| *port != id);
        }
        self.linked_pairs
            .retain(|(from, to)| *from != id && *to != id);
        if self.sink_node_id == Some(id) {
            self.sink_node_id = None;
            self.sink_input_ports.clear();
            self.sink_output_ports.clear();
            self.linked_pairs.clear();
        }
        if self.vsource_node_id == Some(id) {
            self.vsource_node_id = None;
            self.vsource_input_ports.clear();
        }
        if self.mic_node_id == Some(id) {
            reselect_mic(self);
        }
    }

    pub(super) fn report_mic_rejection(
        &mut self,
        reason: RouteRejection,
        tx: &mpsc::Sender<AudioEvent>,
    ) {
        let Some(node_id) = self.mic_node_id else {
            return;
        };
        if reason == RouteRejection::Unknown || self.last_mic_rejection == Some((node_id, reason)) {
            return;
        }
        self.last_mic_rejection = Some((node_id, reason));
        tracing::warn!(node_id, %reason, graph = ?self.graph.borrow(), "microphone route rejected");
        let _ = tx.send(AudioEvent::RouteRejected { node_id, reason });
    }
}
