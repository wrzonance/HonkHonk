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
