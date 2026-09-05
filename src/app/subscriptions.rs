//! Portal, duration-scan, window and animation subscriptions.

use super::*;

fn shortcuts_stream_sub(
    window_id: Option<ashpd::WindowIdentifier>,
) -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    use iced::futures::StreamExt;
    iced::stream::channel(16, async move |mut tx| {
        use crate::shortcuts::{ShortcutEvent, portal};
        let stream = portal::shortcut_stream(window_id);
        let mut stream = std::pin::pin!(stream);
        while let Some(ev) = stream.next().await {
            let msg = match ev {
                ShortcutEvent::Ready => Message::ShortcutsReady,
                ShortcutEvent::Handle(sender) => {
                    Message::ShortcutHandle(crate::shortcuts::PortalCmdSender(sender))
                }
                ShortcutEvent::ConfigureAvailable(v) => Message::ShortcutsConfigureAvailable(v),
                ShortcutEvent::Activated(i) => Message::ShortcutActivated(i),
                ShortcutEvent::Bindings(b) => Message::ShortcutBindingsUpdated(b),
                ShortcutEvent::Changed(b) => Message::ShortcutBindingsUpdated(b),
                ShortcutEvent::Failed(r) => Message::ShortcutsUnavailable(r),
            };
            if tx.send(msg).await.is_err() {
                break;
            }
        }
        // Stream ended unexpectedly (portal crashed mid-session). Notify the UI
        // so the unavailability banner appears, then park to keep the subscription alive.
        let _ = tx
            .send(Message::ShortcutsUnavailable(
                "portal connection lost".into(),
            ))
            .await;
        iced::futures::future::pending::<()>().await;
    })
}

/// Zero-arg wrapper for `Subscription::run` (which requires a fn pointer, not a closure).
fn shortcuts_stream_sub_none() -> impl iced::futures::Stream<Item = Message> {
    shortcuts_stream_sub(None)
}

/// Builder for the one-shot duration scan subscription.
///
/// Returns a `BoxStream` (concrete type) so it can be used as `fn(&D) -> S`
/// with `Subscription::run_with`, which requires a concrete `S: Stream`.
fn duration_scan_builder(
    pairs: &std::sync::Arc<Vec<(String, std::path::PathBuf)>>,
) -> iced::futures::stream::BoxStream<'static, Message> {
    let pairs = std::sync::Arc::clone(pairs);
    Box::pin(iced::stream::channel(1, async move |mut tx| {
        use iced::futures::SinkExt;
        let owned = (*pairs).clone();
        let map =
            tokio::task::spawn_blocking(move || crate::state::library::probe_durations(owned))
                .await
                .unwrap_or_default();
        let _ = tx.send(Message::DurationsLoaded(map)).await;
        iced::futures::future::pending::<()>().await;
    }))
}

impl HonkHonk {
    pub fn subscription(&self) -> Subscription<Message> {
        let shortcuts = Subscription::run(shortcuts_stream_sub_none);

        let tray_poll = iced::time::every(Duration::from_millis(100)).map(|_| Message::TrayPoll);

        let events = iced::event::listen_with(|event, status, _window_id| {
            if let Some(text) = filtering::type_to_filter_text(&event, status) {
                return Some(Message::TypeToFilter(text));
            }

            match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    Some(Message::Import(import::ImportMessage::Drop(path)))
                }
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) => Some(if status == iced::event::Status::Captured {
                    Message::CapturedEscapePressed
                } else {
                    Message::EscapePressed
                }),
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::CursorMoved(position))
                }
                iced::Event::Window(iced::window::Event::Opened { size, .. }) => {
                    Some(Message::WindowResized(size.width, size.height))
                }
                iced::Event::Window(iced::window::Event::Resized(size)) => {
                    Some(Message::WindowResized(size.width, size.height))
                }
                // Route the window-manager close through the same quit path (audio
                // shutdown + config save) instead of iced's default auto-close
                // (which is disabled via window::Settings::exit_on_close_request).
                iced::Event::Window(iced::window::Event::CloseRequested) => Some(Message::Quit),
                _ => None,
            }
        });

        let mut subs = vec![shortcuts, tray_poll, events];

        if !self.durations_loaded {
            subs.push(Subscription::run_with(
                std::sync::Arc::clone(&self.duration_scan_pairs),
                duration_scan_builder,
            ));
        }

        // Vsync-paced playhead animation — subscribed ONLY while a sound plays so
        // an idle tray app never repaints. `window::frames()` yields one `Instant`
        // per refresh; subscriptions are re-evaluated each update, so this drops
        // out automatically when playback ends. No fps cap (let it fly at refresh).
        if self.frame_subscription_needed() {
            subs.push(iced::window::frames().map(Message::Frame));
        }

        if self.notices.has_expiring() {
            subs.push(iced::time::every(Duration::from_millis(250)).map(Message::NoticeTick));
        }

        Subscription::batch(subs)
    }
}
