use super::*;
use crate::audio::RouterCommand;
use crate::audio::streams::StreamEvent;
use crate::ui::mixer::MixerMessage;

fn app() -> HonkHonk {
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

fn change(app: &mut HonkHonk, message: MixerMessage) {
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

fn identified(id: u32, pid: u32) -> StreamEvent {
    StreamEvent::SourceAdded {
        id,
        name: "Browser".into(),
        app_name: Some("Browser".into()),
        app_binary: Some("browser".into()),
        app_pid: Some(pid),
        icon: None,
        media_name: None,
    }
}

#[test]
fn source_levels_default_and_restore_across_process_and_node_restarts() {
    let mut app = app();
    let _ = app.handle_audio_event(AudioEvent::Stream(identified(8, 100)));
    assert_eq!(app.mixer.sources[&8].level.volume, 1.0);
    app.config.mixer_safe_mode = false;
    let _ = app.handle_audio_event(AudioEvent::RoutingChanged {
        node_id: 8,
        enabled: true,
    });
    change(&mut app, MixerMessage::Volume(8, 0.4));
    change(&mut app, MixerMessage::Mute(8, true));
    assert_eq!(app.mixer.sources[&8].level.volume, 0.4);
    assert!(app.mixer.sources[&8].level.muted);
    let saved = serde_json::to_string(&app.config).unwrap();
    let mut restored = HonkHonk::new_for_test();
    restored.config = serde_json::from_str(&saved).unwrap();
    restored.audio = Some(crate::audio::test_handle().0);
    let _ = restored.handle_audio_event(AudioEvent::Stream(identified(19, 200)));
    let _ = restored.handle_audio_event(AudioEvent::RoutingChanged {
        node_id: 19,
        enabled: true,
    });
    assert!(
        matches!(restored.audio.as_ref().unwrap().sent_commands().last(),
        Some(AudioCommand::Router(RouterCommand::SetSourceLevel { source_node_id: 19, level }))
        if level.volume == 0.4 && level.muted)
    );
}

#[test]
fn level_controls_ignore_unrouted_unknown_and_monitor_sources() {
    let mut app = app();
    for id in [7, 999] {
        change(&mut app, MixerMessage::Volume(id, 0.2));
        change(&mut app, MixerMessage::Mute(id, true));
    }
    app.config.mixer_safe_mode = false;
    app.mixer.sources.get_mut(&7).unwrap().enabled = true;
    app.mixer.sources.get_mut(&7).unwrap().monitor = true;
    change(&mut app, MixerMessage::Mute(7, true));
    assert!(app.audio.as_ref().unwrap().sent_commands().is_empty());
    assert!(app.config.source_levels.is_empty());
}

#[test]
fn server_levels_update_display_without_destroying_saved_preference() {
    let mut app = app();
    let _ = app.handle_audio_event(AudioEvent::Stream(identified(8, 1)));
    app.config.mixer_safe_mode = false;
    app.mixer.sources.get_mut(&8).unwrap().enabled = true;
    change(&mut app, MixerMessage::Volume(8, 0.6));
    let saved = app.config.source_levels.clone();
    let _ = app.handle_audio_event(AudioEvent::Stream(StreamEvent::SourceLevelChanged {
        id: 8,
        volume: Some(0.3),
        muted: Some(true),
    }));
    assert_eq!(app.mixer.sources[&8].level.volume, 0.3);
    assert!(app.mixer.sources[&8].level.muted);
    assert_eq!(app.config.source_levels, saved);
}
