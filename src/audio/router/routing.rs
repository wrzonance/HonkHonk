use super::*;

type LinkResult = Result<Vec<pipewire::link::Link>, RouterError>;

impl Router {
    pub(crate) fn retry_pending(&mut self, core: &pipewire::core::CoreRc) {
        let ids: Vec<_> = self
            .known_sources
            .keys()
            .copied()
            .filter(|id| {
                !self.active_links.contains_key(id)
                    && self.wants_reconnect(*id)
                    && self.safety_check(*id, std::time::Instant::now()).is_ok()
                    && self
                        .port_pairs(*id)
                        .is_ok_and(|pairs| self.graph.borrow().check(*id, &pairs).is_ok())
            })
            .collect();
        for id in ids {
            self.try_auto_reconnect(id, core);
        }
    }

    pub fn route_source(&mut self, node_id: u32, core: &pipewire::core::CoreRc) {
        self.route_with(node_id, false, |pairs| create_links(core, pairs));
    }

    pub fn try_auto_reconnect(&mut self, node_id: u32, core: &pipewire::core::CoreRc) {
        self.route_with(node_id, true, |pairs| create_links(core, pairs));
    }

    fn wants_reconnect(&self, node_id: u32) -> bool {
        self.known_sources.get(&node_id).is_some_and(|info| {
            self.intents
                .iter()
                .any(|intent| intent.enabled && intent.identity.matches(&info.identity))
                && info.output_ports.len() >= 2
                && self.sink_input_ports.len() >= 2
        })
    }

    pub(super) fn route_with(
        &mut self,
        node_id: u32,
        automatic: bool,
        create: impl FnOnce(&[(u32, u32)]) -> LinkResult,
    ) {
        if self.active_links.contains_key(&node_id) || (automatic && !self.wants_reconnect(node_id))
        {
            return;
        }
        let result = self.port_pairs(node_id).and_then(|pairs| {
            self.safety_check(node_id, std::time::Instant::now())
                .map_err(|reason| RouterError::UnsafeRoute { node_id, reason })?;
            self.graph
                .borrow()
                .check(node_id, &pairs)
                .map_err(|reason| RouterError::UnsafeRoute { node_id, reason })?;
            create(&pairs)
        });
        match result {
            Ok(links) => {
                let Some(info) = self.known_sources.get(&node_id) else {
                    return;
                };
                let identity = info.identity.clone();
                if !automatic {
                    self.remember_change();
                }
                self.upsert_intent(identity.clone(), true);
                self.safety
                    .activated
                    .insert(node_id, std::time::Instant::now());
                self.active_links.insert(node_id, links);
                let event = if automatic {
                    RouterEvent::AutoReconnected { identity, node_id }
                } else {
                    RouterEvent::RouteCreated { node_id, identity }
                };
                let _ = self.evt_tx.send(event);
            }
            Err(error) => {
                let _ = self.evt_tx.send(RouterEvent::Error(error));
            }
        }
    }

    pub(super) fn port_pairs(&self, node_id: u32) -> Result<Vec<(u32, u32)>, RouterError> {
        if self.sink_input_ports.len() < 2 {
            return Err(RouterError::SinkPortsUnavailable);
        }
        let source = self
            .known_sources
            .get(&node_id)
            .filter(|s| s.output_ports.len() >= 2)
            .ok_or(RouterError::SourcePortsUnavailable { node_id })?;
        Ok(source
            .output_ports
            .iter()
            .copied()
            .zip(self.sink_input_ports.iter().copied())
            .take(2)
            .collect())
    }
}

fn create_links(core: &pipewire::core::CoreRc, pairs: &[(u32, u32)]) -> LinkResult {
    pairs
        .iter()
        .map(|&(src_port, sink_port)| {
            let props = pipewire::properties::properties! {
                "link.output.port" => src_port.to_string(),
                "link.input.port" => sink_port.to_string(),
                "object.linger" => "false",
            };
            core.create_object::<pipewire::link::Link>("link-factory", &props)
                .map_err(|source| RouterError::LinkCreation {
                    src_port,
                    sink_port,
                    source,
                })
        })
        .collect()
}
