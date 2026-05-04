# Symphonia Decode to PCM — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decode MP3, OGG, FLAC, and WAV audio files to interleaved f32 PCM samples using symphonia, establishing the `src/audio/` module with typed errors.

**Architecture:** `src/audio/decoder.rs` exposes a single `decode(path) -> Result<DecodedAudio, AudioError>` function. symphonia probes the file format automatically, decodes all packets into a `SampleBuffer<f32>`, and returns interleaved samples. `src/audio/error.rs` defines `AudioError` with thiserror for typed errors at the module boundary.

**Tech Stack:** symphonia 0.5 (MP3/OGG/Vorbis/FLAC/WAV/PCM/AAC), thiserror 2

**Issue:** #4

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src/audio/mod.rs` | Re-exports: `pub use decoder::{decode, DecodedAudio}; pub use error::AudioError;` |
| Create | `src/audio/error.rs` | `AudioError` enum (thiserror) — file I/O, format, track, codec errors |
| Create | `src/audio/decoder.rs` | `decode()` function + `DecodedAudio` struct |
| Modify | `src/lib.rs` | Add `pub mod audio;` |
| Modify | `Cargo.toml` | Add symphonia, thiserror deps |
| Create | `tests/fixtures/` | Generated test audio files (440Hz sine, mono+stereo, all formats) |
| Create | `tests/decoder_test.rs` | Integration tests for decode function |

---

### Task 1: Project Setup — Dependencies, Module Stubs, Test Fixtures

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/audio/mod.rs`
- Create: `src/audio/error.rs`
- Create: `src/audio/decoder.rs`
- Create: `tests/fixtures/` (generated audio files)

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add after the existing `[dependencies]` entries:

```toml
symphonia = { version = "0.5", features = ["mp3", "ogg", "vorbis", "flac", "wav", "pcm", "aac"] }
thiserror = "2"
```

- [ ] **Step 2: Create `src/audio/error.rs` stub**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("placeholder")]
    Todo,
}
```

- [ ] **Step 3: Create `src/audio/decoder.rs` stub**

```rust
use super::error::AudioError;

pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: std::time::Duration,
}

pub fn decode(_path: &std::path::Path) -> Result<DecodedAudio, AudioError> {
    Err(AudioError::Todo)
}
```

- [ ] **Step 4: Create `src/audio/mod.rs`**

```rust
mod decoder;
mod error;

pub use decoder::{decode, DecodedAudio};
pub use error::AudioError;
```

- [ ] **Step 5: Register audio module in `src/lib.rs`**

Add `pub mod audio;` after the existing module declarations. File becomes:

```rust
pub mod app;
pub mod audio;
pub mod tray;
```

- [ ] **Step 6: Generate test fixture audio files**

Run these commands to create small 1-second 440Hz sine waves:

```bash
mkdir -p tests/fixtures

# Mono WAV (48kHz, 16-bit)
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=48000" -ac 1 tests/fixtures/sine_mono.wav

# Stereo WAV (48kHz, 16-bit)
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=48000" -ac 2 tests/fixtures/sine_stereo.wav

# MP3 (48kHz, mono, 128kbps)
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=48000" -ac 1 -b:a 128k tests/fixtures/sine_mono.mp3

# OGG Vorbis (48kHz, mono)
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=48000" -ac 1 -c:a libvorbis tests/fixtures/sine_mono.ogg

# FLAC (48kHz, mono)
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=48000" -ac 1 tests/fixtures/sine_mono.flac

# Corrupt file (invalid data with .mp3 extension)
echo "this is not audio data at all" > tests/fixtures/corrupt.mp3

# Empty file
touch tests/fixtures/empty.wav
```

- [ ] **Step 7: Verify project builds**

Run: `cargo build`
Expected: Compiles with no errors (stubs only, no logic yet).

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock src/audio/ src/lib.rs tests/fixtures/
git commit -m "chore(audio): add symphonia + thiserror deps, module stubs, test fixtures"
```

---

### Task 2: AudioError Enum

**Files:**
- Test: `tests/decoder_test.rs`
- Modify: `src/audio/error.rs`

- [ ] **Step 1: Write failing test — AudioError variants exist and display**

Create `tests/decoder_test.rs`:

```rust
use honkhonk::audio::AudioError;
use std::io;

#[test]
fn audio_error_file_open_displays_path_context() {
    let err = AudioError::FileOpen(io::Error::new(io::ErrorKind::NotFound, "gone"));
    let msg = err.to_string();
    assert!(msg.contains("open"), "expected 'open' in: {msg}");
}

#[test]
fn audio_error_unsupported_format_displays() {
    let err = AudioError::UnsupportedFormat;
    let msg = err.to_string();
    assert!(msg.contains("unsupported") || msg.contains("format"), "got: {msg}");
}

#[test]
fn audio_error_no_track_displays() {
    let err = AudioError::NoTrack;
    let msg = err.to_string();
    assert!(msg.contains("track"), "got: {msg}");
}

#[test]
fn audio_error_missing_codec_params_displays() {
    let err = AudioError::MissingCodecParams;
    let msg = err.to_string();
    assert!(!msg.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test decoder_test`
Expected: FAIL — `AudioError::Todo` exists but `FileOpen`, `UnsupportedFormat`, `NoTrack`, `MissingCodecParams` do not.

- [ ] **Step 3: Implement AudioError**

Replace `src/audio/error.rs`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("failed to open audio file")]
    FileOpen(#[source] std::io::Error),

    #[error("unsupported audio format")]
    UnsupportedFormat,

    #[error("no audio track found in file")]
    NoTrack,

    #[error("missing codec parameters (sample rate or channels)")]
    MissingCodecParams,

    #[error("failed to create audio decoder")]
    DecoderInit(#[source] symphonia::core::errors::Error),

    #[error("decode error")]
    Decode(#[source] symphonia::core::errors::Error),
}
```

Also update `src/audio/decoder.rs` to remove the `Todo` variant usage — change the stub to:

```rust
use super::error::AudioError;

pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: std::time::Duration,
}

pub fn decode(_path: &std::path::Path) -> Result<DecodedAudio, AudioError> {
    Err(AudioError::UnsupportedFormat)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test decoder_test`
Expected: All 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/audio/error.rs src/audio/decoder.rs tests/decoder_test.rs
git commit -m "feat(audio): add AudioError enum with thiserror"
```

---

### Task 3: Decode WAV to PCM — Core Implementation

**Files:**
- Test: `tests/decoder_test.rs`
- Modify: `src/audio/decoder.rs`

- [ ] **Step 1: Write failing test — decode mono WAV**

Append to `tests/decoder_test.rs`:

```rust
use honkhonk::audio::{decode, DecodedAudio};
use std::path::Path;

#[test]
fn decode_mono_wav_returns_correct_metadata() {
    let path = Path::new("tests/fixtures/sine_mono.wav");
    let audio = decode(path).expect("decode mono WAV failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty(), "samples should not be empty");

    let expected_sample_count = 48000; // 1 second * 48kHz * 1 channel
    let tolerance = expected_sample_count / 10; // 10% tolerance for codec framing
    let diff = (audio.samples.len() as i64 - expected_sample_count as i64).unsigned_abs() as usize;
    assert!(
        diff < tolerance,
        "expected ~{expected_sample_count} samples, got {}",
        audio.samples.len()
    );
}

#[test]
fn decode_mono_wav_returns_valid_duration() {
    let path = Path::new("tests/fixtures/sine_mono.wav");
    let audio = decode(path).expect("decode mono WAV failed");

    let duration_secs = audio.duration.as_secs_f64();
    assert!(
        (duration_secs - 1.0).abs() < 0.1,
        "expected ~1.0s duration, got {duration_secs}s"
    );
}

#[test]
fn decode_mono_wav_samples_in_valid_range() {
    let path = Path::new("tests/fixtures/sine_mono.wav");
    let audio = decode(path).expect("decode mono WAV failed");

    for (i, &sample) in audio.samples.iter().enumerate() {
        assert!(
            (-1.0..=1.0).contains(&sample),
            "sample {i} out of range: {sample}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test decoder_test decode_mono_wav`
Expected: FAIL — `decode()` returns `Err(UnsupportedFormat)`.

- [ ] **Step 3: Implement decode()**

Replace `src/audio/decoder.rs`:

```rust
use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::error::AudioError;

pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Duration,
}

pub fn decode(path: &Path) -> Result<DecodedAudio, AudioError> {
    let file = std::fs::File::open(path).map_err(AudioError::FileOpen)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|_| AudioError::UnsupportedFormat)?;

    let mut format = probed.format;

    let track = format.default_track().ok_or(AudioError::NoTrack)?;
    let track_id = track.id;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or(AudioError::MissingCodecParams)?;

    let channels = track
        .codec_params
        .channels
        .map(|ch| ch.count() as u16)
        .ok_or(AudioError::MissingCodecParams)?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(AudioError::DecoderInit)?;

    let mut all_samples: Vec<f32> = Vec::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(AudioError::Decode(e)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder.decode(&packet).map_err(AudioError::Decode)?;

        let spec = *decoded.spec();
        let capacity = decoded.capacity();

        let buf = sample_buf.get_or_insert_with(|| {
            SampleBuffer::<f32>::new(capacity as u64, spec)
        });

        buf.copy_interleaved_ref(decoded);
        all_samples.extend_from_slice(buf.samples());
    }

    let total_frames = all_samples.len() as u64 / channels as u64;
    let duration = Duration::from_secs_f64(total_frames as f64 / sample_rate as f64);

    Ok(DecodedAudio {
        samples: all_samples,
        sample_rate,
        channels,
        duration,
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test decoder_test decode_mono_wav`
Expected: All 3 WAV tests PASS.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 6: Commit**

```bash
git add src/audio/decoder.rs tests/decoder_test.rs
git commit -m "feat(audio): decode WAV files to interleaved f32 PCM via symphonia"
```

---

### Task 4: Multi-Format Decode — MP3, OGG, FLAC

**Files:**
- Test: `tests/decoder_test.rs`
- (No implementation changes expected — symphonia probes format automatically)

- [ ] **Step 1: Write failing tests — MP3, OGG, FLAC**

Append to `tests/decoder_test.rs`:

```rust
#[test]
fn decode_mp3_returns_correct_metadata() {
    let path = Path::new("tests/fixtures/sine_mono.mp3");
    let audio = decode(path).expect("decode MP3 failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty());

    let duration_secs = audio.duration.as_secs_f64();
    assert!(
        (duration_secs - 1.0).abs() < 0.15,
        "expected ~1.0s, got {duration_secs}s (MP3 framing tolerance)"
    );
}

#[test]
fn decode_ogg_returns_correct_metadata() {
    let path = Path::new("tests/fixtures/sine_mono.ogg");
    let audio = decode(path).expect("decode OGG failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty());

    let duration_secs = audio.duration.as_secs_f64();
    assert!(
        (duration_secs - 1.0).abs() < 0.1,
        "expected ~1.0s, got {duration_secs}s"
    );
}

#[test]
fn decode_flac_returns_correct_metadata() {
    let path = Path::new("tests/fixtures/sine_mono.flac");
    let audio = decode(path).expect("decode FLAC failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 1);
    assert!(!audio.samples.is_empty());

    let duration_secs = audio.duration.as_secs_f64();
    assert!(
        (duration_secs - 1.0).abs() < 0.05,
        "expected ~1.0s, got {duration_secs}s (FLAC is lossless, tight tolerance)"
    );
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test decoder_test decode_mp3 decode_ogg decode_flac`
Expected: All 3 PASS (symphonia auto-detects format from probe + hint). If any fail, investigate and fix in Step 3.

- [ ] **Step 3: Fix if needed**

If any format fails, the issue is likely a missing symphonia feature flag. Verify `Cargo.toml` has all required features: `mp3`, `ogg`, `vorbis`, `flac`, `wav`, `pcm`.

- [ ] **Step 4: Commit**

```bash
git add tests/decoder_test.rs
git commit -m "test(audio): verify MP3, OGG, FLAC decode to PCM"
```

---

### Task 5: Stereo Decode

**Files:**
- Test: `tests/decoder_test.rs`

- [ ] **Step 1: Write failing test — stereo WAV**

Append to `tests/decoder_test.rs`:

```rust
#[test]
fn decode_stereo_wav_returns_two_channels() {
    let path = Path::new("tests/fixtures/sine_stereo.wav");
    let audio = decode(path).expect("decode stereo WAV failed");

    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.channels, 2);

    // Stereo: samples are interleaved [L, R, L, R, ...]
    // 1 second * 48kHz * 2 channels = 96000 samples
    let expected = 96000;
    let tolerance = expected / 10;
    let diff = (audio.samples.len() as i64 - expected as i64).unsigned_abs() as usize;
    assert!(
        diff < tolerance,
        "expected ~{expected} interleaved samples, got {}",
        audio.samples.len()
    );
}

#[test]
fn decode_stereo_wav_duration_matches_mono() {
    let mono = decode(Path::new("tests/fixtures/sine_mono.wav"))
        .expect("decode mono failed");
    let stereo = decode(Path::new("tests/fixtures/sine_stereo.wav"))
        .expect("decode stereo failed");

    let diff = (mono.duration.as_secs_f64() - stereo.duration.as_secs_f64()).abs();
    assert!(
        diff < 0.05,
        "mono ({:.3}s) and stereo ({:.3}s) durations should match",
        mono.duration.as_secs_f64(),
        stereo.duration.as_secs_f64()
    );
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test decoder_test decode_stereo`
Expected: PASS (interleaved output handles stereo naturally).

- [ ] **Step 3: Commit**

```bash
git add tests/decoder_test.rs
git commit -m "test(audio): verify stereo decode with correct channel count and interleaving"
```

---

### Task 6: Error Handling — Corrupt, Missing, Empty Files

**Files:**
- Test: `tests/decoder_test.rs`

- [ ] **Step 1: Write failing tests — error paths**

Append to `tests/decoder_test.rs`:

```rust
#[test]
fn decode_nonexistent_file_returns_file_open_error() {
    let result = decode(Path::new("tests/fixtures/does_not_exist.wav"));
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(
        matches!(err, AudioError::FileOpen(_)),
        "expected FileOpen, got: {err}"
    );
}

#[test]
fn decode_corrupt_file_returns_error() {
    let result = decode(Path::new("tests/fixtures/corrupt.mp3"));
    assert!(result.is_err(), "corrupt file should not decode successfully");
}

#[test]
fn decode_empty_file_returns_error() {
    let result = decode(Path::new("tests/fixtures/empty.wav"));
    assert!(result.is_err(), "empty file should not decode successfully");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test decoder_test error`
Expected: PASS — `FileOpen` for missing file, `UnsupportedFormat` or `Decode` for corrupt/empty.

- [ ] **Step 3: Commit**

```bash
git add tests/decoder_test.rs
git commit -m "test(audio): verify error handling for corrupt, missing, and empty files"
```

---

### Task 7: Sample Value Validation — Known Signal Verification

**Files:**
- Test: `tests/decoder_test.rs`

- [ ] **Step 1: Write failing test — f32 sample range and signal presence**

Append to `tests/decoder_test.rs`:

```rust
#[test]
fn decode_wav_samples_contain_nonzero_signal() {
    let audio = decode(Path::new("tests/fixtures/sine_mono.wav"))
        .expect("decode failed");

    let max_abs = audio
        .samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0_f32, f32::max);

    // 440Hz sine wave should have significant amplitude
    assert!(
        max_abs > 0.5,
        "expected peak amplitude > 0.5, got {max_abs} — signal may be silent or corrupt"
    );
}

#[test]
fn decode_all_formats_produce_similar_sample_counts() {
    let wav = decode(Path::new("tests/fixtures/sine_mono.wav")).expect("WAV");
    let mp3 = decode(Path::new("tests/fixtures/sine_mono.mp3")).expect("MP3");
    let ogg = decode(Path::new("tests/fixtures/sine_mono.ogg")).expect("OGG");
    let flac = decode(Path::new("tests/fixtures/sine_mono.flac")).expect("FLAC");

    // All should be ~48000 samples (1s * 48kHz * 1ch)
    // MP3 has encoder padding so wider tolerance
    let expected = 48000_usize;
    for (name, audio) in [("WAV", &wav), ("MP3", &mp3), ("OGG", &ogg), ("FLAC", &flac)] {
        let diff = (audio.samples.len() as i64 - expected as i64).unsigned_abs() as usize;
        assert!(
            diff < 7000,
            "{name}: expected ~{expected} samples, got {} (diff {diff})",
            audio.samples.len()
        );
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test decoder_test`
Expected: All tests PASS.

- [ ] **Step 3: Run full check suite**

Run: `cargo clippy -- -D warnings && cargo test`
Expected: Zero warnings, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/decoder_test.rs
git commit -m "test(audio): validate sample values and cross-format consistency"
```

---

## Summary

| Task | What | Files | Tests |
|------|------|-------|-------|
| 1 | Deps + stubs + fixtures | Cargo.toml, src/audio/*, tests/fixtures/ | cargo build |
| 2 | AudioError enum | src/audio/error.rs | 4 error display tests |
| 3 | decode() for WAV | src/audio/decoder.rs | 3 WAV tests |
| 4 | MP3, OGG, FLAC | (tests only) | 3 format tests |
| 5 | Stereo handling | (tests only) | 2 stereo tests |
| 6 | Error paths | (tests only) | 3 error tests |
| 7 | Signal validation | (tests only) | 2 signal tests |

**Total: ~250 LOC implementation, ~180 LOC tests, 7 commits.**

**Out of scope:** PipeWire integration, playback, streaming/lazy decode, metadata extraction (title/artist), resampling. These belong to issues #3 and #5.
