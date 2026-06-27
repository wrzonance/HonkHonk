//! Play-dispatch coordination extracted from `app/mod.rs` to keep the Iced
//! Application thin (CLAUDE.md: `app.rs` delegates to module APIs and must not
//! grow). Owns the click -> (warm PCM cache hit | async decode) -> fire
//! pipeline and the `Message::Decoded` landing, as methods on `HonkHonk` so
//! they share its state directly. See spec #151.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct PlaybackDispatch {
    pub(super) generation: u64,
    pub(super) voice_id: u64,
    pub(super) gain: f32,
    pub(super) effects: crate::audio::effects::EffectSettings,
    pub(super) mode: PlayMode,
}

#[derive(Clone, Copy)]
pub(super) struct PendingDecode {
    pub(super) task_voice_id: u64,
    pub(super) dispatch: PlaybackDispatch,
}

impl HonkHonk {
    /// Begins playing `sound`. A warm PCM cache hit fires synchronously and
    /// claims the UI immediately; a cold miss queues off-thread decode work
    /// without taking the highlight until decode succeeds. Same-id cold repeats
    /// update the pending dispatch instead of spawning duplicate decodes (#152).
    pub(super) fn request_play(
        &mut self,
        sound: &SoundEntry,
        force_interrupt: bool,
    ) -> Task<Message> {
        self.capture_recording_at(&sound.path, Instant::now());
        self.play_generation = self.play_generation.wrapping_add(1);
        let id = sound.id.clone();
        let dispatch = PlaybackDispatch {
            generation: self.play_generation,
            voice_id: self.play_generation,
            gain: self.sound_meta.volume_for(&sound.id),
            effects: self.effects_ui.to_effect_settings(),
            mode: self.play_mode_for_request(force_interrupt),
        };
        if dispatch.mode == PlayMode::Interrupt {
            self.pending_play_ids.clear();
            self.pending_decodes.clear();
            if let Some(ref audio) = self.audio {
                audio.send(AudioCommand::Stop);
            }
            self.clear_playback_state();
        }
        if let Some(pcm) = self.audio_store.get_pcm(&sound.id) {
            self.start_playback(&sound.id, pcm, dispatch);
            return Task::none();
        }
        if let Some(pending) = self.pending_decodes.get_mut(&id) {
            pending.dispatch = dispatch;
            return Task::none();
        }
        self.queue_decode(id, sound.path.clone(), dispatch)
    }

    fn queue_decode(
        &mut self,
        id: String,
        path: std::path::PathBuf,
        dispatch: PlaybackDispatch,
    ) -> Task<Message> {
        self.pending_play_ids.insert(dispatch.voice_id);
        self.pending_decodes.insert(
            id.clone(),
            PendingDecode {
                task_voice_id: dispatch.voice_id,
                dispatch,
            },
        );
        Self::decode_task(id, path, dispatch)
    }

    fn decode_task(
        id: String,
        path: std::path::PathBuf,
        dispatch: PlaybackDispatch,
    ) -> Task<Message> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || crate::audio::decode(&path))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.map_err(|e| e.to_string()))
                    .map(|d| crate::audio::CachedPcm {
                        samples: std::sync::Arc::new(d.samples),
                        sample_rate: d.sample_rate,
                        channels: d.channels,
                        duration: d.duration,
                    })
            },
            move |result| Message::Decoded {
                generation: dispatch.generation,
                voice_id: dispatch.voice_id,
                id: id.clone(),
                result,
                gain: dispatch.gain,
                effects: dispatch.effects,
                mode: dispatch.mode,
            },
        )
    }

    /// Applies a completed background decode. Results are accepted only while
    /// their task is still pending and the sound still exists in the library.
    /// An `Ok` caches PCM and starts playback; an `Err` logs and clears only if
    /// that sound still owns the current UI (#151/#152).
    #[allow(
        clippy::cognitive_complexity,
        reason = "decode landing owns stale-generation, cache, playback, and UI cleanup invariants"
    )]
    pub(super) fn handle_decoded(
        &mut self,
        id: String,
        result: Result<crate::audio::CachedPcm, String>,
        dispatch: PlaybackDispatch,
    ) -> Task<Message> {
        let Some(pending) = self.pending_decode_for(&id, dispatch) else {
            return Task::none();
        };
        if !self.sound_exists(&id) {
            return Task::none();
        }
        let dispatch = pending.dispatch;
        match result {
            Ok(pcm) => {
                let pcm = std::sync::Arc::new(pcm);
                let evicted = self
                    .audio_store
                    .insert_pcm(id.clone(), std::sync::Arc::clone(&pcm));
                self.evict_waveform_envelopes(evicted);
                self.start_playback(&id, pcm, dispatch);
            }
            Err(e) => {
                let file = self
                    .sounds
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.path.display().to_string())
                    .unwrap_or_else(|| id.clone()); // fall back to id if rescanned away
                tracing::error!(file = %file, error = %e, "decode failed");
                if dispatch.generation == self.play_generation
                    && self.playing.as_deref() == Some(&id)
                {
                    self.clear_playback_state();
                }
            }
        }
        Task::none()
    }

    fn pending_decode_for(
        &mut self,
        id: &str,
        dispatch: PlaybackDispatch,
    ) -> Option<PendingDecode> {
        let pending = self.pending_decodes.remove(id).unwrap_or(PendingDecode {
            task_voice_id: dispatch.voice_id,
            dispatch,
        });
        self.pending_play_ids
            .remove(&pending.task_voice_id)
            .then_some(pending)
    }

    pub(super) fn reconcile_playback_with_library(&mut self) {
        if self
            .playing
            .as_deref()
            .is_some_and(|id| !self.sound_exists(id))
        {
            self.clear_playback_state();
        }
        let removed: Vec<String> = self
            .pending_decodes
            .keys()
            .filter(|id| !self.sound_exists(id))
            .cloned()
            .collect();
        for id in removed {
            if let Some(pending) = self.pending_decodes.remove(&id) {
                self.pending_play_ids.remove(&pending.task_voice_id);
            }
        }
    }

    fn sound_exists(&self, id: &str) -> bool {
        self.sounds.iter().any(|sound| sound.id == id)
    }

    fn evict_waveform_envelopes(&mut self, ids: Vec<String>) {
        for id in ids {
            self.now_playing.remove_envelope(&id);
        }
    }

    /// Starts the playhead from the decoded duration, ensures the waveform
    /// envelope is cached, and dispatches the engine `Play`. Shared by the warm
    /// cache-hit path and `handle_decoded`.
    ///
    /// In concurrent mode superseded presses stay pending (`pending_play_ids` is
    /// only cleared on Interrupt), so an older press that finishes decoding last
    /// still lands here. Its audio must start, but only the current generation
    /// may own the highlight/playhead — otherwise a late out-of-order decode
    /// retakes the now-playing UI from the newer press.
    fn start_playback(
        &mut self,
        id: &str,
        pcm: std::sync::Arc<crate::audio::CachedPcm>,
        dispatch: PlaybackDispatch,
    ) {
        let owns_ui = dispatch.generation == self.play_generation;
        if owns_ui {
            self.now_playing.start(now_playing::PlaybackStart {
                id,
                duration: pcm.duration,
                samples: pcm.samples.as_ref().as_slice(),
                channels: pcm.channels,
                now: Instant::now(),
            });
        }
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::Play {
                voice_id: dispatch.voice_id,
                sound_id: id.to_string(),
                samples: std::sync::Arc::clone(&pcm.samples),
                sample_rate: pcm.sample_rate,
                channels: pcm.channels,
                generation: dispatch.generation,
                gain: dispatch.gain,
                effects: dispatch.effects,
                mode: dispatch.mode,
            });
            if owns_ui {
                self.playing = Some(id.to_string());
            }
        }
    }

    fn play_mode_for_request(&self, force_interrupt: bool) -> PlayMode {
        if force_interrupt {
            return PlayMode::Interrupt;
        }
        match self.config.overlap_mode {
            OverlapMode::Concurrent => PlayMode::Concurrent,
            OverlapMode::Interrupt => PlayMode::Interrupt,
        }
    }
}

#[cfg(test)]
mod tests;
