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

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum EngineErrorEvent {
    #[error("{detail}")]
    EngineInitialization { detail: String },

    #[error("conf.d write: {detail}")]
    ConfdWrite { detail: String },

    #[error("conf.d path: {detail}")]
    ConfdPath { detail: String },

    #[error("set effect bypass (index {index}): {detail}")]
    EffectBypass { index: usize, detail: String },

    #[error("set effect param (index {index}, param {param:?}): {detail}")]
    EffectParam {
        index: usize,
        param: String,
        detail: String,
    },

    #[error("monitor stream rebuild: {detail}")]
    MonitorStreamRebuild { detail: String },

    #[error("virtual sink not yet registered")]
    VirtualSinkNotRegistered,

    #[error("{detail}")]
    SinkStreamCreation { detail: String },

    #[error("monitor stream unavailable: {detail}")]
    MonitorStreamUnavailable { detail: String },
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

    #[error("effect chain error")]
    EffectChain(#[from] EffectsError),
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

#[cfg(test)]
mod engine_error_event_tests {
    use super::*;

    #[allow(
        clippy::too_many_lines,
        reason = "display regression enumerates every public EngineErrorEvent variant"
    )]
    #[test]
    fn engine_error_event_display_preserves_existing_log_text() {
        let cases = [
            (
                EngineErrorEvent::EngineInitialization {
                    detail: "failed to initialize PipeWire: main loop: boom".into(),
                },
                "failed to initialize PipeWire: main loop: boom",
            ),
            (
                EngineErrorEvent::ConfdWrite {
                    detail: "failed to write PipeWire conf.d file at /tmp/honkhonk.conf".into(),
                },
                "conf.d write: failed to write PipeWire conf.d file at /tmp/honkhonk.conf",
            ),
            (
                EngineErrorEvent::ConfdPath {
                    detail: "failed to resolve XDG config directory for PipeWire conf.d".into(),
                },
                "conf.d path: failed to resolve XDG config directory for PipeWire conf.d",
            ),
            (
                EngineErrorEvent::EffectBypass {
                    index: 1,
                    detail: "effect index 1 out of range (chain length 0)".into(),
                },
                "set effect bypass (index 1): effect index 1 out of range (chain length 0)",
            ),
            (
                EngineErrorEvent::MonitorStreamRebuild {
                    detail: "failed to create playback stream: node missing".into(),
                },
                "monitor stream rebuild: failed to create playback stream: node missing",
            ),
            (
                EngineErrorEvent::EffectParam {
                    index: 2,
                    param: "gain".into(),
                    detail: "unknown parameter \"gain\"".into(),
                },
                "set effect param (index 2, param \"gain\"): unknown parameter \"gain\"",
            ),
            (
                EngineErrorEvent::VirtualSinkNotRegistered,
                "virtual sink not yet registered",
            ),
            (
                EngineErrorEvent::SinkStreamCreation {
                    detail: "failed to create playback stream: sink missing".into(),
                },
                "failed to create playback stream: sink missing",
            ),
            (
                EngineErrorEvent::MonitorStreamUnavailable {
                    detail: "failed to create playback stream: monitor missing".into(),
                },
                "monitor stream unavailable: failed to create playback stream: monitor missing",
            ),
        ];

        for (event, expected) in cases {
            assert_eq!(event.to_string(), expected);
        }
    }

    #[test]
    fn audio_error_event_payload_is_typed() {
        let _ = crate::audio::AudioEvent::Error(EngineErrorEvent::VirtualSinkNotRegistered);
    }
}
