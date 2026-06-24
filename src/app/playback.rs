//! Play-dispatch coordination extracted from `app/mod.rs` to keep the Iced
//! Application thin (CLAUDE.md: `app.rs` delegates to module APIs and must not
//! grow). Owns the click -> (warm PCM cache hit | async decode) -> fire
//! pipeline and the `Message::Decoded` landing, as methods on `HonkHonk` so
//! they share its state directly. See spec #151.

use super::*;

impl HonkHonk {
    /// Begins playing `sound`. A warm PCM cache hit fires synchronously; a miss
    /// returns a `Task` that decodes off the UI thread and yields
    /// `Message::Decoded`. The play generation is bumped here so a stale decode
    /// (superseded by a newer press) is dropped on arrival (#149/#151).
    pub(super) fn request_play(&mut self, sound: &SoundEntry, stop_before: bool) -> Task<Message> {
        self.play_generation = self.play_generation.wrapping_add(1);
        let generation = self.play_generation;
        self.playing = Some(sound.id.clone());
        if stop_before {
            if let Some(ref audio) = self.audio {
                audio.send(AudioCommand::Stop);
            }
        }
        if let Some(pcm) = self.audio_store.get_pcm(&sound.id) {
            self.start_playback(
                &sound.id,
                pcm,
                self.sound_meta.volume_for(&sound.id),
                generation,
            );
            return Task::none();
        }
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
                generation,
                id: id.clone(),
                result,
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
        generation: u64,
        id: String,
        result: Result<crate::audio::CachedPcm, String>,
    ) -> Task<Message> {
        // Drop the decode unless it is still the wanted play: the generation
        // rules out a superseded press, and `playing == Some(id)` rules out a
        // play torn down while the decode was in flight (StopAll, a genuine end).
        // The `playing` check is what stops a StopAll mid-decode from
        // resurrecting a stopped sound — without bumping the generation, which
        // would desync the #149 Started/Finished reconciliation (#151).
        if generation != self.play_generation || self.playing.as_deref() != Some(id.as_str()) {
            return Task::none();
        }
        match result {
            Ok(pcm) => {
                let volume = self.sound_meta.volume_for(&id);
                let pcm = std::sync::Arc::new(pcm);
                self.audio_store
                    .insert_pcm(id.clone(), std::sync::Arc::clone(&pcm));
                self.start_playback(&id, pcm, volume, generation);
            }
            Err(e) => {
                let file = self
                    .sounds
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.path.display().to_string())
                    .unwrap_or_else(|| id.clone()); // fall back to id if rescanned away
                tracing::error!(file = %file, error = %e, "decode failed");
                self.clear_playback_state();
            }
        }
        Task::none()
    }

    /// Starts the playhead from the decoded duration, ensures the waveform
    /// envelope is cached, and dispatches the engine `Play`. Shared by the warm
    /// cache-hit path and `handle_decoded`.
    fn start_playback(
        &mut self,
        id: &str,
        pcm: std::sync::Arc<crate::audio::CachedPcm>,
        volume: f32,
        generation: u64,
    ) {
        self.now_playing.start(now_playing::PlaybackStart {
            id,
            duration: pcm.duration,
            samples: pcm.samples.as_ref().as_slice(),
            channels: pcm.channels,
            now: Instant::now(),
        });
        if let Some(ref audio) = self.audio {
            audio.send(AudioCommand::Play {
                sound_id: id.to_string(),
                samples: std::sync::Arc::clone(&pcm.samples),
                sample_rate: pcm.sample_rate,
                channels: pcm.channels,
                generation,
                volume,
            });
            self.playing = Some(id.to_string());
        }
    }
}
