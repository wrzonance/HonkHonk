//! Audio queue draining, playback teardown and config persistence.

use super::*;

impl HonkHonk {
    pub(super) fn stop_all(&mut self) -> Task<Message> {
        self.import.preview = self.import.preview.wrapping_add(1);
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::Stop);
        }
        // `clear_playback_state` sets `playing = None`; `handle_decoded`
        // gates on `playing == Some(id)`, so any decode still in flight
        // for the stopped sound is dropped on arrival rather than
        // resurrecting it (#151).
        self.pending_play_ids.clear();
        self.pending_decodes.clear();
        self.clear_playback_state();
        self.cancel_macro();
        Task::none()
    }

    /// Process every audio event queued since the last poll tick.
    ///
    /// The engine emits ~10 Progress events/sec while playing plus two events
    /// per Play (Finished for the replaced sound + Started), while this poll
    /// runs at 10 Hz. Draining one event per tick (the old behavior) therefore
    /// could never catch up after a burst of button presses, leaving the UI
    /// seconds behind the audio (#111).
    pub(super) fn drain_audio_events(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        loop {
            let event = match self.audio {
                Some(ref audio) => audio.try_recv(),
                None => None,
            };
            let Some(event) = event else { break };
            tasks.push(self.update(Message::AudioEvent(event)));
        }
        Task::batch(tasks)
    }

    /// Clears all now-playing state together so the highlight, raw progress, and
    /// delegated playback UI state never drift apart.
    /// The single teardown path for StopAll, the genuine PlaybackFinished end,
    /// and a failed decode.
    pub(super) fn clear_playback_state(&mut self) {
        self.playing = None;
        self.progress = 0.0;
        self.now_playing.clear();
    }

    /// Persists the live config unless persistence is disabled (test fixtures
    /// set `persist = false` so `cargo test` never writes the real config file).
    pub(super) fn persist_config(&self) {
        if self.persist
            && let Err(e) = self.config.save()
        {
            tracing::warn!(error = %e, "config save error");
        }
    }
}
