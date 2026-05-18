//! Integration test for issue #26 — external audio stream watcher.
//!
//! Spawns `paplay` to play a sample sound through PipeWire, then asserts
//! the watcher emits a `SourceAdded` carrying the subprocess PID and a
//! matching `SourceRemoved` after kill.
//!
//! Gated behind `pipewire-test` feature — requires a live PipeWire +
//! WirePlumber session AND `paplay` from `pulseaudio-utils`. Skipped in
//! stock GitHub Actions runners.

#![cfg(feature = "pipewire-test")]

use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use honkhonk::audio::StreamEvent;

const SAMPLE_PATH: &str = "/usr/share/sounds/freedesktop/stereo/bell.oga";

type EventLog = Arc<Mutex<Vec<StreamEvent>>>;

fn wait_for<F>(events: &EventLog, deadline: Duration, pred: F) -> Option<StreamEvent>
where
    F: Fn(&StreamEvent) -> bool,
{
    let start = Instant::now();
    while start.elapsed() < deadline {
        if let Some(found) = events
            .lock()
            .expect("event mutex poisoned")
            .iter()
            .find(|e| pred(e))
            .cloned()
        {
            return Some(found);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn paplay_available() -> bool {
    Command::new("paplay")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Watcher mainloop running on a dedicated thread.
///
/// `ready_rx` resolves once `streams::start` is wired and the registry
/// listener is attached. `quit_tx` (paired in caller) terminates the
/// loop. `events` receives every emitted `StreamEvent`.
struct WatcherHandle {
    pw_thread: JoinHandle<()>,
    quit_tx: pipewire::channel::Sender<()>,
}

fn spawn_watcher(events: EventLog) -> WatcherHandle {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let (quit_tx, quit_rx) = pipewire::channel::channel::<()>();

    let pw_thread = std::thread::Builder::new()
        .name("stream-watcher-it".into())
        .spawn(move || {
            let mainloop =
                pipewire::main_loop::MainLoopRc::new(None).expect("test mainloop construction");
            let ctx = pipewire::context::ContextRc::new(&mainloop, None).expect("test context");
            let core = ctx.connect_rc(None).expect("test core connect");

            let (watcher, rx) = honkhonk::audio::streams::start(&core, std::process::id())
                .expect("streams::start failed");

            std::thread::Builder::new()
                .name("stream-event-drain".into())
                .spawn(move || {
                    while let Ok(event) = rx.recv() {
                        events.lock().expect("event mutex poisoned").push(event);
                    }
                })
                .expect("drain thread spawn");

            let mainloop_quit = mainloop.clone();
            let _quit_listener = quit_rx.attach(mainloop.loop_(), move |_| {
                mainloop_quit.quit();
            });

            ready_tx.send(()).expect("ready signal");
            mainloop.run();
            drop(watcher);
        })
        .expect("pw thread spawn");

    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("watcher never reported ready");

    // Give registry a moment to enumerate baseline globals.
    std::thread::sleep(Duration::from_millis(500));

    WatcherHandle { pw_thread, quit_tx }
}

fn shutdown_watcher(handle: WatcherHandle) {
    let _ = handle.quit_tx.send(());
    handle
        .pw_thread
        .join()
        .expect("stream-watcher-it thread panicked");
}

fn spawn_paplay() -> Child {
    Command::new("paplay")
        .arg(SAMPLE_PATH)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("paplay spawn failed")
}

fn assert_source_added_for(events: &EventLog, pid: u32) -> u32 {
    let added = wait_for(
        events,
        Duration::from_secs(3),
        |e| matches!(e, StreamEvent::SourceAdded { app_pid: Some(p), .. } if *p == pid),
    );
    let snapshot = events.lock().expect("event mutex poisoned").clone();
    let Some(StreamEvent::SourceAdded { id, .. }) = added else {
        panic!("expected SourceAdded with pid={pid} within 3s; events: {snapshot:?}");
    };
    id
}

fn assert_source_removed_for(events: &EventLog, id: u32) {
    let removed = wait_for(
        events,
        Duration::from_secs(3),
        |e| matches!(e, StreamEvent::SourceRemoved { id: rid } if *rid == id),
    );
    let snapshot = events.lock().expect("event mutex poisoned").clone();
    assert!(
        removed.is_some(),
        "expected SourceRemoved id={id} within 3s; events: {snapshot:?}"
    );
}

#[test]
fn paplay_subprocess_emits_source_added_then_removed() {
    pipewire::init();
    if !std::path::Path::new(SAMPLE_PATH).exists() {
        eprintln!("skipping: sample {SAMPLE_PATH} not installed");
        return;
    }
    if !paplay_available() {
        eprintln!("skipping: paplay not available");
        return;
    }

    let events: EventLog = Arc::default();
    let watcher = spawn_watcher(events.clone());

    let mut child = spawn_paplay();
    let child_pid = child.id();

    let added_id = assert_source_added_for(&events, child_pid);

    let _ = child.kill();
    let _ = child.wait();

    assert_source_removed_for(&events, added_id);

    shutdown_watcher(watcher);
}
