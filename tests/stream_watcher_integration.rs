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

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use honkhonk::audio::StreamEvent;

const SAMPLE_PATH: &str = "/usr/share/sounds/freedesktop/stereo/bell.oga";

fn wait_for<F>(
    events: &Arc<Mutex<Vec<StreamEvent>>>,
    deadline: Duration,
    pred: F,
) -> Option<StreamEvent>
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

#[test]
fn paplay_subprocess_emits_source_added_then_removed() {
    pipewire::init();
    if !std::path::Path::new(SAMPLE_PATH).exists() {
        eprintln!("skipping: sample {SAMPLE_PATH} not installed");
        return;
    }
    if Command::new("paplay")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        eprintln!("skipping: paplay not available");
        return;
    }

    let events: Arc<Mutex<Vec<StreamEvent>>> = Arc::default();
    let events_for_drain = events.clone();

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
                        events_for_drain.lock().unwrap().push(event);
                    }
                })
                .expect("drain thread spawn");

            // Quit listener — when test signals shutdown, exit the main loop.
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

    let mut child = Command::new("paplay")
        .arg(SAMPLE_PATH)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("paplay spawn failed");
    let child_pid = child.id();

    let added = wait_for(
        &events,
        Duration::from_secs(3),
        |e| matches!(e, StreamEvent::SourceAdded { app_pid: Some(p), .. } if *p == child_pid),
    );
    let snapshot = events.lock().unwrap().clone();
    assert!(
        added.is_some(),
        "expected SourceAdded with pid={child_pid} within 3s; events: {snapshot:?}"
    );

    let added_id = match added {
        Some(StreamEvent::SourceAdded { id, .. }) => id,
        _ => unreachable!(),
    };

    let _ = child.kill();
    let _ = child.wait();

    let removed = wait_for(
        &events,
        Duration::from_secs(3),
        |e| matches!(e, StreamEvent::SourceRemoved { id } if *id == added_id),
    );
    let snapshot = events.lock().unwrap().clone();
    assert!(
        removed.is_some(),
        "expected SourceRemoved id={added_id} within 3s; events: {snapshot:?}"
    );

    let _ = quit_tx.send(());
    let _ = pw_thread.join();
}
