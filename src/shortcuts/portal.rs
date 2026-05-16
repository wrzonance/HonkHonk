use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use ashpd::desktop::CreateSessionOptions;
use ashpd::WindowIdentifier;
use iced::futures::{SinkExt, Stream, StreamExt};
use tokio::sync::mpsc;

use super::{PortalCommand, ShortcutEvent};

const SLOT_COUNT: u8 = 20;

/// Returns a stream of shortcut events.
///
/// Yields `ShortcutEvent::Handle` once with the command sender, then
/// `ShortcutEvent::Bindings` with current key assignments, then
/// `ShortcutEvent::Ready` once the portal session is established, then
/// `ShortcutEvent::Activated(idx)` on each trigger press.
/// Yields `ShortcutEvent::Failed(reason)` once on error, then ends.
pub fn shortcut_stream(
    window_id: Option<WindowIdentifier>,
    initial_desired: [Option<String>; 20],
) -> impl Stream<Item = ShortcutEvent> {
    iced::stream::channel(32, async move |mut tx| {
        macro_rules! bail {
            ($ctx:expr, $err:expr) => {{
                let _ = tx
                    .send(ShortcutEvent::Failed(format!("{}: {}", $ctx, $err)))
                    .await;
                return;
            }};
        }

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<PortalCommand>(8);

        let proxy = match GlobalShortcuts::new().await {
            Ok(p) => p,
            Err(e) => bail!("connecting to portal", e),
        };

        let session = match proxy.create_session(CreateSessionOptions::default()).await {
            Ok(s) => s,
            Err(e) => bail!("creating session", e),
        };

        let mut current_desired = initial_desired;
        let shortcuts = build_shortcuts(&current_desired);

        let req = match proxy
            .bind_shortcuts(
                &session,
                &shortcuts,
                window_id.as_ref(),
                BindShortcutsOptions::default(),
            )
            .await
        {
            Ok(req) => req,
            Err(e) => bail!("binding shortcuts", e),
        };

        let info = match req.response() {
            Ok(info) => info,
            Err(e) => bail!("reading bind response", e),
        };

        let bindings: Vec<(u8, String)> = info
            .shortcuts()
            .iter()
            .filter_map(|s| parse_binding(s.id(), s.trigger_description()))
            .collect();

        let _ = tx.send(ShortcutEvent::Handle(cmd_tx)).await;
        let _ = tx.send(ShortcutEvent::Bindings(bindings)).await;

        let activated = match proxy.receive_activated().await {
            Ok(s) => s,
            Err(e) => bail!("subscribing to activations", e),
        };

        let changed = match proxy.receive_shortcuts_changed().await {
            Ok(s) => s,
            Err(e) => bail!("subscribing to shortcut changes", e),
        };

        let _ = tx.send(ShortcutEvent::Ready).await;

        tokio::pin!(activated);
        tokio::pin!(changed);

        loop {
            tokio::select! {
                Some(event) = activated.next() => {
                    if let Some(idx) = parse_slot_index(event.shortcut_id()) {
                        if tx.send(ShortcutEvent::Activated(idx)).await.is_err() {
                            break;
                        }
                    }
                }
                Some(changed_event) = changed.next() => {
                    let bindings: Vec<(u8, String)> = changed_event
                        .shortcuts()
                        .iter()
                        .filter_map(|s| parse_binding(s.id(), s.trigger_description()))
                        .collect();
                    if tx.send(ShortcutEvent::Changed(bindings)).await.is_err() {
                        break;
                    }
                }
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        PortalCommand::RebindSlot { idx, trigger } => {
                            current_desired[idx as usize] = Some(trigger);
                            let shortcuts = build_shortcuts(&current_desired);
                            let rebind_result = proxy
                                .bind_shortcuts(
                                    &session,
                                    &shortcuts,
                                    window_id.as_ref(),
                                    BindShortcutsOptions::default(),
                                )
                                .await;
                            let event = match rebind_result {
                                Ok(req) => match req.response() {
                                    Ok(info) => {
                                        let bindings = info
                                            .shortcuts()
                                            .iter()
                                            .filter_map(|s| {
                                                parse_binding(s.id(), s.trigger_description())
                                            })
                                            .collect();
                                        ShortcutEvent::RebindResult {
                                            changed_idx: idx,
                                            bindings,
                                        }
                                    }
                                    Err(e) => ShortcutEvent::Failed(format!(
                                        "rebind response error: {e}"
                                    )),
                                },
                                Err(e) => {
                                    ShortcutEvent::Failed(format!("rebind portal error: {e}"))
                                }
                            };
                            if tx.send(event).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                else => break,
            }
        }
    })
}

/// Builds the full 20-slot shortcut list with preferred_trigger hints.
fn build_shortcuts(desired: &[Option<String>; 20]) -> Vec<NewShortcut> {
    (1..=SLOT_COUNT)
        .map(|n| {
            let idx = (n - 1) as usize;
            NewShortcut::new(format!("slot-{n}"), format!("HonkHonk Slot {n}"))
                .preferred_trigger(desired[idx].as_deref())
        })
        .collect()
}

/// Returns `Some((0-indexed slot, trigger))` for a valid, non-empty binding.
fn parse_binding(id: &str, trigger: &str) -> Option<(u8, String)> {
    if trigger.is_empty() {
        return None;
    }
    let idx = parse_slot_index(id)?;
    Some((idx, trigger.to_owned()))
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
    use super::{build_shortcuts, parse_binding, parse_slot_index};

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

    #[test]
    fn bindings_parse_skips_empty_triggers() {
        assert_eq!(
            parse_binding("slot-1", "Meta+1"),
            Some((0, "Meta+1".to_owned()))
        );
        assert_eq!(
            parse_binding("slot-3", "Ctrl+3"),
            Some((2, "Ctrl+3".to_owned()))
        );
        assert_eq!(parse_binding("slot-1", ""), None);
        assert_eq!(parse_binding("slot-0", "X"), None);
    }

    #[test]
    fn build_shortcuts_returns_20_entries() {
        let desired: [Option<String>; 20] = std::array::from_fn(|_| None);
        let shortcuts = build_shortcuts(&desired);
        assert_eq!(shortcuts.len(), 20);
    }

    #[test]
    fn build_shortcuts_with_some_desired_compiles() {
        let mut desired: [Option<String>; 20] = std::array::from_fn(|_| None);
        desired[0] = Some("Meta+1".into());
        desired[4] = Some("Ctrl+Alt+F".into());
        let shortcuts = build_shortcuts(&desired);
        assert_eq!(shortcuts.len(), 20);
    }
}
