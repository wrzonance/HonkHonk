use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;

use super::error::AudioError;
use super::playback::{self, PlaybackState};

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SINK_DESCRIPTION: &str = "HonkHonk Mix";

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play {
        sound_id: String,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        channels: u16,
    },
    Stop,
    SetVolume(f32),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Ready,
    PlaybackStarted { sound_id: String },
    PlaybackFinished { sound_id: String },
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

    pub fn send(&self, cmd: AudioCommand) {
        let _ = self.cmd_tx.send(cmd);
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

struct ActivePlayback {
    sound_id: String,
    sink_state: Rc<RefCell<PlaybackState>>,
    monitor_state: Rc<RefCell<PlaybackState>>,
    _sink_stream: playback::PlaybackStream,
    _monitor_stream: playback::PlaybackStream,
}

fn try_create_links(
    state: &mut RegistryState,
    core: &pipewire::core::Core,
    links: &mut Vec<pipewire::link::Link>,
) {
    if state.links_created {
        return;
    }
    if state.sink_input_ports.is_empty() || state.source_output_ports.is_empty() {
        return;
    }

    let mut all_ok = true;
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
            Err(e) => {
                eprintln!("honkhonk: failed to create mic passthrough link: {e}");
                all_ok = false;
            }
        }
    }

    state.links_created = all_ok;
}

fn create_virtual_sink(
    core: &pipewire::core::CoreRc,
) -> Result<pipewire::node::Node, AudioError> {
    let sink_props = pipewire::properties::properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => SINK_NODE_NAME,
        "node.description" => SINK_DESCRIPTION,
        "media.class" => "Audio/Sink/Virtual",
        "audio.position" => "[FL,FR]",
        "object.linger" => "false",
    };
    core.create_object("adapter", &sink_props)
        .map_err(|e| AudioError::VirtualSinkCreation(e.to_string()))
}

fn handle_registry_global(
    global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
    state: &mut RegistryState,
) {
    let props = match global.props {
        Some(p) => p,
        None => return,
    };

    match global.type_ {
        pipewire::types::ObjectType::Node => {
            let name = props.get("node.name").unwrap_or("");
            let class = props.get("media.class").unwrap_or("");

            if name == SINK_NODE_NAME {
                state.sink_node_id = Some(global.id);
            } else if class == "Audio/Source" && state.source_node_id.is_none() {
                state.source_node_id = Some(global.id);
            }
        }
        pipewire::types::ObjectType::Port => {
            let node_id: u32 = props
                .get("node.id")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let direction = props.get("port.direction").unwrap_or("");

            if Some(node_id) == state.sink_node_id && direction == "in" {
                state.sink_input_ports.push(global.id);
            } else if Some(node_id) == state.source_node_id && direction == "out" {
                state.source_output_ports.push(global.id);
            }
        }
        _ => {}
    }
}

struct RegistryGuard<'a> {
    _registry: pipewire::registry::RegistryBox<'a>,
    _listener: pipewire::registry::Listener,
    _links: Rc<RefCell<Vec<pipewire::link::Link>>>,
}

fn setup_registry_listener(
    core: &pipewire::core::CoreRc,
    shared_sink_id: Rc<Cell<Option<u32>>>,
) -> Result<RegistryGuard<'_>, AudioError> {
    let state = Rc::new(RefCell::new(RegistryState::default()));
    let mic_links: Rc<RefCell<Vec<pipewire::link::Link>>> =
        Rc::new(RefCell::new(Vec::new()));

    let registry = core
        .get_registry()
        .map_err(|e| AudioError::PipeWireInit(format!("registry: {e}")))?;

    let state_ref = state.clone();
    let links_ref = mic_links.clone();
    let core_ref = core.clone();
    let listener = registry
        .add_listener_local()
        .global(move |global| {
            let mut s = state_ref.borrow_mut();
            handle_registry_global(global, &mut s);
            if let Some(id) = s.sink_node_id {
                shared_sink_id.set(Some(id));
            }
            let mut link_store = links_ref.borrow_mut();
            try_create_links(&mut s, &core_ref, &mut link_store);
        })
        .register();

    Ok(RegistryGuard {
        _registry: registry,
        _listener: listener,
        _links: mic_links,
    })
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

    let _sink = create_virtual_sink(&core)?;

    let registry_sink_id: Rc<Cell<Option<u32>>> =
        Rc::new(Cell::new(None));
    let _registry_guard =
        setup_registry_listener(&core, registry_sink_id.clone())?;

    let active: Rc<RefCell<Option<ActivePlayback>>> =
        Rc::new(RefCell::new(None));

    let mainloop_quit = mainloop.clone();
    let registry_sink_id_cmd = registry_sink_id;
    let active_cmd = active;
    let core_cmd = core.clone();
    let evt_tx_cmd = evt_tx.clone();

    let _cmd_listener =
        cmd_rx.attach(mainloop.loop_(), move |cmd| match cmd {
            AudioCommand::Play {
                sound_id,
                samples,
                sample_rate,
                channels,
            } => {
                handle_play(
                    &registry_sink_id_cmd,
                    &core_cmd,
                    &active_cmd,
                    &evt_tx_cmd,
                    sound_id,
                    samples,
                    sample_rate,
                    channels,
                );
            }
            AudioCommand::Stop => {
                let prev = active_cmd.borrow_mut().take();
                if let Some(ap) = prev {
                    let _ = evt_tx_cmd.send(
                        AudioEvent::PlaybackFinished {
                            sound_id: ap.sound_id,
                        },
                    );
                }
            }
            AudioCommand::SetVolume(v) => {
                if let Some(ref ap) = *active_cmd.borrow() {
                    ap.sink_state.borrow_mut().set_volume(v);
                    ap.monitor_state.borrow_mut().set_volume(v);
                }
            }
            AudioCommand::Shutdown => {
                // Drop active playback before quitting the loop.
                let _ = active_cmd.borrow_mut().take();
                mainloop_quit.quit();
            }
        });

    let _ = evt_tx.send(AudioEvent::Ready);
    mainloop.run();

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_play(
    registry_sink_id: &Rc<Cell<Option<u32>>>,
    core: &pipewire::core::CoreRc,
    active: &Rc<RefCell<Option<ActivePlayback>>>,
    evt_tx: &mpsc::Sender<AudioEvent>,
    sound_id: String,
    samples: Arc<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
) {
    let sink_id = match registry_sink_id.get() {
        Some(id) => id,
        None => {
            let _ = evt_tx.send(AudioEvent::Error(
                "virtual sink not yet registered".into(),
            ));
            return;
        }
    };

    // Stop any existing playback before starting a new one.
    let prev = active.borrow_mut().take();
    if let Some(ap) = prev {
        let _ = evt_tx.send(AudioEvent::PlaybackFinished {
            sound_id: ap.sound_id,
        });
    }

    // Two independent PlaybackState instances share the same samples
    // so their cursors advance independently without contention.
    let sink_state = Rc::new(RefCell::new(PlaybackState::new()));
    sink_state.borrow_mut().start(
        sound_id.clone(),
        samples.clone(),
        sample_rate,
        channels,
    );

    let mon_state = Rc::new(RefCell::new(PlaybackState::new()));
    mon_state.borrow_mut().start(
        sound_id.clone(),
        samples,
        sample_rate,
        channels,
    );

    let sink_stream = playback::create_sink_stream(
        core.clone(),
        sink_state.clone(),
        sink_id,
        sample_rate,
        channels,
    );
    let mon_stream = playback::create_monitor_stream(
        core.clone(),
        mon_state.clone(),
        sample_rate,
        channels,
    );

    match (sink_stream, mon_stream) {
        (Ok(sink_s), Ok(mon_s)) => {
            *active.borrow_mut() = Some(ActivePlayback {
                sound_id: sound_id.clone(),
                sink_state,
                monitor_state: mon_state,
                _sink_stream: sink_s,
                _monitor_stream: mon_s,
            });
            let _ = evt_tx.send(AudioEvent::PlaybackStarted {
                sound_id,
            });
        }
        (Err(e), _) | (_, Err(e)) => {
            let _ =
                evt_tx.send(AudioEvent::Error(e.to_string()));
        }
    }
}
