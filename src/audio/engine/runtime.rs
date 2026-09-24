use super::setup::RoutingSetup;
use super::*;
use pipewire::spa::utils::result::AsyncSeq;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Shutdown {
    pending: Option<AsyncSeq>,
    deadline: Option<Instant>,
}

impl Shutdown {
    fn begin(
        &mut self,
        restore: impl FnOnce(),
        sync: impl FnOnce() -> Result<AsyncSeq, pipewire::Error>,
        now: Instant,
    ) -> Result<(), pipewire::Error> {
        self.deadline = Some(now + Duration::from_secs(2));
        restore();
        self.pending = Some(sync()?);
        Ok(())
    }

    fn done(&self, id: u32, seq: AsyncSeq) -> bool {
        id == 0 && self.pending == Some(seq)
    }

    fn expired(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|deadline| now >= deadline)
    }

    pub(super) fn active(&self) -> bool {
        self.deadline.is_some()
    }

    fn failed(&self, id: u32) -> bool {
        id == 0 && self.active()
    }
}

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
    let _shutdown_listener = shutdown_listener(&ctx, &mainloop);
    let _routing_timer = routing_timer(pw_loop, setup, ctx.shutdown.clone(), mainloop.clone())?;
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
        voices: setup.routing.voices.clone(),
        playback_streams: Rc::new(RefCell::new(PlaybackStreams::default())),
        evt_tx,
        engine_volume: Rc::new(Cell::new(1.0)),
        monitor_target: Rc::new(RefCell::new(monitor_device)),
        mixer,
        router: setup.routing.router.clone(),
        stream_watcher: setup.stream_watcher.clone(),
        shutdown: Rc::new(RefCell::new(Shutdown::default())),
    })
}

pub(super) fn begin_shutdown(ctx: &EngineCtx, mainloop: &pipewire::main_loop::MainLoopRc) {
    let result = ctx.shutdown.borrow_mut().begin(
        || {
            for (id, error) in ctx.stream_watcher.restore_all_levels() {
                tracing::warn!(id, %error, "restore source volume during shutdown");
                let _ = ctx.evt_tx.send(super::commands::level_error(id, &error));
            }
        },
        || ctx.core.sync(0),
        Instant::now(),
    );
    if let Err(error) = result {
        tracing::warn!(%error, "could not flush source restoration during shutdown");
        mainloop.quit();
    }
}

fn shutdown_listener(
    ctx: &EngineCtx,
    mainloop: &pipewire::main_loop::MainLoopRc,
) -> pipewire::core::Listener {
    let shutdown = ctx.shutdown.clone();
    let quit = mainloop.clone();
    let errors = ctx.shutdown.clone();
    let failed = mainloop.clone();
    ctx.core
        .add_listener_local()
        .done(move |id, seq| {
            if shutdown.borrow().done(id, seq) {
                quit.quit();
            }
        })
        .error(move |id, seq, code, message| {
            if errors.borrow().active() {
                tracing::warn!(
                    id,
                    seq,
                    code,
                    message,
                    "PipeWire failed during source restoration"
                );
                // Other nodes may still restore successfully. Keep processing
                // until the barrier completes or the bounded timer expires.
                if errors.borrow().failed(id) {
                    failed.quit();
                }
            }
        })
        .register()
}

fn routing_timer<'a>(
    pw_loop: &'a pipewire::loop_::Loop,
    setup: RoutingSetup,
    shutdown: Rc<RefCell<Shutdown>>,
    mainloop: pipewire::main_loop::MainLoopRc,
) -> Result<pipewire::loop_::TimerSource<'a>, AudioError> {
    let timer = pw_loop.add_timer(move |_| {
        if shutdown.borrow().expired(Instant::now()) {
            tracing::warn!("timed out flushing source restoration during shutdown");
            mainloop.quit();
        } else if !shutdown.borrow().active() {
            setup.tick();
        }
    });
    timer
        .update_timer(
            Some(Duration::from_millis(10)),
            Some(Duration::from_millis(10)),
        )
        .into_result()
        .map_err(|e| AudioError::PipeWireInit(format!("arm routing safety timer: {e}")))?;
    Ok(timer)
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;

    #[test]
    fn node_errors_keep_draining_but_core_disconnect_terminates_shutdown() {
        let mut shutdown = Shutdown::default();
        assert!(!shutdown.failed(0));
        shutdown
            .begin(|| {}, || Ok(AsyncSeq::from_seq(1)), Instant::now())
            .unwrap();
        assert!(!shutdown.failed(42));
        assert!(shutdown.failed(0));
        assert!(shutdown.done(0, AsyncSeq::from_seq(1)));
    }

    #[test]
    fn sync_failure_is_reported_after_best_effort_restoration() {
        let mut shutdown = Shutdown::default();
        let restored = Cell::new(false);
        let result = shutdown.begin(
            || restored.set(true),
            || Err(pipewire::Error::CreationFailed),
            Instant::now(),
        );
        assert!(restored.get());
        assert!(matches!(result, Err(pipewire::Error::CreationFailed)));
        assert!(!shutdown.done(0, AsyncSeq::from_seq(1)));
    }

    #[test]
    fn restore_precedes_sync_and_only_matching_core_done_allows_exit() {
        let mut shutdown = Shutdown::default();
        let events = RefCell::new(Vec::new());
        let seq = AsyncSeq::from_seq(9);
        shutdown
            .begin(
                || events.borrow_mut().push("restore"),
                || {
                    events.borrow_mut().push("sync");
                    Ok(seq)
                },
                Instant::now(),
            )
            .unwrap();
        assert_eq!(*events.borrow(), ["restore", "sync"]);
        assert!(!shutdown.done(4, seq));
        assert!(!shutdown.done(0, AsyncSeq::from_seq(8)));
        assert!(shutdown.done(0, seq));
    }

    #[test]
    fn unresponsive_server_has_bounded_shutdown() {
        let mut shutdown = Shutdown::default();
        let now = Instant::now();
        assert!(!shutdown.expired(now + Duration::from_secs(99)));
        shutdown
            .begin(|| {}, || Ok(AsyncSeq::from_seq(1)), now)
            .unwrap();
        assert!(!shutdown.expired(now));
        assert!(shutdown.expired(now + Duration::from_secs(2)));
    }
}
