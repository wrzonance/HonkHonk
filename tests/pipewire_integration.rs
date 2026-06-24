#![cfg(feature = "pipewire-test")]

use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

const SINK_NODE_NAME: &str = "honkhonk-mix";
const SOURCE_NODE_NAME: &str = "honkhonk-mic";
const SINK_DESCRIPTION: &str = "HonkHonk Mix";
const SOURCE_DESCRIPTION: &str = "HonkHonk Mic";
static PIPEWIRE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn pipewire_test_guard() -> MutexGuard<'static, ()> {
    PIPEWIRE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn virtual_sink_appears_in_wpctl() {
    let _guard = pipewire_test_guard();
    pipewire::init();

    let handle = spawn_engine_and_wait();

    let output = Command::new("wpctl")
        .arg("status")
        .output()
        .expect("wpctl not found — is WirePlumber running?");

    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        status.contains(SINK_DESCRIPTION),
        "Virtual sink 'HonkHonk Mix' not found in wpctl status.\n\
         wpctl output:\n{status}"
    );
    assert!(
        status.contains(SOURCE_DESCRIPTION),
        "Virtual source 'HonkHonk Mic' not found in wpctl status.\n\
         wpctl output:\n{status}"
    );
    assert_pipewire_node_present(SINK_NODE_NAME);
    assert_pipewire_node_present(SOURCE_NODE_NAME);

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));

    assert!(
        !pipewire_node_exists(SINK_NODE_NAME),
        "Virtual sink '{SINK_NODE_NAME}' was NOT cleaned up after shutdown"
    );
    assert_pipewire_node_present(SOURCE_NODE_NAME);
}

fn get_default_source_name() -> Option<String> {
    let output = Command::new("pw-metadata")
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

#[test]
fn default_mic_linked_to_virtual_sink() {
    let _guard = pipewire_test_guard();
    pipewire::init();
    let handle = spawn_engine_and_wait();

    let default_source = match get_default_source_name() {
        Some(name) if name != SOURCE_NODE_NAME => name,
        _ => {
            handle.shutdown();
            std::thread::sleep(Duration::from_millis(500));
            return;
        }
    };

    let links = get_pw_links();

    assert!(
        mix_input_lines(&links).join("\n").contains(&default_source),
        "Default source '{default_source}' should be linked to honkhonk-mix.\npw-link:\n{links}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

fn get_pw_links() -> String {
    let output = Command::new("pw-link")
        .arg("--links")
        .output()
        .expect("pw-link not found");
    assert!(
        output.status.success(),
        "pw-link --links failed (exit {:?}):\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn spawn_engine_and_wait() -> honkhonk::audio::AudioHandle {
    let handle = honkhonk::audio::spawn(true, None, None).expect("failed to spawn audio engine");
    wait_for_event(&handle, Duration::from_secs(5), "Ready", |event| {
        matches!(event, honkhonk::audio::AudioEvent::Ready)
    });
    std::thread::sleep(Duration::from_secs(2));
    handle
}

fn expect_started(handle: &honkhonk::audio::AudioHandle, expected: &str) {
    let label = format!("PlaybackStarted({expected})");
    wait_for_event(
        handle,
        Duration::from_secs(5),
        &label,
        |event| matches!(event, honkhonk::audio::AudioEvent::PlaybackStarted { sound_id } if sound_id == expected),
    );
}

fn expect_finished(handle: &honkhonk::audio::AudioHandle, expected: &str, timeout: Duration) {
    let label = format!("PlaybackFinished({expected})");
    wait_for_event(
        handle,
        timeout,
        &label,
        |event| matches!(event, honkhonk::audio::AudioEvent::PlaybackFinished { sound_id, .. } if sound_id == expected),
    );
}

fn wait_for_event(
    handle: &honkhonk::audio::AudioHandle,
    timeout: Duration,
    label: &str,
    matches: impl Fn(&honkhonk::audio::AudioEvent) -> bool,
) -> honkhonk::audio::AudioEvent {
    let deadline = Instant::now() + timeout;
    let mut seen = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "no {label} event within {timeout:?}; seen events: {seen:?}"
        );
        if let Some(event) = handle.recv_timeout(remaining.min(Duration::from_millis(500))) {
            if matches(&event) {
                return event;
            }
            seen.push(event);
        }
    }
}

fn pipewire_node_names() -> Vec<String> {
    let output = Command::new("pw-dump").output().expect("pw-dump not found");
    assert!(
        output.status.success(),
        "pw-dump failed (exit {:?}):\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("pw-dump emitted invalid JSON");
    value
        .as_array()
        .expect("pw-dump root should be a JSON array")
        .iter()
        .filter_map(|node| node.pointer("/info/props/node.name")?.as_str())
        .map(str::to_owned)
        .collect()
}

fn pipewire_node_exists(node_name: &str) -> bool {
    pipewire_node_names().iter().any(|name| name == node_name)
}

fn assert_pipewire_node_present(node_name: &str) {
    let names = pipewire_node_names();
    assert!(
        names.iter().any(|name| name == node_name),
        "PipeWire node '{node_name}' not found; nodes: {names:?}"
    );
}

fn mix_input_lines(links: &str) -> Vec<&str> {
    let mut in_mix = false;
    let mut lines = Vec::new();
    for line in links.lines() {
        if line.starts_with("honkhonk-mix:input") {
            in_mix = true;
            lines.push(line);
        } else if in_mix && line.starts_with("  |") {
            lines.push(line);
        } else {
            in_mix = false;
        }
    }
    lines
}

fn playback_stream_reaches_mix(links: &str) -> bool {
    mix_input_lines(links).iter().any(|line| {
        line.contains("|<-") && !line.contains("alsa_input") && !line.contains(SOURCE_NODE_NAME)
    })
}

#[test]
fn both_stereo_channels_linked_sink_to_source() {
    let _guard = pipewire_test_guard();
    pipewire::init();
    let handle = spawn_engine_and_wait();

    let links = get_pw_links();

    let fl_linked =
        links.contains("honkhonk-mix:capture_FL") && links.contains("honkhonk-mic:input_FL");
    let fr_linked =
        links.contains("honkhonk-mix:capture_FR") && links.contains("honkhonk-mic:input_FR");

    assert!(
        fl_linked,
        "FL link missing between sink capture and source input.\npw-link:\n{links}"
    );
    assert!(
        fr_linked,
        "FR link missing between sink capture and source input.\npw-link:\n{links}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn sink_stream_reaches_virtual_sink() {
    let _guard = pipewire_test_guard();
    pipewire::init();
    let handle = spawn_engine_and_wait();

    let samples = std::sync::Arc::new(vec![0.5f32; 48000 * 5 * 2]);
    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "routing-test".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
        generation: 1,
        volume: 1.0,
    });

    expect_started(&handle, "routing-test");

    std::thread::sleep(Duration::from_millis(500));

    let links = get_pw_links();

    assert!(
        playback_stream_reaches_mix(&links),
        "playback stream should be connected to honkhonk-mix:input, \
         but only mic passthrough found.\npw-link:\n{links}"
    );

    handle.send(honkhonk::audio::AudioCommand::Stop);
    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn audio_pipeline_end_to_end() {
    let _guard = pipewire_test_guard();
    pipewire::init();
    let handle = spawn_engine_and_wait();

    let samples = std::sync::Arc::new(vec![0.5f32; 48000 * 3 * 2]);
    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "e2e-test".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
        generation: 1,
        volume: 1.0,
    });

    expect_started(&handle, "e2e-test");

    std::thread::sleep(Duration::from_millis(500));

    let links = get_pw_links();

    let fl_sink_to_source =
        links.contains("honkhonk-mix:capture_FL") && links.contains("honkhonk-mic:input_FL");
    let fr_sink_to_source =
        links.contains("honkhonk-mix:capture_FR") && links.contains("honkhonk-mic:input_FR");

    assert!(fl_sink_to_source, "FL sink→source link missing.\n{links}");
    assert!(fr_sink_to_source, "FR sink→source link missing.\n{links}");

    assert!(
        playback_stream_reaches_mix(&links),
        "Full pipeline broken: playback stream not connected to virtual sink.\n{links}"
    );

    handle.send(honkhonk::audio::AudioCommand::Stop);
    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn engine_cleans_up_on_shutdown() {
    let _guard = pipewire_test_guard();
    pipewire::init();

    let handle = spawn_engine_and_wait();

    std::thread::sleep(Duration::from_millis(500));
    assert_pipewire_node_present(SINK_NODE_NAME);
    assert_pipewire_node_present(SOURCE_NODE_NAME);

    handle.shutdown();
    std::thread::sleep(Duration::from_secs(1));

    assert!(
        !pipewire_node_exists(SINK_NODE_NAME),
        "Sink should be destroyed after shutdown"
    );
    assert_pipewire_node_present(SOURCE_NODE_NAME);
}

#[test]
fn play_sound_emits_started_and_finished_events() {
    let _guard = pipewire_test_guard();
    pipewire::init();

    let handle = spawn_engine_and_wait();

    let decoded = honkhonk::audio::decode(std::path::Path::new("tests/fixtures/sine_mono.wav"))
        .expect("decode failed");

    let samples = std::sync::Arc::new(decoded.samples);

    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "test-sine".into(),
        samples,
        sample_rate: decoded.sample_rate,
        channels: decoded.channels,
        generation: 1,
        volume: 1.0,
    });

    expect_started(&handle, "test-sine");

    expect_finished(&handle, "test-sine", Duration::from_secs(10));

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn stop_command_halts_playback() {
    let _guard = pipewire_test_guard();
    pipewire::init();

    let handle = spawn_engine_and_wait();

    let samples = std::sync::Arc::new(vec![0.3f32; 48000 * 5 * 2]);
    let replacement = std::sync::Arc::new(vec![0.2f32; 48000 * 5 * 2]);

    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "long-sound".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
        generation: 1,
        volume: 1.0,
    });

    expect_started(&handle, "long-sound");

    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "replacement-sound".into(),
        samples: replacement,
        sample_rate: 48000,
        channels: 2,
        generation: 2,
        volume: 1.0,
    });

    expect_finished(&handle, "long-sound", Duration::from_secs(5));
    expect_started(&handle, "replacement-sound");

    std::thread::sleep(Duration::from_millis(500));
    handle.send(honkhonk::audio::AudioCommand::Stop);

    expect_finished(&handle, "replacement-sound", Duration::from_secs(5));

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}
