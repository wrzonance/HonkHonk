use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;

use super::confd;
use super::error::{AudioError, EngineErrorEvent};
use super::playback::{self, PlaybackState};
use super::registry::{setup_registry_listener, RegistryConfig};
use super::router::{Router, RouterEvent};
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
        /// Monotonic token identifying this specific play. Echoed back on the
        /// matching `PlaybackFinished` so the app can tell a genuine end from the
        /// stale `Finished` emitted for a voice that was immediately superseded by
        /// a re-press of the same sound (#149).
        generation: u64,
        /// Per-sound volume multiplier, applied alongside the master volume in
        /// `PlaybackState`. Lets the app send the canonical (pre-volume) PCM Arc
        /// without an O(n) copy per play (#151).
        volume: f32,
    },
    Stop,
    SetVolume(f32),
    SetMicPassthrough(bool),
    SetMicPassthroughLevel(f32),
    SetMonitorDevice(Option<String>),
    /// Select the microphone (input) source to pass through. `None` = Auto
    /// (system default, excluding HonkHonk's own virtual source).
    SetInputDevice(Option<String>),
    Router(super::router::RouterCommand),
    Shutdown,
    /// Set bypass state for the effect at `index` in the mixer chain.
    SetEffectBypass {
        index: usize,
        bypass: bool,
    },
    /// Set a parameter on the effect at `index`.
    SetEffectParam {
        index: usize,
        param: String,
        value: f32,
    },
    /// Set the chain-level wet/dry mix.
    SetEffectWetDry(f32),
    /// Set chain-level bypass.
    SetEffectChainBypass(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    Ready,
    PlaybackStarted {
        sound_id: String,
    },
    PlaybackFinished {
        sound_id: String,
        /// Echoes the `generation` of the `Play` this voice came from, so a stale
        /// `Finished` for a superseded voice can be distinguished from a genuine
        /// end (#149).
        generation: u64,
    },
    Progress(f32),
    Error(EngineErrorEvent),
    OutputDevicesChanged(Vec<(String, String)>),
    /// The set of real microphone (input) sources changed; carries
    /// (node_name, display_name) for each, to populate the input-device picker.
    InputDevicesChanged(Vec<(String, String)>),
    /// Emitted once on a first run that created the source programmatically and
    /// wrote the per-user conf.d. The UI shows a one-time notice telling the
    /// user the "HonkHonk Mic" device now persists and to select it in
    /// Discord/OBS. Carries whether a new conf.d file was actually written.
    SourceFirstRun {
        confd_written: bool,
    },
    /// The effect chain's total latency changed (in samples).
    EffectsLatencyChanged(u32),
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

/// Build an `AudioHandle` whose event channel is fed by the returned sender,
/// without spawning an engine thread. Lets app-level tests enqueue
/// `AudioEvent`s and observe how the UI drains them.
#[cfg(test)]
pub(crate) fn test_handle() -> (AudioHandle, mpsc::Sender<AudioEvent>) {
    let (cmd_tx, _cmd_rx) = pipewire::channel::channel::<AudioCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>();
    (AudioHandle { cmd_tx, evt_rx }, evt_tx)
}

pub fn spawn(
    initial_passthrough: bool,
    initial_monitor_device: Option<String>,
    initial_input_device: Option<String>,
) -> Result<AudioHandle, AudioError> {
    let (cmd_tx, cmd_rx) = pipewire::channel::channel::<AudioCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>();

    std::thread::Builder::new()
        .name("honkhonk-pw".into())
        .spawn(move || {
            // An explicitly chosen input device wins; otherwise fall back to the
            // system default source (the registry sanitizes out our own mic).
            let preferred_source = initial_input_device.or_else(query_default_source_name);
            if let Err(e) = run_engine(
                cmd_rx,
                evt_tx.clone(),
                preferred_source,
                initial_passthrough,
                initial_monitor_device,
            ) {
                let _ = evt_tx.send(AudioEvent::Error(EngineErrorEvent::EngineInitialization {
                    detail: e.to_string(),
                }));
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
    /// The `generation` of the `Play` command that started this voice; echoed on
    /// its `PlaybackFinished` so the app can ignore a stale end (#149).
    generation: u64,
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
    mixer: Rc<RefCell<super::mixer::Mixer>>,
    router: Rc<RefCell<Router>>,
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

/// First-run decision: create the virtual source programmatically only when
/// no `honkhonk-mic` node already exists (i.e. no packaged/user conf.d has
/// declared it). When it already exists we reuse it and never recreate.
fn should_create_source(source_already_exists: bool) -> bool {
    !source_already_exists
}

/// Pure scan: does a `pw-dump` (JSON) or `pw-cli` text blob mention a node
/// whose `node.name` is our virtual source? Matches the quoted name token so a
/// substring like `honkhonk-mic-foo` does not false-positive. Tolerant of both
/// `pw-cli` form (`node.name = "honkhonk-mic"`) and `pw-dump` JSON form
/// (`"node.name": "honkhonk-mic",`).
fn source_present_in_dump(dump: &str) -> bool {
    let needle = format!("\"{SOURCE_NODE_NAME}\"");
    dump.lines().any(|line| {
        let l = line.trim().trim_start_matches('"');
        l.starts_with("node.name") && l.contains(&needle)
    })
}

/// Probe PipeWire (via `pw-dump`) for an existing `honkhonk-mic` node.
/// Returns `false` if the tool is missing or fails — the caller then falls
/// back to programmatic creation, which itself fails gracefully without PW.
fn source_already_exists() -> bool {
    std::process::Command::new("pw-dump")
        .output()
        .ok()
        .map(|o| source_present_in_dump(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or(false)
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
        // Lingering: the programmatically-created source survives app exit
        // (until reboot) as a first-run bridge until a packaged/user conf.d
        // takes effect. See ADR-004. The internal mixing sink stays linger=false.
        "object.linger" => "true",
    };
    core.create_object("adapter", &source_props)
        .map_err(|e| AudioError::VirtualSourceCreation(e.to_string()))
}

/// Write the per-user conf.d bridge, reporting failures as non-fatal events.
/// Returns whether a new file was written.
fn write_first_run_confd(evt_tx: &mpsc::Sender<AudioEvent>) -> bool {
    match confd::user_confd_dir() {
        Ok(dir) => confd::write_user_confd_in(&dir).unwrap_or_else(|e| {
            let _ = evt_tx.send(AudioEvent::Error(EngineErrorEvent::ConfdWrite {
                detail: e.to_string(),
            }));
            false
        }),
        Err(e) => {
            let _ = evt_tx.send(AudioEvent::Error(EngineErrorEvent::ConfdPath {
                detail: e.to_string(),
            }));
            false
        }
    }
}

/// Ensure the persistent virtual source exists (issue #49).
///
/// If a `honkhonk-mic` node already exists (packaged/user conf.d case) we reuse
/// it and create nothing — returns `None`. Otherwise (dev/unpackaged first run)
/// we create it programmatically (lingering), write the per-user conf.d bridge,
/// and emit `SourceFirstRun`. The returned `Node`, when `Some`, is held to
/// end-of-scope and is NEVER explicitly destroyed: a lingering node survives
/// app exit, and the conf.d bridge re-creates it next session regardless.
fn ensure_virtual_source(
    core: &pipewire::core::CoreRc,
    evt_tx: &mpsc::Sender<AudioEvent>,
) -> Result<Option<pipewire::node::Node>, AudioError> {
    if !should_create_source(source_already_exists()) {
        return Ok(None);
    }
    let node = create_virtual_source(core)?;
    let confd_written = write_first_run_confd(evt_tx);
    let _ = evt_tx.send(AudioEvent::SourceFirstRun { confd_written });
    Ok(Some(node))
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
                    generation: ap.generation,
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

/// Bootstrap the external-stream observer (issue #26).
///
/// Starts the `streams::start` watcher bound to the engine's PipeWire core.
/// Returns both the watcher (MUST be held to end-of-scope — dropping detaches
/// the registry listener) and the receiver for stream events, which the caller
/// attaches to the PipeWire main loop so the Router receives events on the
/// engine thread.
fn spawn_stream_watcher(
    core: &pipewire::core::CoreRc,
) -> Result<(streams::StreamWatcher, mpsc::Receiver<streams::StreamEvent>), AudioError> {
    let self_pid = std::process::id();
    let (stream_watcher, stream_rx) = streams::start(core, self_pid)?;
    Ok((stream_watcher, stream_rx))
}

fn run_engine(
    cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
    preferred_source: Option<String>,
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

    // Persistent virtual source (issue #49): reuse a conf.d-declared device if
    // present; otherwise create it programmatically (lingering) and write the
    // per-user conf.d as the persistence bridge for dev/unpackaged runs.
    let _source = ensure_virtual_source(&core, &evt_tx)?;

    // Shared sink input ports: updated by the registry listener (global() callback)
    // and read by the stream listener on every SourceAdded so the Router always
    // has the latest port list when it attempts to create links.
    let shared_sink_ports: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));

    let registry_sink_id: Rc<Cell<Option<u32>>> = Rc::new(Cell::new(None));
    let registry_guard = setup_registry_listener(
        &core,
        RegistryConfig {
            shared_sink_id: registry_sink_id.clone(),
            default_source_name: preferred_source,
            mic_passthrough,
            evt_tx: evt_tx.clone(),
            shared_sink_ports: shared_sink_ports.clone(),
        },
    )?;

    // External-stream observer (issue #26 / #27). The receiver is attached to
    // the PipeWire main loop so StreamEvents are dispatched on the engine thread
    // directly to the Router (no cross-thread handoff needed).
    let (_stream_watcher, stream_rx) = spawn_stream_watcher(&core)?;

    // Router (issue #27): persistent link router keyed by AppIdentity.
    // RouterEvents are drained on a daemon thread; future issues (#28) will
    // forward selected events to the UI via the AudioEvent channel.
    let (router_evt_tx, router_evt_rx) = mpsc::channel::<RouterEvent>();
    let router: Rc<RefCell<Router>> = Rc::new(RefCell::new(Router::new(router_evt_tx)));
    {
        std::thread::Builder::new()
            .name("honkhonk-router-drain".into())
            .spawn(move || {
                for event in router_evt_rx {
                    eprintln!("honkhonk router: {event:?}");
                }
            })
            .map_err(AudioError::ThreadSpawn)?;
    }

    // Drain StreamEvents from the stream watcher into the Router.
    // `stream_rx` is an `mpsc::Receiver` (not a PW channel receiver), so it
    // cannot be attached to the PW main loop directly. We poll it on a PW timer
    // that fires every 50 ms — low enough latency for routing, high enough
    // interval to avoid busy-spinning.
    let router_for_stream = router.clone();
    let core_for_stream = core.clone();
    let sink_ports_for_stream = shared_sink_ports.clone();
    let _stream_drain_timer = {
        let pw_loop_ref = mainloop.loop_();
        let timer = pw_loop_ref.add_timer(move |_| {
            use streams::StreamEvent;
            while let Ok(event) = stream_rx.try_recv() {
                match event {
                    StreamEvent::SourceAdded {
                        id,
                        app_name,
                        app_binary,
                        app_pid,
                        ..
                    } => {
                        let ports = sink_ports_for_stream.borrow().clone();
                        let mut r = router_for_stream.borrow_mut();
                        r.update_sink_ports(ports);
                        r.on_source_added(id, app_name, app_binary, app_pid);
                    }
                    StreamEvent::SourceRemoved { id } => {
                        router_for_stream.borrow_mut().on_source_removed(id);
                    }
                    StreamEvent::PortAdded {
                        id,
                        node_id,
                        channel,
                        direction,
                    } => {
                        router_for_stream
                            .borrow_mut()
                            .on_port_added(id, node_id, channel, direction);
                        // Attempt auto-reconnect on each port addition. Succeeds once
                        // enough ports exist (typically after FR port arrives).
                        router_for_stream
                            .borrow_mut()
                            .try_auto_reconnect(node_id, &core_for_stream);
                    }
                    StreamEvent::SourceUpdated { .. } | StreamEvent::PortRemoved { .. } => {}
                }
            }
        });
        if let Err(e) = timer
            .update_timer(
                Some(std::time::Duration::from_millis(50)),
                Some(std::time::Duration::from_millis(50)),
            )
            .into_result()
        {
            return Err(AudioError::PipeWireInit(format!(
                "arm stream-drain timer: {e}"
            )));
        }
        timer
    };

    let active: Rc<RefCell<Option<ActivePlayback>>> = Rc::new(RefCell::new(None));
    let engine_volume: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));
    let mixer = Rc::new(RefCell::new(super::mixer::Mixer::new(4096)));
    mixer.borrow_mut().install_default_chain(48_000)?;

    let ctx = EngineCtx {
        registry_sink_id,
        core: core.clone(),
        active: active.clone(),
        evt_tx: evt_tx.clone(),
        engine_volume,
        monitor_target,
        mixer,
        router: router.clone(),
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
            generation,
            volume,
        } => {
            handle_play(
                &ctx,
                PlayRequest {
                    sound_id,
                    samples,
                    sample_rate,
                    channels,
                    generation,
                    volume,
                },
            );
        }
        AudioCommand::Stop => {
            let prev = ctx.active.borrow_mut().take();
            if let Some(ap) = prev {
                let _ = ctx.evt_tx.send(AudioEvent::PlaybackFinished {
                    sound_id: ap.sound_id,
                    generation: ap.generation,
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
        AudioCommand::SetInputDevice(target) => {
            // Resolve runtime "Auto" the same way as startup: an explicit device
            // wins, otherwise follow the system default source (sanitized in the
            // registry). Keeps the picker's Auto consistent across startup and
            // live switches.
            let resolved = target.or_else(query_default_source_name);
            registry_guard.set_input_device(resolved);
        }
        AudioCommand::Router(cmd) => {
            use super::router::RouterCommand;
            let mut r = ctx.router.borrow_mut();
            match cmd {
                RouterCommand::RouteSource { source_node_id } => {
                    r.route_source(source_node_id, &ctx.core);
                }
                RouterCommand::UnrouteSource { source_node_id } => {
                    r.handle_command_unroute_source(source_node_id);
                }
                RouterCommand::UnrouteAll => {
                    r.handle_command_unroute_all();
                }
            }
        }
        AudioCommand::Shutdown => {
            let _ = ctx.active.borrow_mut().take();
            mainloop_quit.quit();
        }
        AudioCommand::SetEffectBypass { index, bypass } => {
            if let Err(e) = ctx.mixer.borrow_mut().chain_mut().set_bypass(index, bypass) {
                let _ = ctx
                    .evt_tx
                    .send(AudioEvent::Error(EngineErrorEvent::EffectBypass {
                        index,
                        detail: e.to_string(),
                    }));
            }
        }
        AudioCommand::SetEffectParam {
            index,
            param,
            value,
        } => {
            if let Err(e) = ctx
                .mixer
                .borrow_mut()
                .chain_mut()
                .set_param(index, &param, value)
            {
                let _ = ctx
                    .evt_tx
                    .send(AudioEvent::Error(EngineErrorEvent::EffectParam {
                        index,
                        param,
                        detail: e.to_string(),
                    }));
            }
        }
        AudioCommand::SetEffectWetDry(wet_dry) => {
            ctx.mixer.borrow_mut().chain_mut().set_wet_dry(wet_dry);
        }
        AudioCommand::SetEffectChainBypass(bypass) => {
            ctx.mixer.borrow_mut().chain_mut().set_chain_bypass(bypass);
        }
    });

    let _ = evt_tx.send(AudioEvent::Ready);
    mainloop.run();

    Ok(())
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
                let _ =
                    ctx.evt_tx
                        .send(AudioEvent::Error(EngineErrorEvent::MonitorStreamRebuild {
                            detail: e.to_string(),
                        }));
            }
        }
    }
}

/// Decoded PCM plus identity for a single play, bundled so `handle_play` stays
/// within the argument-count lint as fields accrete (e.g. `generation`, #149).
struct PlayRequest {
    sound_id: String,
    samples: Arc<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    generation: u64,
    volume: f32,
}

fn handle_play(ctx: &EngineCtx, req: PlayRequest) {
    let PlayRequest {
        sound_id,
        samples,
        sample_rate,
        channels,
        generation,
        volume,
    } = req;
    if ctx.registry_sink_id.get().is_none() {
        let _ = ctx.evt_tx.send(AudioEvent::Error(
            EngineErrorEvent::VirtualSinkNotRegistered,
        ));
        // Uphold the invariant that every Play yields one matching-generation
        // Finished. Without it the app's optimistic playing/playhead state for
        // this generation would stick forever, since the generation guard
        // ignores any other Finished (#149).
        let _ = ctx.evt_tx.send(AudioEvent::PlaybackFinished {
            sound_id,
            generation,
        });
        return;
    }

    let prev = ctx.active.borrow_mut().take();
    if let Some(ap) = prev {
        // The displaced voice carries its OWN (older) generation, so the app can
        // tell this "superseded" Finished from the genuine end of the new voice
        // when a sound is re-pressed while still playing (#149).
        let _ = ctx.evt_tx.send(AudioEvent::PlaybackFinished {
            sound_id: ap.sound_id,
            generation: ap.generation,
        });
    }

    let vol = ctx.engine_volume.get();
    let sink_state = Rc::new(RefCell::new(PlaybackState::with_volume(vol)));
    sink_state.borrow_mut().start(
        sound_id.clone(),
        samples.clone(),
        sample_rate,
        channels,
        volume,
    );

    let mon_state = Rc::new(RefCell::new(PlaybackState::with_volume(vol)));
    mon_state
        .borrow_mut()
        .start(sound_id.clone(), samples, sample_rate, channels, volume);

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
            let _ = ctx
                .evt_tx
                .send(AudioEvent::Error(EngineErrorEvent::SinkStreamCreation {
                    detail: e.to_string(),
                }));
            // Same invariant as the sink-not-registered path: signal that this
            // generation produced no playback so the app clears it rather than
            // getting stuck on a phantom playing state (#149). The displaced
            // voice (if any) was already taken above and dropped.
            let _ = ctx.evt_tx.send(AudioEvent::PlaybackFinished {
                sound_id,
                generation,
            });
            return;
        }
    };
    let monitor_stream = match mon_stream {
        Ok(s) => Some(s),
        Err(e) => {
            mon_state.borrow_mut().stop();
            let _ = ctx.evt_tx.send(AudioEvent::Error(
                EngineErrorEvent::MonitorStreamUnavailable {
                    detail: e.to_string(),
                },
            ));
            None
        }
    };
    *ctx.active.borrow_mut() = Some(ActivePlayback {
        sound_id: sound_id.clone(),
        generation,
        sink_state,
        monitor_state: mon_state,
        _sink_stream: sink_s,
        monitor_stream,
    });
    let _ = ctx.evt_tx.send(AudioEvent::PlaybackStarted { sound_id });
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
    fn audio_command_set_input_device_none_is_constructible() {
        let _ = AudioCommand::SetInputDevice(None);
    }

    #[test]
    fn audio_command_set_input_device_some_is_constructible() {
        let _ = AudioCommand::SetInputDevice(Some("alsa_input.pci-test".into()));
    }

    #[test]
    fn should_create_source_false_when_node_already_present() {
        assert!(!should_create_source(true));
    }

    #[test]
    fn parse_source_present_detects_honkhonk_mic() {
        let dump = r#"
        id 42, type PipeWire:Interface:Node/3
            node.name = "honkhonk-mic"
            media.class = "Audio/Source/Virtual"
        "#;
        assert!(source_present_in_dump(dump));
    }

    #[test]
    fn parse_source_present_detects_honkhonk_mic_pw_dump_json() {
        let dump = r#"
        {
          "props": {
            "node.name": "honkhonk-mic",
            "media.class": "Audio/Source/Virtual"
          }
        }
        "#;
        assert!(source_present_in_dump(dump));
    }

    #[test]
    fn parse_source_present_false_when_absent() {
        let dump = r#"
        id 7, type PipeWire:Interface:Node/3
            node.name = "alsa_input.pci-0000"
        "#;
        assert!(!source_present_in_dump(dump));
    }

    #[test]
    fn parse_source_present_false_on_empty() {
        assert!(!source_present_in_dump(""));
    }

    #[test]
    fn parse_source_present_false_on_substring_node_name() {
        // A different node whose name merely contains our name must not match.
        let dump = r#"node.name = "honkhonk-mic-monitor""#;
        assert!(!source_present_in_dump(dump));
    }

    #[test]
    fn should_create_source_true_when_node_absent() {
        assert!(should_create_source(false));
    }

    #[test]
    fn audio_event_source_first_run_is_constructible() {
        let _ = AudioEvent::SourceFirstRun {
            confd_written: true,
        };
        let _ = AudioEvent::SourceFirstRun {
            confd_written: false,
        };
    }

    #[test]
    fn audio_event_output_devices_changed_is_constructible() {
        let _ = AudioEvent::OutputDevicesChanged(vec![(
            "alsa_output.pci-test".into(),
            "Built-in Audio".into(),
        )]);
    }

    #[test]
    fn audio_command_set_effect_bypass_is_constructible() {
        let _ = AudioCommand::SetEffectBypass {
            index: 0,
            bypass: true,
        };
    }

    #[test]
    fn audio_command_set_effect_wet_dry_is_constructible() {
        let _ = AudioCommand::SetEffectWetDry(0.5);
    }

    #[test]
    fn audio_event_effects_latency_changed_is_constructible() {
        let _ = AudioEvent::EffectsLatencyChanged(512);
    }

    #[test]
    fn audio_command_router_variant_is_constructible() {
        use crate::audio::router::RouterCommand;
        let _ = AudioCommand::Router(RouterCommand::UnrouteAll);
        let _ = AudioCommand::Router(RouterCommand::RouteSource { source_node_id: 1 });
        let _ = AudioCommand::Router(RouterCommand::UnrouteSource { source_node_id: 1 });
    }
}
