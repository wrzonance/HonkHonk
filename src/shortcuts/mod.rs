pub mod config_ui;
pub mod portal;

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
    /// Whether `configure_shortcuts()` (portal v2) is available.
    /// Emitted once after session setup; re-emitted as false on first failed attempt.
    ConfigureAvailable(bool),
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
/// included in `Message`, which derives `PartialEq`. Two senders to the same
/// channel are equal; senders to different channels are not.
#[derive(Debug, Clone)]
pub struct PortalCmdSender(pub tokio::sync::mpsc::Sender<PortalCommand>);

impl PartialEq for PortalCmdSender {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_channel(&other.0)
    }
}
