use super::setup::RoutingSetup;
use super::*;

pub(super) fn run_engine(
    cmd_rx: pipewire::channel::Receiver<AudioCommand>,
    evt_tx: mpsc::Sender<AudioEvent>,
    preferred_source: Option<String>,
    initial_passthrough: bool,
    initial_monitor_device: Option<String>,
) -> Result<(), AudioError> {
    let mainloop = pipewire::main_loop::MainLoopRc::new(None)
        .map_err(|e| AudioError::PipeWireInit(format!("main loop: {e}")))?;
    let context = pipewire::context::ContextRc::new(&mainloop, None)
        .map_err(|e| AudioError::PipeWireInit(format!("context: {e}")))?;
    let core = context
        .connect_rc(None)
        .map_err(|e| AudioError::PipeWireInit(format!("core connect: {e}")))?;
    let setup = RoutingSetup::new(
        core.clone(),
        evt_tx.clone(),
        preferred_source,
        initial_passthrough,
    )?;
    let ctx = engine_context(core, evt_tx.clone(), &setup, initial_monitor_device)?;
    let registry = setup.routing.registry.clone();
    let pw_loop = mainloop.loop_();
    let _routing_timer = pw_loop.add_timer(move |_| setup.tick());
    _routing_timer
        .update_timer(
            Some(std::time::Duration::from_millis(10)),
            Some(std::time::Duration::from_millis(10)),
        )
        .into_result()
        .map_err(|e| AudioError::PipeWireInit(format!("arm routing safety timer: {e}")))?;
    let _completion_timer = setup_completion_timer(pw_loop, ctx.voices.clone(), evt_tx.clone())?;
    let quit = mainloop.clone();
    let _cmd_listener = cmd_rx.attach(pw_loop, move |cmd| {
        super::commands::handle(&ctx, &registry, &quit, cmd)
    });
    let _ = evt_tx.send(AudioEvent::Ready);
    mainloop.run();
    Ok(())
}

fn engine_context(
    core: pipewire::core::CoreRc,
    evt_tx: mpsc::Sender<AudioEvent>,
    setup: &RoutingSetup,
    monitor_device: Option<String>,
) -> Result<EngineCtx, AudioError> {
    let mixer = Rc::new(RefCell::new(crate::audio::mixer::Mixer::new(4096)));
    mixer.borrow_mut().install_default_chain(48_000)?;
    Ok(EngineCtx {
        registry_sink_id: setup.sink_id.clone(),
        core,
        voices: Rc::new(RefCell::new(VoicePool::new())),
        playback_streams: Rc::new(RefCell::new(PlaybackStreams::default())),
        evt_tx,
        engine_volume: Rc::new(Cell::new(1.0)),
        monitor_target: Rc::new(RefCell::new(monitor_device)),
        mixer,
        router: setup.routing.router.clone(),
        stream_watcher: setup.stream_watcher.clone(),
    })
}
