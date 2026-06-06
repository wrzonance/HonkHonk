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

/// Structured failure modes for the PipeWire router (issue #27).
#[derive(Error, Debug)]
pub enum RouterError {
    /// `core.create_object::<Link>()` failed for a specific port pair.
    #[error("failed to create link from src port {src_port} to sink port {sink_port}")]
    LinkCreation {
        src_port: u32,
        sink_port: u32,
        #[source]
        source: pipewire::Error,
    },

    /// The virtual sink's input ports are not yet known (registry hasn't seen them).
    #[error("virtual sink input ports not yet available")]
    SinkPortsUnavailable,

    /// The source node's output ports are not yet known.
    #[error("source node {node_id} output ports not yet available")]
    SourcePortsUnavailable { node_id: u32 },
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

    #[error("router error")]
    RouterError(#[source] RouterError),
}

#[cfg(test)]
mod router_error_tests {
    use super::*;

    #[test]
    fn router_error_link_creation_is_audio_error() {
        let e = AudioError::RouterError(RouterError::LinkCreation {
            src_port: 1,
            sink_port: 2,
            source: pipewire::Error::CreationFailed,
        });
        let msg = e.to_string();
        assert!(msg.contains("router"), "expected 'router' in: {msg}");
    }

    #[test]
    fn router_error_sink_ports_unavailable_is_constructible() {
        let e = RouterError::SinkPortsUnavailable;
        let msg = e.to_string();
        assert!(msg.contains("sink"), "expected 'sink' in: {msg}");
    }

    #[test]
    fn router_error_source_ports_unavailable_is_constructible() {
        let e = RouterError::SourcePortsUnavailable { node_id: 42 };
        let msg = e.to_string();
        assert!(
            msg.contains("42") || msg.contains("source"),
            "expected node info in: {msg}"
        );
    }
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
