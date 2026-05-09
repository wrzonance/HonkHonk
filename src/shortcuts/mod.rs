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
    Activated(u8),               // 0-indexed slot (0 = Slot 1)
    Bindings(Vec<(u8, String)>), // (0-indexed slot, trigger string e.g. "Meta+1")
    Failed(String),
}
