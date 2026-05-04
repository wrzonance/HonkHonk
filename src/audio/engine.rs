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

    let mainloop_quit = mainloop.clone();
    let _cmd_listener = cmd_rx.attach(mainloop.loop_(), move |cmd| match cmd {
        AudioCommand::Shutdown => mainloop_quit.quit(),
    });

    let _ = evt_tx.send(AudioEvent::Ready);
    mainloop.run();

    Ok(())
}
