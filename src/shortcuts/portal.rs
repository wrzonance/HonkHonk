use iced::futures::Stream;

use super::ShortcutEvent;

/// Returns a stream of shortcut events. Yields Ready on success, then
/// Activated events. Yields Failed(reason) if portal is unavailable.
pub async fn shortcut_stream() -> impl Stream<Item = ShortcutEvent> {
    use iced::futures::stream;
    // Stub — real implementation comes later
    stream::empty()
}
