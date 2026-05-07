#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("portal connection failed: {0}")]
    Connection(#[from] ashpd::Error),
    #[error("session creation failed: {0}")]
    Session(String),
    #[error("shortcut registration failed: {0}")]
    Registration(String),
}
