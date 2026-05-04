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
