use thiserror::Error;

/// Structured failure modes for the external-stream watcher (issue #26).
///
/// Crossed via `AudioError::StreamWatcherInit` so callers can match on
/// the underlying cause instead of parsing an opaque message string.
#[derive(Error, Debug)]
pub enum WatcherError {
    /// `core.get_registry_rc()` failed during watcher startup.
    #[error("failed to acquire PipeWire registry")]
    RegistryAcquire(#[source] pipewire::Error),
}

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("failed to open audio file")]
    FileOpen(#[source] std::io::Error),

    #[error("unsupported audio format")]
    UnsupportedFormat(#[source] symphonia::core::errors::Error),

    #[error("no audio track found in file")]
    NoTrack,

    #[error("missing codec parameters (sample rate or channels)")]
    MissingCodecParams,

    #[error("failed to create audio decoder")]
    DecoderInit(#[source] symphonia::core::errors::Error),

    #[error("decode error")]
    Decode(#[source] symphonia::core::errors::Error),

    #[error("failed to initialize PipeWire: {0}")]
    PipeWireInit(String),

    #[error("failed to create virtual sink: {0}")]
    VirtualSinkCreation(String),

    #[error("failed to create virtual source: {0}")]
    VirtualSourceCreation(String),

    #[error("failed to create audio link: {0}")]
    LinkCreation(String),

    #[error("failed to spawn audio thread")]
    ThreadSpawn(#[source] std::io::Error),

    #[error("failed to create playback stream: {0}")]
    StreamCreation(String),

    #[error("stream watcher initialization failed")]
    StreamWatcherInit(#[source] WatcherError),
}
