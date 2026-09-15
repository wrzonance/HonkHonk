use super::{ProcessingError, fingerprint, measure_gain};
use crate::audio::{CachedPcm, decode};
use std::{path::Path, sync::Arc};

#[derive(Debug, Clone, PartialEq)]
pub struct AudioAnalysis {
    pub fingerprint: String,
    pub normalization_gain: f32,
    pub repaired_channel: bool,
}

impl Default for AudioAnalysis {
    fn default() -> Self {
        Self {
            fingerprint: String::new(),
            normalization_gain: 1.0,
            repaired_channel: false,
        }
    }
}

/// Run only on a blocking worker. Verify file bytes stayed stable across decode;
/// controls must never be saved against a different file's content identity.
pub fn decode_cached(path: &Path) -> Result<CachedPcm, ProcessingError> {
    let identity = fingerprint(path)?;
    let audio = decode(path)?;
    if fingerprint(path)? != identity {
        return Err(ProcessingError::FileChanged);
    }
    let normalization_gain = measure_gain(&audio.samples, audio.sample_rate, audio.channels)?;
    Ok(CachedPcm {
        samples: Arc::new(audio.samples),
        sample_rate: audio.sample_rate,
        channels: audio.channels,
        duration: audio.duration,
        analysis: AudioAnalysis {
            fingerprint: identity,
            normalization_gain,
            repaired_channel: audio.repaired_channel,
        },
    })
}
