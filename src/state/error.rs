use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("failed to serialize config")]
    Serialize(#[source] serde_json::Error),

    #[error("failed to deserialize config")]
    Deserialize(#[source] serde_json::Error),

    #[error("failed to create directory: {path}")]
    DirectoryCreation {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
