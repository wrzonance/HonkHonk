#![cfg(feature = "pipewire-test")]

use std::process::Command;
use std::time::Duration;

#[test]
fn virtual_sink_appears_in_wpctl() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("failed to spawn audio engine");

    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no event received from audio engine within 5s");

    assert!(
        matches!(event, honkhonk::audio::AudioEvent::Ready),
        "expected Ready event, got: {event:?}"
    );

    // Give WirePlumber a moment to register the node
    std::thread::sleep(Duration::from_millis(500));

    let output = Command::new("wpctl")
        .arg("status")
        .output()
        .expect("wpctl not found — is WirePlumber running?");

    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        status.contains("HonkHonk Mix"),
        "Virtual sink 'HonkHonk Mix' not found in wpctl status.\n\
         wpctl output:\n{status}"
    );
    assert!(
        status.contains("HonkHonk Mic"),
        "Virtual source 'HonkHonk Mic' not found in wpctl status.\n\
         wpctl output:\n{status}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));

    let output = Command::new("wpctl")
        .arg("status")
        .output()
        .expect("wpctl failed");

    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        !status.contains("HonkHonk Mix"),
        "Virtual sink 'HonkHonk Mix' was NOT cleaned up after shutdown.\n\
         wpctl output:\n{status}"
    );
    assert!(
        !status.contains("HonkHonk Mic"),
        "Virtual source 'HonkHonk Mic' was NOT cleaned up after shutdown.\n\
         wpctl output:\n{status}"
    );
}

#[test]
fn mic_linked_to_virtual_sink() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("failed to spawn audio engine");

    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no event received within 5s");

    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    // Wait for registry discovery + link creation
    std::thread::sleep(Duration::from_secs(2));

    let output = Command::new("pw-link")
        .arg("--links")
        .output()
        .expect("pw-link not found");

    let links = String::from_utf8_lossy(&output.stdout);

    let has_sink = links.contains("honkhonk-mix");

    // Check if any audio source exists in the system
    let wpctl = Command::new("wpctl")
        .arg("status")
        .output()
        .expect("wpctl failed");
    let status = String::from_utf8_lossy(&wpctl.stdout);
    let has_source = status.contains("Sources:");

    if has_source {
        assert!(
            has_sink,
            "Expected links to honkhonk-mix when audio sources exist.\n\
             pw-link output:\n{links}"
        );
    }

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn engine_cleans_up_on_shutdown() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("spawn failed");
    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    std::thread::sleep(Duration::from_millis(500));
    let output = Command::new("wpctl").arg("status").output().unwrap();
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        status.contains("HonkHonk Mix"),
        "Sink should exist before shutdown"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_secs(1));

    let output = Command::new("wpctl").arg("status").output().unwrap();
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        !status.contains("HonkHonk Mix"),
        "Sink should be destroyed after shutdown"
    );
}

#[test]
fn play_sound_emits_started_and_finished_events() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("failed to spawn audio engine");

    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no Ready event");
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    // Wait for registry to discover sink node ID
    std::thread::sleep(Duration::from_secs(2));

    // Decode a short test fixture
    let decoded = honkhonk::audio::decode(std::path::Path::new("tests/fixtures/sine_mono.wav"))
        .expect("decode failed");

    let samples = std::sync::Arc::new(decoded.samples);

    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "test-sine".into(),
        samples,
        sample_rate: decoded.sample_rate,
        channels: decoded.channels,
    });

    // Should get PlaybackStarted
    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no PlaybackStarted event");
    assert!(
        matches!(event, honkhonk::audio::AudioEvent::PlaybackStarted { ref sound_id } if sound_id == "test-sine"),
        "expected PlaybackStarted, got: {event:?}"
    );

    // Should get PlaybackFinished within a few seconds (short audio file)
    let event = handle
        .recv_timeout(Duration::from_secs(10))
        .expect("no PlaybackFinished event");
    assert!(
        matches!(event, honkhonk::audio::AudioEvent::PlaybackFinished { ref sound_id } if sound_id == "test-sine"),
        "expected PlaybackFinished, got: {event:?}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn stop_command_halts_playback() {
    pipewire::init();

    let handle = honkhonk::audio::spawn().expect("spawn failed");
    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));

    std::thread::sleep(Duration::from_secs(2));

    // Use a long synthetic sound (~5 seconds stereo)
    let samples = std::sync::Arc::new(vec![0.3f32; 48000 * 5 * 2]);

    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "long-sound".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
    });

    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        matches!(event, honkhonk::audio::AudioEvent::PlaybackStarted { ref sound_id } if sound_id == "long-sound"),
        "expected PlaybackStarted for long-sound, got: {event:?}"
    );

    // Stop after 500ms
    std::thread::sleep(Duration::from_millis(500));
    handle.send(honkhonk::audio::AudioCommand::Stop);

    let event = handle.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        matches!(event, honkhonk::audio::AudioEvent::PlaybackFinished { ref sound_id } if sound_id == "long-sound"),
        "expected PlaybackFinished after stop, got: {event:?}"
    );

    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}
