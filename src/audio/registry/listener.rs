use super::*;

struct ListenerState {
    state: Rc<RefCell<RegistryState>>,
    mic_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
    other_links: Rc<RefCell<Vec<pipewire::link::Link>>>,
    core: pipewire::core::CoreRc,
    cfg: RegistryConfig,
}

impl ListenerState {
    fn global(
        &self,
        global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
    ) {
        let mut state = self.state.borrow_mut();
        let change = handle_registry_global(global, &mut state);
        self.publish_ports(&state);
        if self.cfg.mic_passthrough.get() {
            try_create_mic_links(
                &mut state,
                &self.core,
                &mut self.mic_links.borrow_mut(),
                &self.cfg.evt_tx,
            );
        }
        try_create_monitor_links(&mut state, &self.core, &mut self.other_links.borrow_mut());
        match change {
            DeviceChange::Outputs => {
                let _ = self
                    .cfg
                    .evt_tx
                    .send(AudioEvent::OutputDevicesChanged(sink_names(&state)));
            }
            DeviceChange::Inputs => {
                let _ = self
                    .cfg
                    .evt_tx
                    .send(AudioEvent::InputDevicesChanged(source_names(&state)));
            }
            DeviceChange::None => {}
        }
    }

    fn remove(&self, id: u32) {
        let mut state = self.state.borrow_mut();
        let inputs = state.input_sources.len();
        let outputs = state.output_sinks.len();
        if state.mic_node_id == Some(id)
            || state.sink_node_id == Some(id)
            || state.mic_output_ports.contains(&id)
            || state.sink_input_ports.contains(&id)
        {
            drop_mic_links(&mut state, &mut self.mic_links.borrow_mut());
        }
        state.remove_global(id);
        self.publish_ports(&state);
        if outputs != state.output_sinks.len() {
            let _ = self
                .cfg
                .evt_tx
                .send(AudioEvent::OutputDevicesChanged(sink_names(&state)));
        }
        if inputs != state.input_sources.len() {
            let _ = self
                .cfg
                .evt_tx
                .send(AudioEvent::InputDevicesChanged(source_names(&state)));
        }
    }

    fn publish_ports(&self, state: &RegistryState) {
        self.cfg.shared_sink_id.set(state.sink_node_id);
        *self.cfg.shared_sink_ports.borrow_mut() = state.sink_input_ports.clone();
    }
}

pub fn setup(
    core: &pipewire::core::CoreRc,
    cfg: RegistryConfig,
) -> Result<RegistryGuard, AudioError> {
    let registry = core
        .get_registry_rc()
        .map_err(|e| AudioError::PipeWireInit(format!("registry: {e}")))?;
    let state = Rc::new(RefCell::new(RegistryState {
        graph: cfg.graph.clone(),
        preferred_source_name: sanitize_preferred_source(cfg.default_source_name.clone()),
        ..Default::default()
    }));
    let shared = Rc::new(ListenerState {
        state: state.clone(),
        mic_links: Rc::default(),
        other_links: Rc::default(),
        core: core.clone(),
        cfg,
    });
    let global = shared.clone();
    let remove = shared.clone();
    let listener = registry
        .add_listener_local()
        .global(move |object| global.global(object))
        .global_remove(move |id| remove.remove(id))
        .register();
    Ok(RegistryGuard {
        _registry: registry,
        _listener: listener,
        _other_links: shared.other_links.clone(),
        mic_links: shared.mic_links.clone(),
        state,
        mic_passthrough: shared.cfg.mic_passthrough.clone(),
        core: core.clone(),
        evt_tx: shared.cfg.evt_tx.clone(),
    })
}
