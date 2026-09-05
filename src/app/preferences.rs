//! Preferences and their state transitions.

use super::*;

impl HonkHonk {
    pub(super) fn change_theme(&mut self, t: theme::Theme) -> Task<Message> {
        if self.config.theme != t {
            self.config = AppConfig {
                theme: t,
                ..self.config.clone()
            };
            self.persist_config();
        }
        Task::none()
    }

    pub(super) fn change_density(&mut self, d: crate::state::config::Density) -> Task<Message> {
        if self.config.density != d {
            self.config = AppConfig {
                density: d,
                ..self.config.clone()
            };
            self.persist_config();
        }
        Task::none()
    }

    pub(super) fn change_renderer(&mut self, r: crate::state::Renderer) -> Task<Message> {
        if self.config.renderer != r {
            self.config = AppConfig {
                renderer: r,
                ..self.config.clone()
            };
            self.persist_config();
        }
        Task::none()
    }

    pub(super) fn change_mic_passthrough(&mut self, v: bool) -> Task<Message> {
        let config = AppConfig {
            mic_passthrough: v,
            ..self.config.clone()
        };
        self.config = config;
        self.persist_config();
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::SetMicPassthrough(v));
        }
        Task::none()
    }

    pub(super) fn change_mic_passthrough_level(&mut self, v: f32) -> Task<Message> {
        let config = AppConfig {
            mic_passthrough_level: v.clamp(0.0, 1.0),
            ..self.config.clone()
        };
        self.config = config;
        self.persist_config();
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::SetMicPassthroughLevel(
                self.config.mic_passthrough_level,
            ));
        }
        Task::none()
    }

    pub(super) fn change_overlap_mode(&mut self, overlap_mode: OverlapMode) -> Task<Message> {
        if self.config.overlap_mode != overlap_mode {
            let config = AppConfig {
                overlap_mode,
                ..self.config.clone()
            };
            self.config = config;
            self.persist_config();
        }
        Task::none()
    }

    pub(super) fn change_monitor_device(&mut self, target: Option<String>) -> Task<Message> {
        if self.config.monitor_device == target {
            return Task::none();
        }
        let config = AppConfig {
            monitor_device: target.clone(),
            ..self.config.clone()
        };
        self.config = config;
        self.persist_config();
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::SetMonitorDevice(target));
        }
        Task::none()
    }

    pub(super) fn change_input_device(&mut self, target: Option<String>) -> Task<Message> {
        if self.config.input_device == target {
            return Task::none();
        }
        let config = AppConfig {
            input_device: target.clone(),
            ..self.config.clone()
        };
        self.config = config;
        self.persist_config();
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::SetInputDevice(target));
        }
        Task::none()
    }
}
