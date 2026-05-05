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
    pipewire::init();
    let handle = spawn_engine_and_wait();

    let default_source = match get_default_source_name() {
        Some(name) => name,
        None => {
            handle.shutdown();
            return;
        }
    };

    let links = get_pw_links();

    let mix_input_section: String = links
        .lines()
        .skip_while(|l| !l.starts_with("honkhonk-mix:input_FL"))
        .take_while(|l| l.starts_with("honkhonk-mix:input") || l.starts_with("  |"))
        .collect::<Vec<_>>()
        .join("\n");

    let default_linked = mix_input_section.contains(&default_source);

    assert!(
        default_linked,
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
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn spawn_engine_and_wait() -> honkhonk::audio::AudioHandle {
    let handle = honkhonk::audio::spawn().expect("failed to spawn audio engine");
    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no Ready event within 5s");
    assert!(matches!(event, honkhonk::audio::AudioEvent::Ready));
    std::thread::sleep(Duration::from_secs(2));
    handle
}

#[test]
fn both_stereo_channels_linked_sink_to_source() {
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
    pipewire::init();
    let handle = spawn_engine_and_wait();

    let samples = std::sync::Arc::new(vec![0.5f32; 48000 * 5 * 2]);
    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "routing-test".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
    });

    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no PlaybackStarted event");
    assert!(matches!(
        event,
        honkhonk::audio::AudioEvent::PlaybackStarted { .. }
    ));

    std::thread::sleep(Duration::from_millis(500));

    let links = get_pw_links();

    let mix_input_section: Vec<&str> = links
        .lines()
        .skip_while(|l| !l.starts_with("honkhonk-mix:input_FL"))
        .take_while(|l| l.starts_with("honkhonk-mix:input") || l.starts_with("  |"))
        .collect();

    let has_non_mic_source = mix_input_section
        .iter()
        .any(|l| l.contains("|<-") && !l.contains("alsa_input"));

    assert!(
        has_non_mic_source,
        "playback stream should be connected to honkhonk-mix:input, \
         but only mic passthrough found.\npw-link:\n{links}"
    );

    handle.send(honkhonk::audio::AudioCommand::Stop);
    handle.shutdown();
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn audio_pipeline_end_to_end() {
    pipewire::init();
    let handle = spawn_engine_and_wait();

    let samples = std::sync::Arc::new(vec![0.5f32; 48000 * 3 * 2]);
    handle.send(honkhonk::audio::AudioCommand::Play {
        sound_id: "e2e-test".into(),
        samples,
        sample_rate: 48000,
        channels: 2,
    });

    let event = handle
        .recv_timeout(Duration::from_secs(5))
        .expect("no PlaybackStarted event");
    assert!(matches!(
        event,
        honkhonk::audio::AudioEvent::PlaybackStarted { .. }
    ));

    std::thread::sleep(Duration::from_millis(500));

    let links = get_pw_links();

    let fl_sink_to_source =
        links.contains("honkhonk-mix:capture_FL") && links.contains("honkhonk-mic:input_FL");
    let fr_sink_to_source =
        links.contains("honkhonk-mix:capture_FR") && links.contains("honkhonk-mic:input_FR");

    assert!(fl_sink_to_source, "FL sink→source link missing.\n{links}");
    assert!(fr_sink_to_source, "FR sink→source link missing.\n{links}");

    let mix_input_section: Vec<&str> = links
        .lines()
        .skip_while(|l| !l.starts_with("honkhonk-mix:input_FL"))
        .take_while(|l| l.starts_with("honkhonk-mix:input") || l.starts_with("  |"))
        .collect();

    let playback_reaches_sink = mix_input_section
        .iter()
        .any(|l| l.contains("|<-") && !l.contains("alsa_input"));

    assert!(
        playback_reaches_sink,
        "Full pipeline broken: playback stream not connected to virtual sink.\n{links}"
    );

    handle.send(honkhonk::audio::AudioCommand::Stop);
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
