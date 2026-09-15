//! Window actions and their state transitions.

use super::*;

impl HonkHonk {
    pub(super) fn toggle_visibility(&mut self) -> Task<Message> {
        self.visible = !self.visible;
        if !self.visible {
            self.panel_flourish.clear();
        }
        Task::none()
    }

    pub(super) fn quit(&mut self) -> Task<Message> {
        if let Some(ref audio) = self.audio {
            audio.shutdown();
        }
        // Persist the latest window size (recorded in-memory on
        // resize) and any other config on a real quit.
        if self.should_persist_config_on_quit() {
            self.persist_config();
        }
        self.exit = true;
        iced::exit()
    }

    pub(super) fn poll_tray(&mut self) -> Task<Message> {
        let event = self.tray_rx.lock().ok().and_then(|rx| rx.try_recv().ok());
        if let Some(e) = event {
            let msg = Message::from_tray_event(e);
            return self.update(msg);
        }

        self.drain_audio_events()
    }

    pub(super) fn resize_window(&mut self, w: f32, h: f32) -> Task<Message> {
        self.window_size = (w, h);
        // Record into config (in-memory only — no disk write per resize
        // event); persisted on quit and by any other settings save.
        // Degenerate events must not clobber the last real size; NaN
        // also fails these comparisons and is skipped.
        if w >= MIN_WINDOW_DIMENSION && h >= MIN_WINDOW_DIMENSION {
            self.config.window_width = w.round() as u32;
            self.config.window_height = h.round() as u32;
        }
        Task::none()
    }
}
