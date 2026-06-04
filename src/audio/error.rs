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

    #[error("failed to resolve XDG config directory for PipeWire conf.d")]
    ConfdNoConfigDir,

    #[error("failed to create PipeWire conf.d directory at {path}")]
    ConfdDirCreate {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write PipeWire conf.d file at {path}")]
    ConfdWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Error, Debug)]
pub enum EffectsError {
    #[error("effect chain exceeds maximum length of {max} (got {got})")]
    ChainTooLong { max: usize, got: usize },

    #[error("unknown parameter {param:?}")]
    ParamUnknown { param: String },

    #[error("effect index {index} out of range (chain length {len})")]
    IndexOutOfRange { index: usize, len: usize },
}

#[cfg(test)]
mod effects_error_tests {
    use super::*;

    #[test]
    fn effects_error_chain_too_long_is_constructible() {
        let e = EffectsError::ChainTooLong { max: 16, got: 17 };
        assert!(e.to_string().contains("16"));
    }

    #[test]
    fn effects_error_param_unknown_is_constructible() {
        let e = EffectsError::ParamUnknown {
            param: "gain".into(),
        };
        assert!(e.to_string().contains("gain"));
    }

    #[test]
    fn effects_error_index_out_of_range_is_constructible() {
        let e = EffectsError::IndexOutOfRange { index: 3, len: 2 };
        assert!(e.to_string().contains("3"));
    }
}
