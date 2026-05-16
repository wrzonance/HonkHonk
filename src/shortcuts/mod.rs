pub mod error;
pub mod portal;
pub mod capture;

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
    /// The stream's command sender — store this to send rebind requests.
    Handle(tokio::sync::mpsc::Sender<PortalCommand>),
    Activated(u8),
    /// Initial bindings from BindShortcuts response: (0-indexed slot, trigger string).
    Bindings(Vec<(u8, String)>),
    /// Result of a RebindSlot command: full binding set returned by portal.
    RebindResult {
        changed_idx: u8,
        bindings: Vec<(u8, String)>,
    },
    /// DE changed shortcuts externally (user reconfigured in System Settings).
    Changed(Vec<(u8, String)>),
    Failed(String),
}

/// Commands sent into the running portal stream.
#[derive(Debug, Clone)]
pub enum PortalCommand {
    RebindSlot { idx: u8, trigger: String },
}

/// Newtype wrapping `tokio::sync::mpsc::Sender<PortalCommand>` so it can be
/// carried inside `Message`, which derives `PartialEq`. Two senders are never
/// meaningfully equal, so `PartialEq` is always `false`.
#[derive(Debug, Clone)]
pub struct PortalCmdSender(pub tokio::sync::mpsc::Sender<PortalCommand>);

impl PartialEq for PortalCmdSender {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindFeedback {
    #[default]
    Unset,
    Saved,
    NotSaved,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_feedback_default_is_unset() {
        assert_eq!(BindFeedback::default(), BindFeedback::Unset);
    }
}
