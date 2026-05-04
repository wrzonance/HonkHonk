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
    _cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
) -> Result<(), AudioError> {
    let _ = evt_tx.send(AudioEvent::Error("not implemented".into()));
    Ok(())
}
