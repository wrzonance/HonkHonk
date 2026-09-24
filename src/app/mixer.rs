//! Mixer state owns presentation; the router remains the authority for safety.
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use super::{HonkHonk, Message};
use crate::audio::{AudioCommand, FeedbackSource, RouteRejection, RouterCommand, StreamEvent};
use crate::ui::mixer::MixerMessage;

#[derive(Default)]
pub(crate) struct MixerState {
    pub sources: BTreeMap<u32, MixerSource>,
    pub confirm: Option<u32>,
    pending_titles: BTreeMap<u32, (Instant, String)>,
    monitor_nodes: BTreeSet<u32>,
    pub mic_cooldown: Option<Instant>,
    pub now: Option<Instant>,
    pub feedback_source: Option<String>,
    pub feedback_until: Option<Instant>,
}

pub(crate) struct MixerSource {
    pub name: String,
    pub media_name: Option<String>,
    pub enabled: bool,
    pub monitor: bool,
    pub warning: Option<RouteRejection>,
    pub cooldown: Option<Instant>,
}

impl MixerSource {
    pub fn blocked(&self, now: Instant) -> bool {
        self.monitor || self.cooldown.is_some_and(|until| until > now)
    }
}

impl MixerState {
    pub fn stream(&mut self, event: StreamEvent, now: Instant) {
        match event {
            StreamEvent::SourceAdded {
                id,
                name,
                media_name,
                ..
            } => self.source_added(id, name, media_name),
            StreamEvent::SourceRemoved { id } => {
                self.sources.remove(&id);
                self.pending_titles.remove(&id);
                self.monitor_nodes.remove(&id);
                if self.confirm == Some(id) {
                    self.confirm = None;
                }
            }
            StreamEvent::SourceUpdated {
                id,
                media_name: Some(title),
            } => {
                if self.sources.contains_key(&id) {
                    self.pending_titles
                        .entry(id)
                        .and_modify(|(_, current)| current.clone_from(&title))
                        .or_insert((now + Duration::from_millis(200), title));
                }
            }
            StreamEvent::PortAdded {
                node_id,
                monitor: true,
                ..
            } => {
                self.monitor_nodes.insert(node_id);
                if let Some(source) = self.sources.get_mut(&node_id) {
                    source.monitor = true;
                }
                if self.confirm == Some(node_id) {
                    self.confirm = None;
                }
            }
            _ => {}
        }
    }

    fn source_added(&mut self, id: u32, name: String, media_name: Option<String>) {
        self.sources.insert(
            id,
            MixerSource {
                name,
                media_name,
                enabled: false,
                monitor: self.monitor_nodes.contains(&id),
                warning: None,
                cooldown: None,
            },
        );
    }

    pub fn tick(&mut self, now: Instant) {
        self.now = Some(now);
        if self.mic_cooldown.is_some_and(|until| until <= now) {
            self.mic_cooldown = None;
        }
        self.pending_titles.retain(|id, (until, title)| {
            if *until > now {
                return true;
            }
            if let Some(source) = self.sources.get_mut(id) {
                source.media_name = Some(title.clone());
            }
            false
        });
        for source in self.sources.values_mut() {
            if source.cooldown.is_some_and(|until| until <= now) {
                source.cooldown = None;
            }
        }
        if self.feedback_until.is_some_and(|until| until <= now) {
            self.feedback_until = None;
        }
    }

    pub fn needs_tick(&self) -> bool {
        !self.pending_titles.is_empty()
            || self.feedback_until.is_some()
            || self.mic_cooldown.is_some()
            || self
                .sources
                .values()
                .any(|source| source.cooldown.is_some())
    }

    pub fn routed(&mut self, id: u32, enabled: bool) {
        if let Some(source) = self.sources.get_mut(&id) {
            source.enabled = enabled;
            if enabled {
                source.warning = None;
            }
        }
    }

    pub fn rejected(&mut self, id: u32, reason: RouteRejection, now: Instant) {
        if let Some(source) = self.sources.get_mut(&id) {
            source.warning = Some(reason);
            if reason == RouteRejection::Cooldown {
                source.cooldown = Some(now + Duration::from_secs(2));
            }
        }
    }

    pub fn feedback(&mut self, suspect: Option<&FeedbackSource>, now: Instant) {
        self.confirm = None;
        self.now = Some(now);
        self.feedback_until = Some(now + Duration::from_secs(2));
        self.feedback_source = Some(suspect.map_or("Unidentified source", |s| &s.name).into());
        if let Some(source) = suspect.and_then(|s| self.sources.get_mut(&s.node_id)) {
            source.enabled = false;
            source.cooldown = self.feedback_until;
            source.warning = Some(RouteRejection::Cooldown);
        }
    }
}

impl HonkHonk {
    pub(super) fn update_mixer(&mut self, message: MixerMessage) -> iced::Task<Message> {
        match message {
            MixerMessage::Tick(now) => self.mixer.tick(now),
            MixerMessage::SafeMode(enabled) => {
                self.config.mixer_safe_mode = enabled;
                self.mixer.confirm = None;
                self.persist_config();
                self.send_router(RouterCommand::SetSafeMode(enabled));
            }
            MixerMessage::ShowMonitors(show) => {
                self.config.mixer_show_monitors = show;
                self.persist_config();
            }
            MixerMessage::Route(id) => self.request_mixer_route(id),
            MixerMessage::Cancel => self.mixer.confirm = None,
            MixerMessage::Confirm => {
                if let Some(id) = self.mixer.confirm.take()
                    && self.can_route(id)
                {
                    self.send_router(RouterCommand::RouteSource { source_node_id: id });
                }
            }
            MixerMessage::Undo => {
                self.mixer.confirm = None;
                self.send_router(RouterCommand::UndoRoutingChange);
            }
        }
        iced::Task::none()
    }

    fn can_route(&self, id: u32) -> bool {
        !self.config.mixer_safe_mode
            && self
                .mixer
                .sources
                .get(&id)
                .is_some_and(|source| !source.blocked(Instant::now()) && !source.enabled)
    }

    fn request_mixer_route(&mut self, id: u32) {
        if self.mixer.sources.get(&id).is_some_and(|s| s.enabled) {
            self.send_router(RouterCommand::UnrouteSource { source_node_id: id });
        } else if self.can_route(id) {
            self.mixer.confirm = Some(id);
        }
    }

    fn send_router(&self, command: RouterCommand) {
        self.send_audio_commands([AudioCommand::Router(command)]);
    }
}
