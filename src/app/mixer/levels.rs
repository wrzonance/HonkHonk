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
            self.persist_config();
        }
        self.send_router(RouterCommand::SetSourceLevel {
            source_node_id: id,
            level,
        });
    }
}
