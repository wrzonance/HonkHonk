pub mod error;
pub mod portal;

pub use error::PortalError;

#[derive(Debug, Clone, PartialEq)]
pub enum ShortcutsStatus {
    Initializing,
    Active,
    Unavailable(String),
}

#[derive(Debug, Clone)]
pub enum ShortcutEvent {
    Ready,
    /// The stream's command sender — store this to send commands to the portal.
    Handle(tokio::sync::mpsc::Sender<PortalCommand>),
    Activated(u8),
    /// Initial bindings from BindShortcuts response: (0-indexed slot, trigger string).
    Bindings(Vec<(u8, String)>),
    /// DE changed shortcuts externally (user reconfigured in System Settings).
    Changed(Vec<(u8, String)>),
    Failed(String),
}

/// Commands sent into the running portal stream.
#[derive(Debug, Clone)]
pub enum PortalCommand {
    ConfigureShortcuts,
}

/// Newtype wrapping `tokio::sync::mpsc::Sender<PortalCommand>` so it can be
/// included in `Message`, which derives `PartialEq`. Senders are never
/// meaningfully equal; this impl always returns `false`.
#[derive(Debug, Clone)]
pub struct PortalCmdSender(pub tokio::sync::mpsc::Sender<PortalCommand>);

impl PartialEq for PortalCmdSender {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}
