//! Effects-panel animation glue extracted from `app/mod.rs`.
//!
//! `ui::side_panel` owns the pure animation model and rendering. This module is
//! only the app-state bridge: when to emit, tick, save the user toggle, and
//! subscribe to frames.

use super::*;
use crate::ui::side_panel::{PanelTransition, panel_geometry};

impl HonkHonk {
    pub(super) fn tick_frame(&mut self, now: Instant) {
        self.now_playing.tick(now);
        self.panel_progress = self.effects_panel.tick(now);
        if self.visible && self.config.panel_animations {
            self.panel_flourish.tick(now, Some(self.cursor_pos));
        } else {
            self.panel_flourish.clear();
        }
    }

    pub(super) fn set_panel_animations(&mut self, enabled: bool) -> Task<Message> {
        if self.config.panel_animations != enabled {
            self.config = AppConfig {
                panel_animations: enabled,
                ..self.config.clone()
            };
            if let Err(e) = self.config.save() {
                tracing::warn!(error = %e, "config save error");
            }
        }
        if !enabled {
            self.panel_flourish.clear();
        }
        Task::none()
    }

    pub(super) fn toggle_effects_panel(&mut self) -> Task<Message> {
        let now = Instant::now();
        self.effects_panel.toggle(now);
        let transition = if self.effects_panel.is_open() {
            PanelTransition::Open
        } else {
            PanelTransition::Close
        };
        self.emit_effects_panel_flourish(transition, now);
        self.panel_progress = self.effects_panel.progress(now);
        Task::none()
    }

    pub(super) fn close_effects_panel(&mut self) -> Task<Message> {
        let now = Instant::now();
        let was_visible = self.effects_panel.is_visible();
        self.effects_panel.close(now);
        if was_visible {
            self.emit_effects_panel_flourish(PanelTransition::Close, now);
        }
        self.panel_progress = self.effects_panel.progress(now);
        Task::none()
    }

    pub(super) fn close_effects_panel_from_escape(&mut self, now: Instant) {
        let should_emit = self.effects_panel.is_open();
        self.effects_panel.close(now);
        if should_emit {
            self.emit_effects_panel_flourish(PanelTransition::Close, now);
        }
        self.panel_progress = self.effects_panel.progress(now);
    }

    pub(super) fn frame_subscription_needed(&self) -> bool {
        self.visible
            && (self.playing.is_some()
                || self.effects_panel.is_animating()
                || self.panel_flourish.is_animating())
    }

    fn emit_effects_panel_flourish(&mut self, transition: PanelTransition, now: Instant) {
        if !self.visible || !self.config.panel_animations {
            return;
        }
        let panel = panel_geometry(self.window_size, effects_panel_view::EFFECTS_PANEL_W);
        self.panel_flourish
            .emit(panel, self.window_size, transition, now);
    }
}
