//! Behavioral invariants for app state and message handling.

use super::*;

mod devices;
mod favorites_and_panels;
mod lifecycle;
mod lists_and_shortcuts;
mod playback_events;
mod playhead;
mod preferences;
mod sound_editor;
mod tags;

/// Minimal 16-bit PCM mono WAV (4 samples) so tests can exercise the real
/// decode path without fixture files.
fn write_test_wav(path: &std::path::Path) {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&44u32.to_le_bytes()); // riff chunk size
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&44100u32.to_le_bytes());
    bytes.extend_from_slice(&88200u32.to_le_bytes()); // byte rate
    bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&8u32.to_le_bytes());
    for s in [0i16, 8000, -8000, 0] {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, bytes).expect("write test wav");
}
