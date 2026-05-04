use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use super::error::AudioError;

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SINK_DESCRIPTION: &str = "HonkHonk Mix";

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum AudioEvent {
    Ready,
    Error(String),
}

pub struct AudioHandle {
    cmd_tx: pipewire::channel::Sender<AudioCommand>,
    evt_rx: mpsc::Receiver<AudioEvent>,
}

impl AudioHandle {
    pub fn try_recv(&self) -> Option<AudioEvent> {
        self.evt_rx.try_recv().ok()
    }

    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<AudioEvent> {
        self.evt_rx.recv_timeout(timeout).ok()
    }

    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(AudioCommand::Shutdown);
    }
}

pub fn spawn() -> Result<AudioHandle, AudioError> {
    let (cmd_tx, cmd_rx) = pipewire::channel::channel::<AudioCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>();

    std::thread::Builder::new()
        .name("honkhonk-pw".into())
        .spawn(move || {
            if let Err(e) = run_engine(cmd_rx, evt_tx.clone()) {
                let _ = evt_tx.send(AudioEvent::Error(e.to_string()));
            }
        })
        .map_err(AudioError::ThreadSpawn)?;

    Ok(AudioHandle { cmd_tx, evt_rx })
}

#[derive(Default)]
struct RegistryState {
    sink_node_id: Option<u32>,
    sink_input_ports: Vec<u32>,
    source_node_id: Option<u32>,
    source_output_ports: Vec<u32>,
    links_created: bool,
}

fn try_create_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
) {
    if state.links_created {
        return;
    }
    if state.sink_input_ports.len() < 2 || state.source_output_ports.len() < 2 {
        return;
    }

    state.links_created = true;

    for (src_port, sink_port) in state
        .source_output_ports
        .iter()
        .zip(state.sink_input_ports.iter())
    {
        let link_props = pipewire::properties::properties! {
            "link.output.port" => src_port.to_string(),
            "link.input.port" => sink_port.to_string(),
            "object.linger" => "false",
        };
        match core.create_object::<pipewire::link::Link>("link-factory", &link_props) {
            Ok(link) => links.push(link),
            Err(e) => eprintln!("honkhonk: failed to create mic passthrough link: {e}"),
        }
    }
}

fn run_engine(
    cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
) -> Result<(), AudioError> {
    let mainloop = pipewire::main_loop::MainLoopRc::new(None)
        .map_err(|e| AudioError::PipeWireInit(format!("main loop: {e}")))?;

    let context = pipewire::context::ContextRc::new(&mainloop, None)
        .map_err(|e| AudioError::PipeWireInit(format!("context: {e}")))?;

    let core = context
        .connect_rc(None)
        .map_err(|e| AudioError::PipeWireInit(format!("core connect: {e}")))?;

    let sink_props = pipewire::properties::properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => SINK_NODE_NAME,
        "node.description" => SINK_DESCRIPTION,
        "media.class" => "Audio/Sink/Virtual",
        "audio.position" => "[FL,FR]",
        "object.linger" => "false",
    };
    let _sink: pipewire::node::Node = core
        .create_object("adapter", &sink_props)
        .map_err(|e| AudioError::VirtualSinkCreation(e.to_string()))?;

    let state = Rc::new(RefCell::new(RegistryState::default()));
    let mic_links: Rc<RefCell<Vec<pipewire::link::Link>>> = Rc::new(RefCell::new(Vec::new()));

    let registry = core
        .get_registry()
        .map_err(|e| AudioError::PipeWireInit(format!("registry: {e}")))?;

    let state_ref = state.clone();
    let links_ref = mic_links.clone();
    let core_ref = core.clone();
    let _reg_listener = registry
        .add_listener_local()
        .global(move |global| {
            let props = match global.props {
                Some(p) => p,
                None => return,
            };

            let mut s = state_ref.borrow_mut();

            match global.type_ {
                pipewire::types::ObjectType::Node => {
                    let name = props.get("node.name").unwrap_or("");
                    let class = props.get("media.class").unwrap_or("");

                    if name == SINK_NODE_NAME {
                        s.sink_node_id = Some(global.id);
                    } else if class == "Audio/Source" && s.source_node_id.is_none() {
                        s.source_node_id = Some(global.id);
                    }
                }
                pipewire::types::ObjectType::Port => {
                    let node_id: u32 = props
                        .get("node.id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let direction = props.get("port.direction").unwrap_or("");

                    if Some(node_id) == s.sink_node_id && direction == "in" {
                        s.sink_input_ports.push(global.id);
                    } else if Some(node_id) == s.source_node_id && direction == "out" {
                        s.source_output_ports.push(global.id);
                    }
                }
                _ => {}
            }

            let mut link_store = links_ref.borrow_mut();
            try_create_links(&mut s, &core_ref, &mut link_store);
        })
        .register();

    let mainloop_quit = mainloop.clone();
    let _cmd_listener = cmd_rx.attach(mainloop.loop_(), move |cmd| match cmd {
        AudioCommand::Shutdown => mainloop_quit.quit(),
    });

    let _ = evt_tx.send(AudioEvent::Ready);
    mainloop.run();

    drop(_reg_listener);
    drop(mic_links);

    Ok(())
}
