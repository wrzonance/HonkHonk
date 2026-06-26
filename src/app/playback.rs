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

impl HonkHonk {
    /// Begins playing `sound`. A warm PCM cache hit fires synchronously; a miss
    /// returns a `Task` that decodes off the UI thread and yields
    /// `Message::Decoded`. The play generation is bumped here so a stale decode
    /// (superseded by a newer press) is dropped on arrival (#149/#151).
    pub(super) fn request_play(
        &mut self,
        sound: &SoundEntry,
        force_interrupt: bool,
    ) -> Task<Message> {
        self.capture_recording_at(&sound.path, Instant::now());
        self.play_generation = self.play_generation.wrapping_add(1);
        let dispatch = PlaybackDispatch {
            generation: self.play_generation,
            voice_id: self.play_generation,
            gain: self.sound_meta.volume_for(&sound.id),
            effects: self.effects_ui.to_effect_settings(),
            mode: self.play_mode_for_request(force_interrupt),
        };
        self.playing = Some(sound.id.clone());
        self.now_playing.pending(&sound.id);
        if dispatch.mode == PlayMode::Interrupt {
            self.pending_play_ids.clear();
            if let Some(ref audio) = self.audio {
                audio.send(AudioCommand::Stop);
            }
        }
        if let Some(pcm) = self.audio_store.get_pcm(&sound.id) {
            self.start_playback(&sound.id, pcm, dispatch);
            return Task::none();
        }
        self.pending_play_ids.insert(dispatch.voice_id);
        let id = sound.id.clone();
        let path = sound.path.clone();
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

    /// Applies a completed background decode for `generation`. A stale
    /// generation (a superseded press, or a StopAll mid-decode) is dropped; an
    /// `Ok` caches the PCM and starts playback; an `Err` logs and tears down the
    /// optimistic state so it does not stick (#151). The `Decoded` arm in
    /// `update` delegates straight here.
    pub(super) fn handle_decoded(
        &mut self,
        id: String,
        result: Result<crate::audio::CachedPcm, String>,
        dispatch: PlaybackDispatch,
    ) -> Task<Message> {
        if !self.pending_play_ids.remove(&dispatch.voice_id) {
            return Task::none();
        }
        match result {
            Ok(pcm) => {
                let pcm = std::sync::Arc::new(pcm);
                self.audio_store
                    .insert_pcm(id.clone(), std::sync::Arc::clone(&pcm));
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
mod tests {
    use super::*;

    #[test]
    fn cold_play_updates_waveform_cache_key_before_decode_lands() {
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);
        app.now_playing.sync(Some("old"), 0.75);

        let sound = SoundEntry {
            id: "new".into(),
            name: "New".into(),
            path: "/tmp/new.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: Some(100),
            category: "Test".into(),
        };
        let _ = app.request_play(&sound, false);

        assert!(
            app.now_playing
                .current_key()
                .is_some_and(|key| key.matches(Some("new"), 0.0)),
            "cold play must invalidate stale waveform bars before decode completes"
        );
    }

    #[test]
    fn late_concurrent_decode_keeps_newest_in_now_playing() {
        let mut app = HonkHonk::new_for_test();
        let (handle, _evt_tx) = crate::audio::test_handle();
        app.audio = Some(handle);
        // Default overlap mode is Concurrent, so superseded presses stay pending.

        let sound = |id: &str| SoundEntry {
            id: id.into(),
            name: id.to_uppercase(),
            path: format!("/tmp/{id}.wav").into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: Some(100),
            category: "Test".into(),
        };

        // Two cold presses: each bumps the generation (A=1, B=2) and, in
        // concurrent mode, both stay pending awaiting their decode.
        let _ = app.request_play(&sound("a"), false);
        let _ = app.request_play(&sound("b"), false);

        let effects = app.effects_ui.to_effect_settings();
        let pcm = || crate::audio::CachedPcm {
            samples: std::sync::Arc::new(vec![0.0_f32; 16]),
            sample_rate: 48_000,
            channels: 1,
            duration: std::time::Duration::from_millis(100),
        };
        let dispatch = |generation: u64| PlaybackDispatch {
            generation,
            voice_id: generation,
            gain: 1.0,
            effects,
            mode: PlayMode::Concurrent,
        };

        // Decodes land out of order: the newest press (B) finishes first, then
        // the older press (A).
        let _ = app.handle_decoded("b".into(), Ok(pcm()), dispatch(2));
        let _ = app.handle_decoded("a".into(), Ok(pcm()), dispatch(1));

        // The older decode must still start its audio (cached + accepted, not
        // dropped as stale)...
        assert!(
            app.audio_store.get_pcm("a").is_some(),
            "older concurrent decode must still start playing"
        );
        // ...but the newest press keeps ownership of the highlight/playhead.
        assert_eq!(
            app.playing.as_deref(),
            Some("b"),
            "a late older decode must not retake `playing` from the newer press"
        );
        assert!(
            app.now_playing
                .current_key()
                .is_some_and(|key| key.matches(Some("b"), 0.0)),
            "a late older decode must not retake the now-playing highlight"
        );
    }
}
