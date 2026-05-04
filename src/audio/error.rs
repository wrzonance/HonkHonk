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
