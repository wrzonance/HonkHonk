use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error: {path}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to serialize config: {path}")]
    Serialize {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to deserialize config: {path}")]
    Deserialize {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to create directory: {path}")]
    DirectoryCreation {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("unable to determine XDG config directory")]
    NoConfigDir,

    #[error("library scan error: {0}")]
    ScanEntry(String),
}
