use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;

use super::error::AudioError;
use super::playback::{self, PlaybackState};
use super::registry::setup_registry_listener;
use super::streams;

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SINK_DESCRIPTION: &str = "HonkHonk Mix";
const SOURCE_NODE_NAME: &str = "honkhonk-mic";
const SOURCE_DESCRIPTION: &str = "HonkHonk Mic";

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
    SetMicPassthrough(bool),
    SetMicPassthroughLevel(f32),
    SetMonitorDevice(Option<String>),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Ready,
    PlaybackStarted { sound_id: String },
    PlaybackFinished { sound_id: String },
    Progress(f32),
    Error(String),
    OutputDevicesChanged(Vec<(String, String)>),
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

pub fn spawn(
    initial_passthrough: bool,
    initial_monitor_device: Option<String>,
) -> Result<AudioHandle, AudioError> {
    let (cmd_tx, cmd_rx) = pipewire::channel::channel::<AudioCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>();

    std::thread::Builder::new()
        .name("honkhonk-pw".into())
        .spawn(move || {
            let default_source = query_default_source_name();
            if let Err(e) = run_engine(
                cmd_rx,
                evt_tx.clone(),
                default_source,
                initial_passthrough,
                initial_monitor_device,
            ) {
                let _ = evt_tx.send(AudioEvent::Error(e.to_string()));
            }
        })
        .map_err(AudioError::ThreadSpawn)?;

    Ok(AudioHandle { cmd_tx, evt_rx })
}

fn query_default_source_name() -> Option<String> {
    let output = std::process::Command::new("pw-metadata")
        .args(["0", "default.audio.source"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split("\"name\":\"")
        .nth(1)?
        .split('"')
        .next()
        .map(String::from)
}

struct ActivePlayback {
    sound_id: String,
    sink_state: Rc<RefCell<PlaybackState>>,
    monitor_state: Rc<RefCell<PlaybackState>>,
    _sink_stream: playback::PlaybackStream,
    monitor_stream: Option<playback::PlaybackStream>,
}

struct EngineCtx {
    registry_sink_id: Rc<Cell<Option<u32>>>,
    core: pipewire::core::CoreRc,
    active: Rc<RefCell<Option<ActivePlayback>>>,
    evt_tx: mpsc::Sender<AudioEvent>,
    engine_volume: Rc<Cell<f32>>,
    monitor_target: Rc<RefCell<Option<String>>>,
}

fn create_virtual_sink(core: &pipewire::core::CoreRc) -> Result<pipewire::node::Node, AudioError> {
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

fn create_virtual_source(
    core: &pipewire::core::CoreRc,
) -> Result<pipewire::node::Node, AudioError> {
    let source_props = pipewire::properties::properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => SOURCE_NODE_NAME,
        "node.description" => SOURCE_DESCRIPTION,
        "media.class" => "Audio/Source/Virtual",
        "audio.position" => "[FL,FR]",
        "object.linger" => "false",
    };
    core.create_object("adapter", &source_props)
        .map_err(|e| AudioError::VirtualSourceCreation(e.to_string()))
}

fn setup_completion_timer(
    pw_loop: &pipewire::loop_::Loop,
    active_timer: Rc<RefCell<Option<ActivePlayback>>>,
    evt_tx_timer: mpsc::Sender<AudioEvent>,
) -> Result<pipewire::loop_::TimerSource<'_>, AudioError> {
    let timer = pw_loop.add_timer(move |_expirations| {
        let (done, progress) = {
            let borrow = active_timer.borrow();
            if let Some(ref ap) = *borrow {
                let sink_done = !ap.sink_state.borrow().is_active();
                let mon_done = !ap.monitor_state.borrow().is_active();
                let p = ap.sink_state.borrow().progress();
                (sink_done && mon_done, Some(p))
            } else {
                (false, None)
            }
        };

        if let Some(p) = progress {
            let _ = evt_tx_timer.send(AudioEvent::Progress(p));
        }

        if done {
            if let Some(ap) = active_timer.borrow_mut().take() {
                let _ = evt_tx_timer.send(AudioEvent::PlaybackFinished {
                    sound_id: ap.sound_id,
                });
            }
        }
    });

    if let Err(e) = timer
        .update_timer(
            Some(std::time::Duration::from_millis(100)),
            Some(std::time::Duration::from_millis(100)),
        )
        .into_result()
    {
        return Err(AudioError::PipeWireInit(format!(
            "arm completion timer: {e}"
        )));
    }

    Ok(timer)
}

fn run_engine(
    cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
    default_source: Option<String>,
    initial_passthrough: bool,
    initial_monitor_device: Option<String>,
) -> Result<(), AudioError> {
    let mic_passthrough: Rc<Cell<bool>> = Rc::new(Cell::new(initial_passthrough));
    let monitor_target: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(initial_monitor_device));
    let mainloop = pipewire::main_loop::MainLoopRc::new(None)
        .map_err(|e| AudioError::PipeWireInit(format!("main loop: {e}")))?;

    let context = pipewire::context::ContextRc::new(&mainloop, None)
        .map_err(|e| AudioError::PipeWireInit(format!("context: {e}")))?;

    let core = context
        .connect_rc(None)
        .map_err(|e| AudioError::PipeWireInit(format!("core connect: {e}")))?;

    let _sink = create_virtual_sink(&core)?;
    let _source = create_virtual_source(&core)?;

    let registry_sink_id: Rc<Cell<Option<u32>>> = Rc::new(Cell::new(None));
    let registry_guard = setup_registry_listener(
        &core,
        registry_sink_id.clone(),
        default_source,
        mic_passthrough,
        evt_tx.clone(),
    )?;

    // External-stream observer (issue #26). Held to end-of-scope so its
    // Drop detaches the registry listener at shutdown.
    let self_pid = std::process::id();
    let (_stream_watcher, stream_rx) = streams::start(&core, self_pid)?;
    std::thread::Builder::new()
        .name("honkhonk-stream-drain".into())
        .spawn(move || {
            while let Ok(event) = stream_rx.recv() {
                eprintln!("honkhonk stream: {event:?}");
            }
        })
        .map_err(AudioError::ThreadSpawn)?;

    let active: Rc<RefCell<Option<ActivePlayback>>> = Rc::new(RefCell::new(None));
    let engine_volume: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));

    let ctx = EngineCtx {
        registry_sink_id,
        core: core.clone(),
        active: active.clone(),
        evt_tx: evt_tx.clone(),
        engine_volume,
        monitor_target,
    };

    let active_timer = active;
    let evt_tx_timer = evt_tx.clone();
    let pw_loop = mainloop.loop_();
    let _completion_timer = setup_completion_timer(pw_loop, active_timer, evt_tx_timer)?;

    let mainloop_quit = mainloop.clone();
    let _cmd_listener = cmd_rx.attach(mainloop.loop_(), move |cmd| match cmd {
        AudioCommand::Play {
            sound_id,
            samples,
            sample_rate,
            channels,
        } => {
            handle_play(&ctx, sound_id, samples, sample_rate, channels);
        }
        AudioCommand::Stop => {
            let prev = ctx.active.borrow_mut().take();
            if let Some(ap) = prev {
                let _ = ctx.evt_tx.send(AudioEvent::PlaybackFinished {
                    sound_id: ap.sound_id,
                });
            }
        }
        AudioCommand::SetVolume(v) => {
            ctx.engine_volume.set(v.clamp(0.0, 1.0));
            if let Some(ref ap) = *ctx.active.borrow() {
                ap.sink_state.borrow_mut().set_volume(v);
                ap.monitor_state.borrow_mut().set_volume(v);
            }
        }
        AudioCommand::SetMicPassthrough(v) => {
            registry_guard.apply_passthrough(v);
        }
        AudioCommand::SetMicPassthroughLevel(_) => {}
        AudioCommand::SetMonitorDevice(target) => {
            *ctx.monitor_target.borrow_mut() = target;
            rebuild_monitor_stream(&ctx);
        }
        AudioCommand::Shutdown => {
            let _ = ctx.active.borrow_mut().take();
            mainloop_quit.quit();
        }
    });

    let _ = evt_tx.send(AudioEvent::Ready);
    mainloop.run();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_command_set_mic_passthrough_is_constructible() {
        let _ = AudioCommand::SetMicPassthrough(true);
        let _ = AudioCommand::SetMicPassthrough(false);
    }

    #[test]
    fn audio_command_set_mic_passthrough_level_is_constructible() {
        let _ = AudioCommand::SetMicPassthroughLevel(0.5);
    }

    #[test]
    fn audio_command_set_monitor_device_none_is_constructible() {
        let _ = AudioCommand::SetMonitorDevice(None);
    }

    #[test]
    fn audio_command_set_monitor_device_some_is_constructible() {
        let _ = AudioCommand::SetMonitorDevice(Some("alsa_output.pci-test".into()));
    }

    #[test]
    fn audio_event_output_devices_changed_is_constructible() {
        let _ = AudioEvent::OutputDevicesChanged(vec![(
            "alsa_output.pci-test".into(),
            "Built-in Audio".into(),
        )]);
    }
}

fn rebuild_monitor_stream(ctx: &EngineCtx) {
    if let Some(ref mut ap) = *ctx.active.borrow_mut() {
        let (rate, ch) = {
            let ms = ap.monitor_state.borrow();
            (ms.sample_rate(), ms.channels())
        };
        let target = ctx.monitor_target.borrow().clone();
        match playback::create_monitor_stream(
            ctx.core.clone(),
            ap.monitor_state.clone(),
            rate,
            ch,
            target.as_deref(),
        ) {
            Ok(stream) => ap.monitor_stream = Some(stream),
            Err(e) => {
                ap.monitor_stream = None;
                ap.monitor_state.borrow_mut().stop();
                let _ = ctx
                    .evt_tx
                    .send(AudioEvent::Error(format!("monitor stream rebuild: {e}")));
            }
        }
    }
}

fn handle_play(
    ctx: &EngineCtx,
    sound_id: String,
    samples: Arc<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
) {
    if ctx.registry_sink_id.get().is_none() {
        let _ = ctx
            .evt_tx
            .send(AudioEvent::Error("virtual sink not yet registered".into()));
        return;
    }

    let prev = ctx.active.borrow_mut().take();
    if let Some(ap) = prev {
        let _ = ctx.evt_tx.send(AudioEvent::PlaybackFinished {
            sound_id: ap.sound_id,
        });
    }

    let vol = ctx.engine_volume.get();
    let sink_state = Rc::new(RefCell::new(PlaybackState::with_volume(vol)));
    sink_state
        .borrow_mut()
        .start(sound_id.clone(), samples.clone(), sample_rate, channels);

    let mon_state = Rc::new(RefCell::new(PlaybackState::with_volume(vol)));
    mon_state
        .borrow_mut()
        .start(sound_id.clone(), samples, sample_rate, channels);

    let target = ctx.monitor_target.borrow().clone();
    let sink_stream = playback::create_sink_stream(
        ctx.core.clone(),
        sink_state.clone(),
        SINK_NODE_NAME,
        sample_rate,
        channels,
    );
    let mon_stream = playback::create_monitor_stream(
        ctx.core.clone(),
        mon_state.clone(),
        sample_rate,
        channels,
        target.as_deref(),
    );

    let sink_s = match sink_stream {
        Ok(s) => s,
        Err(e) => {
            let _ = ctx.evt_tx.send(AudioEvent::Error(e.to_string()));
            return;
        }
    };
    let monitor_stream = match mon_stream {
        Ok(s) => Some(s),
        Err(e) => {
            mon_state.borrow_mut().stop();
            let _ = ctx.evt_tx.send(AudioEvent::Error(format!(
                "monitor stream unavailable: {e}"
            )));
            None
        }
    };
    *ctx.active.borrow_mut() = Some(ActivePlayback {
        sound_id: sound_id.clone(),
        sink_state,
        monitor_state: mon_state,
        _sink_stream: sink_s,
        monitor_stream,
    });
    let _ = ctx.evt_tx.send(AudioEvent::PlaybackStarted { sound_id });
}
