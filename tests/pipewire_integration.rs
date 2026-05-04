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
