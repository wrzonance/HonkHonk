use super::*;
use crate::audio::RouterCommand;
use crate::audio::streams::StreamEvent;
use crate::ui::mixer::MixerMessage;

pub(super) fn app() -> HonkHonk {
    let mut app = HonkHonk::new_for_test();
    app.audio = Some(crate::audio::test_handle().0);
    let _ = app.update(Message::AudioEvent(AudioEvent::Stream(added(7))));
    app
}

fn added(id: u32) -> StreamEvent {
    StreamEvent::SourceAdded {
        id,
        name: "Browser".into(),
        app_name: None,
        app_binary: None,
        app_pid: None,
        icon: None,
        media_name: None,
    }
}

pub(super) fn change(app: &mut HonkHonk, message: MixerMessage) {
    let _ = app.update(Message::Mixer(message));
}

#[test]
fn safe_default_confirmation_and_backend_acknowledgement() {
    let mut app = app();
    change(&mut app, MixerMessage::Route(7));
    assert!(app.mixer.confirm.is_none());
    assert!(app.audio.as_ref().unwrap().sent_commands().is_empty());
    change(&mut app, MixerMessage::SafeMode(false));
    change(&mut app, MixerMessage::Route(7));
    assert_eq!(app.mixer.confirm, Some(7));
    assert!(!app.mixer.sources[&7].enabled);
    change(&mut app, MixerMessage::Confirm);
    assert!(matches!(
        app.audio.as_ref().unwrap().sent_commands().last(),
        Some(AudioCommand::Router(RouterCommand::RouteSource {
            source_node_id: 7
        }))
    ));
    assert!(!app.mixer.sources[&7].enabled);
    let _ = app.update(Message::AudioEvent(AudioEvent::RoutingChanged {
        node_id: 7,
        enabled: true,
    }));
    assert!(app.mixer.sources[&7].enabled);
}

#[test]
fn title_updates_coalesce_and_removal_cancels_pending_state() {
    let mut app = app();
    let now = Instant::now();
    for title in [Some("old"), None, Some("last"), None] {
        app.mixer.stream(
            StreamEvent::SourceUpdated {
                id: 7,
                media_name: title.map(str::to_owned),
            },
            now,
        );
    }
    app.mixer.tick(now + Duration::from_millis(199));
    assert_eq!(app.mixer.sources[&7].media_name, None);
    app.mixer.tick(now + Duration::from_millis(200));
    assert_eq!(app.mixer.sources[&7].media_name.as_deref(), Some("last"));
    change(&mut app, MixerMessage::SafeMode(false));
    change(&mut app, MixerMessage::Route(7));
    app.mixer.stream(StreamEvent::SourceRemoved { id: 7 }, now);
    change(&mut app, MixerMessage::Confirm);
    assert!(app.mixer.sources.is_empty());
    assert!(app.mixer.confirm.is_none());
}

#[test]
fn feedback_blocks_reenable_and_monitor_reveal_never_allows_route() {
    let mut app = app();
    change(&mut app, MixerMessage::SafeMode(false));
    let _ = app.update(Message::AudioEvent(AudioEvent::FeedbackDetected {
        suspected_source: Some(crate::audio::FeedbackSource {
            node_id: 7,
            name: "Browser".into(),
        }),
    }));
    change(&mut app, MixerMessage::Route(7));
    assert!(app.mixer.confirm.is_none());
    app.mixer.tick(Instant::now() + Duration::from_secs(3));
    app.mixer.stream(
        StreamEvent::PortAdded {
            id: 8,
            node_id: 7,
            channel: "FL".into(),
            direction: crate::audio::streams::Direction::Output,
            monitor: true,
        },
        Instant::now(),
    );
    change(&mut app, MixerMessage::ShowMonitors(true));
    change(&mut app, MixerMessage::Route(7));
    assert!(app.mixer.confirm.is_none());
    assert!(app.mixer.sources[&7].monitor);
}

#[test]
fn safe_mode_persists_and_is_sent_on_ready() {
    let mut app = app();
    change(&mut app, MixerMessage::SafeMode(false));
    let json = serde_json::to_string(&app.config).unwrap();
    let restored: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(!restored.mixer_safe_mode);
    let mut legacy = serde_json::to_value(restored).unwrap();
    legacy.as_object_mut().unwrap().remove("mixer_safe_mode");
    assert!(
        serde_json::from_value::<AppConfig>(legacy)
            .unwrap()
            .mixer_safe_mode
    );
    let _ = app.update(Message::AudioEvent(AudioEvent::Ready));
    assert!(
        app.audio
            .as_ref()
            .unwrap()
            .sent_commands()
            .iter()
            .any(|command| matches!(
                command,
                AudioCommand::Router(RouterCommand::SetSafeMode(false))
            ))
    );
}

#[test]
fn monitor_port_before_metadata_stays_blocked_and_id_reuse_cleans_up() {
    let mut app = HonkHonk::new_for_test();
    let now = Instant::now();
    app.mixer.stream(
        StreamEvent::PortAdded {
            id: 8,
            node_id: 7,
            channel: "FL".into(),
            direction: crate::audio::Direction::Output,
            monitor: true,
        },
        now,
    );
    app.mixer.stream(added(7), now);
    assert!(app.mixer.sources[&7].monitor);
    app.mixer.stream(StreamEvent::SourceRemoved { id: 7 }, now);
    app.mixer.stream(added(7), now);
    assert!(!app.mixer.sources[&7].monitor);
}

#[test]
fn safe_mode_invalidates_confirmation_and_undo_uses_guarded_backend() {
    let mut app = app();
    change(&mut app, MixerMessage::SafeMode(false));
    change(&mut app, MixerMessage::Route(7));
    change(&mut app, MixerMessage::SafeMode(true));
    change(&mut app, MixerMessage::Confirm);
    assert!(
        !app.audio
            .as_ref()
            .unwrap()
            .sent_commands()
            .iter()
            .any(|command| matches!(
                command,
                AudioCommand::Router(RouterCommand::RouteSource { .. })
            ))
    );
    change(&mut app, MixerMessage::Undo);
    assert!(matches!(
        app.audio.as_ref().unwrap().sent_commands().last(),
        Some(AudioCommand::Router(RouterCommand::UndoRoutingChange))
    ));
}

#[test]
fn microphone_feedback_updates_control_and_blocks_cooldown_reenable() {
    let mut app = app();
    let _ = app.update(Message::AudioEvent(AudioEvent::MicrophoneFeedbackMuted));
    assert!(!app.config.mic_passthrough);
    let _ = app.update(Message::MicPassthroughChanged(true));
    assert!(!app.config.mic_passthrough);
    assert!(
        !app.audio
            .as_ref()
            .unwrap()
            .sent_commands()
            .iter()
            .any(|command| matches!(command, AudioCommand::SetMicPassthrough(true)))
    );
}

#[test]
fn feedback_banner_expires_at_timeout_without_clearing_row_warning() {
    let mut app = app();
    let now = Instant::now();
    app.mixer.feedback(
        Some(&crate::audio::FeedbackSource {
            node_id: 7,
            name: "Browser".into(),
        }),
        now,
    );
    app.mixer.tick(now + Duration::from_millis(1999));
    assert_eq!(app.mixer.feedback_source.as_deref(), Some("Browser"));
    assert!(app.mixer.feedback_until.is_some());
    app.mixer.tick(now + Duration::from_secs(2));
    assert!(app.mixer.feedback_source.is_none());
    assert!(app.mixer.feedback_until.is_none());
    assert!(!app.mixer.needs_tick());
    assert_eq!(
        app.mixer.sources[&7].warning,
        Some(crate::audio::RouteRejection::Cooldown)
    );
    assert!(!app.mixer.sources[&7].enabled);
}

fn port(id: u32, node_id: u32, monitor: bool) -> StreamEvent {
    StreamEvent::PortAdded {
        id,
        node_id,
        channel: "FL".into(),
        direction: crate::audio::Direction::Output,
        monitor,
    }
}

#[test]
fn last_monitor_port_removal_restores_routing_without_affecting_other_sources() {
    let mut app = app();
    let now = Instant::now();
    app.mixer.stream(added(9), now);
    for event in [
        port(70, 7, true),
        port(71, 7, true),
        port(72, 7, false),
        port(90, 9, true),
    ] {
        app.mixer.stream(event, now);
    }
    change(&mut app, MixerMessage::SafeMode(false));
    app.mixer.stream(StreamEvent::PortRemoved { id: 72 }, now);
    app.mixer.stream(StreamEvent::PortRemoved { id: 70 }, now);
    change(&mut app, MixerMessage::Route(7));
    assert!(app.mixer.sources[&7].monitor);
    assert!(app.mixer.confirm.is_none());
    app.mixer.stream(StreamEvent::PortRemoved { id: 71 }, now);
    app.mixer.stream(StreamEvent::PortRemoved { id: 71 }, now);
    assert!(!app.mixer.sources[&7].monitor);
    assert!(app.mixer.sources[&9].monitor);
    change(&mut app, MixerMessage::Route(7));
    assert_eq!(app.mixer.confirm, Some(7));
}

#[test]
fn monitor_port_removed_before_source_metadata_does_not_mark_source() {
    let mut app = HonkHonk::new_for_test();
    let now = Instant::now();
    app.mixer.stream(port(70, 7, true), now);
    app.mixer.stream(StreamEvent::PortRemoved { id: 70 }, now);
    app.mixer.stream(added(7), now);
    assert!(!app.mixer.sources[&7].monitor);
}

#[test]
fn source_removal_cleans_all_monitor_port_mappings_before_node_id_reuse() {
    let mut app = app();
    let now = Instant::now();
    app.mixer.stream(port(70, 7, true), now);
    app.mixer.stream(port(71, 7, true), now);
    app.mixer.stream(StreamEvent::SourceRemoved { id: 7 }, now);
    app.mixer.stream(added(7), now);
    assert!(!app.mixer.sources[&7].monitor);
    app.mixer.stream(port(72, 7, true), now);
    app.mixer.stream(StreamEvent::PortRemoved { id: 72 }, now);
    assert!(!app.mixer.sources[&7].monitor);
    app.mixer.stream(StreamEvent::PortRemoved { id: 70 }, now);
    assert!(!app.mixer.sources[&7].monitor);
}
