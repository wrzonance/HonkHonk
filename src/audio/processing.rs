//! Background loudness measurement and allocation-free playback dynamics.
mod cache;
mod channels;
pub use channels::ChannelLayout;
mod dynamics;
mod settings;
pub use cache::{AudioAnalysis, decode_cached};
pub use dynamics::Dynamics;
pub use settings::{DynamicsSettings, GlobalProcessing, OutputMode, SoundProcessing};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceProcessing {
    pub normalization_gain: f32,
    pub sound: SoundProcessing,
}

impl Default for VoiceProcessing {
    fn default() -> Self {
        Self {
            normalization_gain: 1.0,
            sound: SoundProcessing::default(),
        }
    }
}

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ProcessingError {
    #[error("cannot decode audio: {0}; check the file is readable or re-export as PCM WAV")]
    Decode(#[from] super::AudioError),
    #[error(
        "audio file changed while loading; wait for the download or editor to finish and retry"
    )]
    FileChanged,
    #[error("cannot read audio fingerprint: {0}")]
    Fingerprint(#[from] std::io::Error),
    #[error("audio contains invalid samples or format; re-export as a PCM WAV file")]
    InvalidAudio,
    #[error("cannot measure audio loudness: {0}")]
    Loudness(#[from] ebur128::Error),
}

pub fn fingerprint(path: &Path) -> Result<String, ProcessingError> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Integrated K-weighted, gated loudness towards -18 LUFS, with at most 12 dB
/// boost. No measurable program (silence or <400 ms) means unity gain.
pub fn measure_gain(samples: &[f32], rate: u32, channels: u16) -> Result<f32, ProcessingError> {
    if rate == 0
        || channels == 0
        || !samples.len().is_multiple_of(channels as usize)
        || samples.iter().any(|s| !s.is_finite())
    {
        return Err(ProcessingError::InvalidAudio);
    }
    let mut meter = ebur128::EbuR128::new(channels.into(), rate, ebur128::Mode::I)?;
    meter.add_frames_f32(samples)?;
    let loudness = meter.loudness_global()?;
    Ok(if loudness.is_finite() {
        10.0_f64.powf(((-18.0 - loudness) / 20.0).min(0.6)) as f32
    } else {
        1.0
    })
}

/// Balance control: center is unity; positive values attenuate left. For mono,
/// callers expand to stereo when pan is nonzero. Other channels remain intact.
pub fn pan(samples: &mut [f32], channels: u16, value: f32) {
    if channels < 2 {
        return;
    }
    let value = settings::finite(value, -1.0, 1.0, 0.0);
    for frame in samples.chunks_exact_mut(channels as usize) {
        frame[0] *= 1.0 - value.max(0.0);
        frame[1] *= 1.0 + value.min(0.0);
    }
}
