use super::*;

impl HonkHonk {
    pub(in crate::app) fn mixer_stream(&mut self, event: StreamEvent) {
        let added = match &event {
            StreamEvent::SourceAdded { id, .. } => Some(*id),
            _ => None,
        };
        let first_observation = match &event {
            StreamEvent::SourceLevelChanged { id, .. } => self
                .mixer
                .sources
                .get(id)
                .filter(|source| !source.level_observed)
                .map(|_| *id),
            _ => None,
        };
        self.mixer.stream(event, Instant::now());
        if let Some(id) = added
            && let Some(level) = self.saved_source_level(id)
            && let Some(source) = self.mixer.sources.get_mut(&id)
        {
            source.level = level;
        }
        if let Some(id) = first_observation {
            self.restore_source_level(id);
        }
    }

    fn saved_source_level(&self, id: u32) -> Option<crate::audio::streams::SourceLevel> {
        self.mixer
            .sources
            .get(&id)
            .and_then(|s| s.preference_key.as_ref())
            .and_then(|key| self.config.source_levels.get(key))
            .copied()
            .map(crate::audio::streams::SourceLevel::normalized)
    }

    pub(in crate::app) fn restore_source_level(&mut self, id: u32) {
        let Some(level) = self.saved_source_level(id) else {
            return;
        };
        if let Some(source) = self.mixer.sources.get_mut(&id) {
            if !source.enabled
                || !source.level_observed
                || source.blocked(Instant::now())
                || self.config.mixer_safe_mode
            {
                return;
            }
            source.level = level;
            self.send_router(RouterCommand::SetSourceLevel {
                source_node_id: id,
                level,
            });
        }
    }

    pub(super) fn set_source_level(&mut self, id: u32, volume: Option<f32>, muted: Option<bool>) {
        let Some(source) = self.mixer.sources.get_mut(&id) else {
            return;
        };
        if !source.enabled
            || !source.level_observed
            || source.blocked(Instant::now())
            || self.config.mixer_safe_mode
            || volume.is_some_and(|v| !v.is_finite())
        {
            return;
        }
        if let Some(v) = volume {
            source.level.volume = v.clamp(0.0, 1.0);
        }
        if let Some(v) = muted {
            source.level.muted = v;
        }
        let level = source.level;
        if let Some(key) = &source.preference_key {
            self.config.source_levels.insert(key.clone(), level);
            if muted.is_some() {
                self.persist_config();
            }
        }
        self.send_router(RouterCommand::SetSourceLevel {
            source_node_id: id,
            level,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::mixer_tests::{app, change};
    use crate::audio::AudioEvent;

    #[test]
    fn slider_release_saves_preferences_once() {
        let mut app = app();
        let saves = crate::app::lifecycle::CONFIG_SAVES.with(|count| count.get());
        change(&mut app, MixerMessage::VolumeSave);
        assert_eq!(
            crate::app::lifecycle::CONFIG_SAVES.with(|c| c.get()),
            saves + 1
        );
    }

    #[test]
    fn dragging_volume_updates_audio_and_preferences_without_saving() {
        let mut app = app();
        app.config.mixer_safe_mode = false;
        let _ = app.handle_audio_event(AudioEvent::Stream(identified(8, 1)));
        observe_level(&mut app, 8);
        app.mixer.sources.get_mut(&8).unwrap().enabled = true;
        let saves = crate::app::lifecycle::CONFIG_SAVES.with(|count| count.get());
        for volume in [0.7, 0.6, 0.5] {
            change(&mut app, MixerMessage::Volume(8, volume));
        }
        let key = app.mixer.sources[&8].preference_key.as_ref().unwrap();
        assert_eq!(app.config.source_levels[key].volume, 0.5);
        let commands = app.audio.as_ref().unwrap().sent_commands();
        assert_eq!(commands.len(), 3);
        assert!(matches!(commands.last(), Some(AudioCommand::Router(
            RouterCommand::SetSourceLevel { level, .. })) if level.volume == 0.5));
        assert_eq!(crate::app::lifecycle::CONFIG_SAVES.with(|c| c.get()), saves);
        change(&mut app, MixerMessage::Mute(8, true));
        assert_eq!(
            crate::app::lifecycle::CONFIG_SAVES.with(|c| c.get()),
            saves + 1
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
        observe_level(&mut app, 8);
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
        observe_level(&mut restored, 19);
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
        observe_level(&mut app, 8);
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

    #[test]
    fn first_route_preserves_observed_levels_without_a_saved_preference() {
        let mut app = app();
        app.config.mixer_safe_mode = false;
        let _ = app.handle_audio_event(AudioEvent::Stream(StreamEvent::SourceLevelChanged {
            id: 7,
            volume: Some(0.25),
            muted: Some(true),
        }));
        let _ = app.handle_audio_event(AudioEvent::RoutingChanged {
            node_id: 7,
            enabled: true,
        });
        assert_eq!(app.mixer.sources[&7].level.volume, 0.25);
        assert!(app.mixer.sources[&7].level.muted);
        assert!(app.audio.as_ref().unwrap().sent_commands().is_empty());
    }

    #[test]
    fn saved_level_waits_for_initial_props_and_applies_only_once() {
        let mut app = app();
        app.config.mixer_safe_mode = false;
        let _ = app.handle_audio_event(AudioEvent::Stream(identified(8, 100)));
        let key = app.mixer.sources[&8].preference_key.clone().unwrap();
        app.config.source_levels.insert(
            key,
            crate::audio::streams::SourceLevel {
                volume: 0.6,
                muted: true,
            },
        );
        let _ = app.handle_audio_event(AudioEvent::RoutingChanged {
            node_id: 8,
            enabled: true,
        });
        assert!(app.audio.as_ref().unwrap().sent_commands().is_empty());
        for volume in [0.3, 0.6] {
            let _ = app.handle_audio_event(AudioEvent::Stream(StreamEvent::SourceLevelChanged {
                id: 8,
                volume: Some(volume),
                muted: Some(false),
            }));
        }
        let commands = app.audio.as_ref().unwrap().sent_commands();
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(commands.last(), Some(AudioCommand::Router(RouterCommand::SetSourceLevel { source_node_id: 8, level }))
        if level.volume == 0.6 && level.muted)
        );
    }

    fn observe_level(app: &mut HonkHonk, id: u32) {
        let _ = app.handle_audio_event(AudioEvent::Stream(StreamEvent::SourceLevelChanged {
            id,
            volume: Some(1.0),
            muted: Some(false),
        }));
    }
}
