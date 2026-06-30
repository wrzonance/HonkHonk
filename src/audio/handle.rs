//! The app-facing audio command/event handle, split out of `engine.rs` to keep
//! that module within budget. Owns the pipewire command sender and the engine
//! event receiver; the engine thread itself lives in `engine.rs`.

use std::sync::mpsc;

use super::engine::{AudioCommand, AudioEvent};

pub struct AudioHandle {
    cmd_tx: pipewire::channel::Sender<AudioCommand>,
    evt_rx: mpsc::Receiver<AudioEvent>,
    // Test-only tap recording every command sent through this handle. The
    // pipewire command channel can't be drained synchronously, so this is how
    // unit tests assert the engine command boundary (e.g. exactly one `Play`
    // per cold press, a `StopVoice` on library reconcile).
    #[cfg(test)]
    sent: std::sync::Arc<std::sync::Mutex<Vec<AudioCommand>>>,
}

impl AudioHandle {
    /// Wraps an engine's command sender and event receiver. The single
    /// constructor — both `spawn` (real engine) and `test_handle` (no thread)
    /// route through here, so the test command tap is initialised in one place.
    pub(crate) fn from_parts(
        cmd_tx: pipewire::channel::Sender<AudioCommand>,
        evt_rx: mpsc::Receiver<AudioEvent>,
    ) -> Self {
        Self {
            cmd_tx,
            evt_rx,
            #[cfg(test)]
            sent: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn try_recv(&self) -> Option<AudioEvent> {
        self.evt_rx.try_recv().ok()
    }

    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<AudioEvent> {
        self.evt_rx.recv_timeout(timeout).ok()
    }

    pub fn send(&self, cmd: AudioCommand) {
        #[cfg(test)]
        self.sent
            .lock()
            .expect("audio command tap poisoned")
            .push(cmd.clone());
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(AudioCommand::Shutdown);
    }
}

#[cfg(test)]
impl AudioHandle {
    /// Ordered snapshot of every command sent through this handle. Test-only
    /// tap for asserting the engine command boundary.
    pub(crate) fn sent_commands(&self) -> Vec<AudioCommand> {
        self.sent
            .lock()
            .expect("audio command tap poisoned")
            .clone()
    }
}

/// Build an `AudioHandle` whose event channel is fed by the returned sender,
/// without spawning an engine thread. Lets app-level tests enqueue
/// `AudioEvent`s and observe how the UI drains them.
#[cfg(test)]
pub(crate) fn test_handle() -> (AudioHandle, mpsc::Sender<AudioEvent>) {
    let (cmd_tx, _cmd_rx) = pipewire::channel::channel::<AudioCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>();
    (AudioHandle::from_parts(cmd_tx, evt_rx), evt_tx)
}
