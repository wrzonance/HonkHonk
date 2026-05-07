use iced::futures::{SinkExt, Stream, StreamExt};

use super::ShortcutEvent;

const SLOT_COUNT: u8 = 20;

/// Returns a stream of shortcut events.
///
/// Yields `ShortcutEvent::Ready` once the portal session is established, then
/// `ShortcutEvent::Activated(idx)` (0-indexed) on each trigger. Yields
/// `ShortcutEvent::Failed(reason)` once on error, then ends.
///
/// The proxy, session, and activated subscription all live inside the returned
/// stream's async block — no lifetime borrow escapes the function.
pub fn shortcut_stream() -> impl Stream<Item = ShortcutEvent> {
    iced::stream::channel(32, async |mut tx| {
        use ashpd::desktop::global_shortcuts::{
            BindShortcutsOptions, GlobalShortcuts, NewShortcut,
        };
        use ashpd::desktop::CreateSessionOptions;

        macro_rules! bail {
            ($err:expr) => {{
                let _ = tx.send(ShortcutEvent::Failed($err.to_string())).await;
                return;
            }};
        }

        let proxy = match GlobalShortcuts::new().await {
            Ok(p) => p,
            Err(e) => bail!(e),
        };

        let session = match proxy.create_session(CreateSessionOptions::default()).await {
            Ok(s) => s,
            Err(e) => bail!(e),
        };

        let shortcuts: Vec<NewShortcut> = (1..=SLOT_COUNT)
            .map(|n| NewShortcut::new(format!("slot-{n}"), format!("Slot {n}")))
            .collect();

        match proxy
            .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
            .await
        {
            Ok(req) => {
                if let Err(e) = req.response() {
                    bail!(e);
                }
            }
            Err(e) => bail!(e),
        }

        let mut activated = match proxy.receive_activated().await {
            Ok(s) => s,
            Err(e) => bail!(e),
        };

        let _ = tx.send(ShortcutEvent::Ready).await;

        while let Some(event) = activated.next().await {
            if let Some(idx) = parse_slot_index(event.shortcut_id()) {
                if tx.send(ShortcutEvent::Activated(idx)).await.is_err() {
                    break;
                }
            }
        }
    })
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
