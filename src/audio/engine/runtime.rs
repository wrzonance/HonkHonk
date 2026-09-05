use super::*;

#[allow(
    clippy::too_many_lines,
    reason = "PipeWire mainloop setup keeps Rc handles and guards alive for the engine lifetime"
)]
pub(super) fn run_engine(
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
                    tracing::debug!(?event, "router event");
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

    let voices: Rc<RefCell<VoicePool>> = Rc::new(RefCell::new(VoicePool::new()));
    let playback_streams: Rc<RefCell<PlaybackStreams>> =
        Rc::new(RefCell::new(PlaybackStreams::default()));
    let engine_volume: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));
    let mixer = Rc::new(RefCell::new(crate::audio::mixer::Mixer::new(4096)));
    mixer.borrow_mut().install_default_chain(48_000)?;

    let ctx = EngineCtx {
        registry_sink_id,
        core: core.clone(),
        voices: voices.clone(),
        playback_streams,
        evt_tx: evt_tx.clone(),
        engine_volume,
        monitor_target,
        mixer,
        router: router.clone(),
    };

    let voices_timer = voices;
    let evt_tx_timer = evt_tx.clone();
    let pw_loop = mainloop.loop_();
    let _completion_timer = setup_completion_timer(pw_loop, voices_timer, evt_tx_timer)?;

    let mainloop_quit = mainloop.clone();
    let _cmd_listener = cmd_rx.attach(mainloop.loop_(), move |cmd| match cmd {
        AudioCommand::Play {
            processing,
            voice_id,
            sound_id,
            samples,
            sample_rate,
            channels,
            generation,
            gain,
            effects,
            mode,
        } => {
            handle_play(
                &ctx,
                PlayRequest {
                    processing,
                    voice_id,
                    sound_id,
                    samples,
                    sample_rate,
                    channels,
                    generation,
                    gain,
                    effects,
                    mode,
                },
            );
        }
        AudioCommand::StopVoice(voice_id) => {
            let finished = ctx.voices.borrow_mut().stop_voice(voice_id);
            send_finished_events(&ctx.evt_tx, finished);
        }
        AudioCommand::Stop => {
            let finished = ctx.voices.borrow_mut().stop_all();
            send_finished_events(&ctx.evt_tx, finished);
        }
        AudioCommand::SetDynamics(settings) => ctx.voices.borrow_mut().set_dynamics(settings),
        AudioCommand::SetVolume(v) => {
            let volume = v.clamp(0.0, 1.0);
            ctx.engine_volume.set(volume);
            ctx.voices.borrow_mut().set_master_volume(volume);
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
            use crate::audio::router::RouterCommand;
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
            let _ = ctx.voices.borrow_mut().stop_all();
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
