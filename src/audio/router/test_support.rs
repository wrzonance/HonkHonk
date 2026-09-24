use super::*;

impl Router {
    fn populate_test_graph(&mut self, node_id: u32) {
        use crate::audio::routing_graph::{NodeInfo, PortInfo};
        let mut graph = self.graph.borrow_mut();
        graph.nodes.insert(
            1,
            NodeInfo {
                name: "honkhonk-mix".into(),
                ..Default::default()
            },
        );
        graph.nodes.insert(
            node_id,
            NodeInfo {
                class: "Stream/Output/Audio".into(),
                ..Default::default()
            },
        );
        for &id in &self.sink_input_ports {
            graph.ports.insert(
                id,
                PortInfo {
                    node: 1,
                    input: true,
                    monitor: false,
                },
            );
        }
        if let Some(source) = self.known_sources.get(&node_id) {
            for &id in &source.output_ports {
                graph.ports.insert(
                    id,
                    PortInfo {
                        node: node_id,
                        input: false,
                        monitor: false,
                    },
                );
            }
        }
    }

    pub fn route_source_test(&mut self, node_id: u32) {
        self.set_safe_mode(false);
        self.populate_test_graph(node_id);
        self.route_with(node_id, false, |_| Ok(vec![]));
    }

    pub fn try_auto_reconnect_test(&mut self, node_id: u32) {
        self.set_safe_mode(false);
        self.populate_test_graph(node_id);
        self.route_with(node_id, true, |_| Ok(vec![]));
    }
}
