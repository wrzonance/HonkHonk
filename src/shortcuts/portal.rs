use iced::futures::{Stream, StreamExt, stream};

use super::ShortcutEvent;
use crate::shortcuts::error::PortalError;

const SLOT_COUNT: u8 = 20;

/// Returns a stream of shortcut events.
///
/// First yields `ShortcutEvent::Ready` on successful portal session setup,
/// then `ShortcutEvent::Activated(index)` (0-indexed) for each triggered
/// shortcut. Yields `ShortcutEvent::Failed(reason)` exactly once on error,
/// then ends.
pub async fn shortcut_stream() -> impl Stream<Item = ShortcutEvent> {
    match init_session().await {
        Ok(activated_stream) => {
            let ready = stream::once(async { ShortcutEvent::Ready });
            let events = activated_stream;
            ready.chain(events).left_stream()
        }
        Err(err) => {
            let msg = err.to_string();
            stream::once(async move { ShortcutEvent::Failed(msg) }).right_stream()
        }
    }
}

/// Initialises the GlobalShortcuts portal session and registers 20 slots.
/// Returns a stream that yields `ShortcutEvent::Activated` on each trigger.
async fn init_session() -> Result<impl Stream<Item = ShortcutEvent>, PortalError> {
    use ashpd::desktop::global_shortcuts::{
        BindShortcutsOptions, GlobalShortcuts, NewShortcut,
    };
    use ashpd::desktop::CreateSessionOptions;

    let proxy = GlobalShortcuts::new()
        .await
        .map_err(PortalError::Connection)?;

    let session = proxy
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|e| PortalError::Session(e.to_string()))?;

    let shortcuts: Vec<NewShortcut> = (1..=SLOT_COUNT)
        .map(|n| NewShortcut::new(format!("slot-{n}"), format!("Slot {n}")))
        .collect();

    proxy
        .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
        .await
        .map_err(|e| PortalError::Registration(e.to_string()))?
        .response()
        .map_err(|e| PortalError::Registration(e.to_string()))?;

    let activated_stream = proxy
        .receive_activated()
        .await
        .map_err(|e| PortalError::Registration(e.to_string()))?;

    let mapped = activated_stream.filter_map(|event| async move {
        parse_slot_index(event.shortcut_id()).map(ShortcutEvent::Activated)
    });

    Ok(mapped)
}

/// Parses "slot-N" → 0-indexed slot index.
///
/// "slot-1" → `Some(0)`, "slot-20" → `Some(19)`, everything else → `None`.
fn parse_slot_index(id: &str) -> Option<u8> {
    let n_str = id.strip_prefix("slot-")?;
    let n: u8 = n_str.parse().ok()?;
    if !(1..=SLOT_COUNT).contains(&n) {
        return None;
    }
    Some(n - 1)
}

#[cfg(test)]
mod tests {
    use super::parse_slot_index;

    #[test]
    fn parse_valid_slot_ids() {
        assert_eq!(parse_slot_index("slot-1"), Some(0));
        assert_eq!(parse_slot_index("slot-10"), Some(9));
        assert_eq!(parse_slot_index("slot-20"), Some(19));
    }

    #[test]
    fn parse_invalid_slot_ids() {
        assert_eq!(parse_slot_index("slot-0"), None);
        assert_eq!(parse_slot_index("slot-21"), None);
        assert_eq!(parse_slot_index("f1"), None);
        assert_eq!(parse_slot_index("slot-"), None);
        assert_eq!(parse_slot_index(""), None);
    }
}
