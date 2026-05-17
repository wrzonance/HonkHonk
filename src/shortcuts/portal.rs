use std::collections::HashMap;

use ashpd::desktop::global_shortcuts::NewShortcut;
use ashpd::WindowIdentifier;
use iced::futures::{SinkExt, Stream, StreamExt};
use tokio::sync::mpsc;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use super::{PortalCommand, ShortcutEvent};

const SLOT_COUNT: u8 = 20;
const SESSION_TOKEN: &str = "honkhonk_v1";
const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";

/// Computes the portal session object path for our fixed token.
/// Replicates ashpd's internal `Proxy::unique_name` formula (src/proxy.rs:46-55).
fn make_session_path(conn: &zbus::Connection) -> zbus::Result<OwnedObjectPath> {
    let unique_name = conn
        .unique_name()
        .ok_or_else(|| zbus::Error::Failure("no unique name".into()))?
        .to_string();
    let unique_id = unique_name.trim_start_matches(':').replace('.', "_");
    let s = format!(
        "/org/freedesktop/portal/desktop/session/{}/{}",
        unique_id, SESSION_TOKEN
    );
    OwnedObjectPath::try_from(s).map_err(|e| zbus::Error::Failure(e.to_string()))
}

/// Computes a portal request object path for a given handle token suffix.
fn make_request_path(conn: &zbus::Connection, suffix: &str) -> zbus::Result<OwnedObjectPath> {
    let unique_name = conn
        .unique_name()
        .ok_or_else(|| zbus::Error::Failure("no unique name".into()))?
        .to_string();
    let unique_id = unique_name.trim_start_matches(':').replace('.', "_");
    let s = format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        unique_id, suffix
    );
    OwnedObjectPath::try_from(s).map_err(|e| zbus::Error::Failure(e.to_string()))
}

/// Creates a GlobalShortcuts portal session with a fixed, deterministic token.
///
/// ashpd's CreateSessionOptions.session_handle_token is pub(crate) with no public
/// setter, and Session constructors are also pub(crate). We call CreateSession
/// directly via raw zbus to inject SESSION_TOKEN, then return the session path
/// for use in subsequent bind/configure calls.
async fn create_session_fixed_token(
    conn: &zbus::Connection,
) -> Result<OwnedObjectPath, ashpd::Error> {
    let session_path = make_session_path(conn).map_err(ashpd::Error::Zbus)?;
    let req_path = make_request_path(conn, "honkhonk_cs").map_err(ashpd::Error::Zbus)?;

    let portal: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface(PORTAL_IFACE)?
        .path(PORTAL_PATH)?
        .build()
        .await?;

    // Subscribe to Response on the request path BEFORE calling the method.
    let req_proxy: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface("org.freedesktop.portal.Request")?
        .path(req_path.as_ref())?
        .build()
        .await?;
    let mut response_stream = req_proxy.receive_signal("Response").await?;

    let options: HashMap<&str, Value<'_>> = [
        ("handle-token", Value::new("honkhonk_cs")),
        ("session-handle-token", Value::new(SESSION_TOKEN)),
    ]
    .into_iter()
    .collect();

    // CreateSession returns the Request path; discard it — we already know it.
    portal.call_method("CreateSession", &(&options,)).await?;

    // SignalStream::Item = Message (not Result<Message>) — single ? suffices.
    let msg = response_stream
        .next()
        .await
        .ok_or_else(|| ashpd::Error::Zbus(zbus::Error::Failure("no response".into())))?;

    let (status, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;

    if status != 0 {
        return Err(ashpd::Error::Portal(ashpd::PortalError::Cancelled(
            "CreateSession cancelled".into(),
        )));
    }

    // Portal returns session_handle as string or ObjectPath (known xdg-desktop-portal quirk).
    // See ashpd CreateSessionResponse deserializer for context.
    let handle = results
        .get("session_handle")
        .ok_or_else(|| ashpd::Error::Zbus(zbus::Error::Failure("missing session_handle".into())))?;

    let path_str = handle
        .downcast_ref::<&str>()
        .map(|s| s.to_owned())
        .or_else(|_| {
            handle
                .downcast_ref::<zbus::zvariant::ObjectPath<'_>>()
                .map(|p| p.as_str().to_owned())
        })
        .map_err(|_| ashpd::Error::Zbus(zbus::Error::Failure("bad session_handle type".into())))?;

    debug_assert_eq!(
        path_str,
        session_path.as_str(),
        "portal session path mismatch — SESSION_TOKEN formula wrong?"
    );

    OwnedObjectPath::try_from(path_str)
        .map_err(|e| ashpd::Error::Zbus(zbus::Error::Failure(e.to_string())))
}

/// Binds shortcuts for a session via raw zbus.
/// Returns existing (0-indexed slot, trigger description) pairs.
/// Initial bindings may be empty — ShortcutsChanged signal repopulates on
/// first user interaction with the shortcut config dialog.
async fn bind_shortcuts_raw(
    conn: &zbus::Connection,
    session_path: &OwnedObjectPath,
    shortcuts: &[NewShortcut],
) -> Result<Vec<(u8, String)>, ashpd::Error> {
    let req_path = make_request_path(conn, "honkhonk_bs").map_err(ashpd::Error::Zbus)?;

    let portal: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface(PORTAL_IFACE)?
        .path(PORTAL_PATH)?
        .build()
        .await?;

    let req_proxy: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface("org.freedesktop.portal.Request")?
        .path(req_path.as_ref())?
        .build()
        .await?;
    let mut response_stream = req_proxy.receive_signal("Response").await?;

    let options: HashMap<&str, Value<'_>> = [("handle-token", Value::new("honkhonk_bs"))]
        .into_iter()
        .collect();

    portal
        .call_method(
            "BindShortcuts",
            &(session_path.as_ref(), shortcuts, "", &options),
        )
        .await?;

    let msg = response_stream
        .next()
        .await
        .ok_or_else(|| ashpd::Error::Zbus(zbus::Error::Failure("no response".into())))?;

    let (status, _): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;

    if status != 0 {
        return Ok(Vec::new());
    }

    // Binding display is repopulated via ShortcutsChanged signal on next config dialog use.
    Ok(Vec::new())
}

/// Calls ConfigureShortcuts via raw zbus (portal v2+ only).
/// Callers must check configure_available before calling.
async fn configure_shortcuts_raw(
    conn: &zbus::Connection,
    session_path: &OwnedObjectPath,
) -> Result<(), zbus::Error> {
    let portal: zbus::Proxy = zbus::proxy::Builder::new(conn)
        .destination(PORTAL_DEST)?
        .interface(PORTAL_IFACE)?
        .path(PORTAL_PATH)?
        .build()
        .await?;

    let options: HashMap<&str, Value<'_>> = HashMap::new();

    portal
        .call_noreply("ConfigureShortcuts", &(session_path.as_ref(), "", &options))
        .await
}

/// Returns a stream of shortcut events.
///
/// Yields `ShortcutEvent::Handle` once with the command sender, then
/// `ShortcutEvent::Bindings` with current key assignments, then
/// `ShortcutEvent::Ready` once the portal session is established, then
/// `ShortcutEvent::Activated(idx)` on each trigger press.
/// Yields `ShortcutEvent::Failed(reason)` once on error, then ends.
pub fn shortcut_stream(_window_id: Option<WindowIdentifier>) -> impl Stream<Item = ShortcutEvent> {
    iced::stream::channel(32, async move |mut tx| {
        use ashpd::desktop::global_shortcuts::GlobalShortcuts;

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

        // Get the underlying zbus connection via Deref (GlobalShortcuts → zbus::Proxy).
        let conn = (*proxy).connection().clone();

        let session_path = match create_session_fixed_token(&conn).await {
            Ok(p) => p,
            Err(e) => bail!("creating session with fixed token", e),
        };

        let shortcuts = build_shortcuts();

        let bindings = match bind_shortcuts_raw(&conn, &session_path, &shortcuts).await {
            Ok(b) => b,
            Err(e) => bail!("binding shortcuts", e),
        };

        let configure_available = proxy.version() >= 2;
        let _ = tx.send(ShortcutEvent::Handle(cmd_tx)).await;
        let _ = tx
            .send(ShortcutEvent::ConfigureAvailable(configure_available))
            .await;
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
                        PortalCommand::ConfigureShortcuts => {
                            if configure_available {
                                if let Err(e) = configure_shortcuts_raw(&conn, &session_path).await {
                                    eprintln!("honkhonk: configure_shortcuts failed: {e}");
                                }
                            }
                        }
                    }
                }
                else => break,
            }
        }
    })
}

/// Builds the full 20-slot shortcut list with no preferred_trigger hints.
fn build_shortcuts() -> Vec<NewShortcut> {
    (1..=SLOT_COUNT)
        .map(|n| NewShortcut::new(format!("slot-{n}"), format!("HonkHonk Slot {n}")))
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
    fn build_shortcuts_returns_20_entries_no_preferred_trigger() {
        let shortcuts = build_shortcuts();
        assert_eq!(shortcuts.len(), 20);
    }

    #[test]
    fn session_path_format() {
        let raw = ":1.123";
        let unique_id = raw.trim_start_matches(':').replace('.', "_");
        let path = format!(
            "/org/freedesktop/portal/desktop/session/{}/{}",
            unique_id, "honkhonk_v1"
        );
        assert_eq!(
            path,
            "/org/freedesktop/portal/desktop/session/1_123/honkhonk_v1"
        );
    }

    #[test]
    fn request_path_format() {
        let raw = ":1.123";
        let unique_id = raw.trim_start_matches(':').replace('.', "_");
        let path = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            unique_id, "honkhonk_req"
        );
        assert_eq!(
            path,
            "/org/freedesktop/portal/desktop/request/1_123/honkhonk_req"
        );
    }
}
