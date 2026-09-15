use super::*;
use crate::audio::processing::{GlobalProcessing, SoundProcessing, VoiceProcessing};

#[derive(Default)]
pub(super) struct ProcessingUi {
    pub draft: SoundProcessing,
    pub loading: bool,
    pub generation: u64,
}

impl HonkHonk {
    pub(super) fn adopt_audio_identity(&mut self, id: &str, pcm: &crate::audio::CachedPcm) {
        if self
            .sound_meta
            .bind_fingerprint(id, &pcm.analysis.fingerprint)
        {
            self.persist_sound_metadata();
        }
        if pcm.analysis.repaired_channel {
            self.notices.push(Notice::info("Silent channel repaired",
                "One nearly silent channel was replaced with the active channel. Both sides now play the same audio."), Instant::now());
        }
    }

    pub(super) fn voice_processing(
        &self,
        id: &str,
        pcm: &crate::audio::CachedPcm,
    ) -> VoiceProcessing {
        let sound = self.sound_meta.get(id).processing.sanitized();
        VoiceProcessing {
            normalization_gain: if sound.normalize.unwrap_or(self.config.processing.normalize) {
                pcm.analysis.normalization_gain
            } else {
                1.0
            },
            sound,
        }
    }

    pub(super) fn change_processing(&mut self, settings: GlobalProcessing) -> Task<Message> {
        self.config.processing = GlobalProcessing {
            dynamics: settings.dynamics.sanitized(),
            ..settings
        };
        self.send_audio_commands([AudioCommand::SetDynamics(self.config.processing.dynamics)]);
        self.persist_config();
        Task::none()
    }

    pub(super) fn playback_error(&mut self, id: &str, error: &str) {
        self.notices.push(Notice::error("Sound could not play",
            format!("{id}: {error}. Check the file is readable and complete; try re-exporting as PCM WAV.")), Instant::now());
    }

    pub(super) fn load_editor_fingerprint(&mut self, id: String) -> Task<Message> {
        self.processing_ui.loading = false;
        self.processing_ui.generation = self.processing_ui.generation.wrapping_add(1);
        let generation = self.processing_ui.generation;
        let Some(sound) = self.sounds.iter().find(|s| s.id == id) else {
            return Task::none();
        };
        let path = sound.path.clone();
        self.processing_ui.loading = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || crate::audio::processing::fingerprint(&path))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.map_err(|e| e.to_string()))
            },
            move |result| Message::AudioFingerprintReady {
                id: id.clone(),
                generation,
                result,
            },
        )
    }

    pub(super) fn editor_fingerprint_ready(
        &mut self,
        id: String,
        generation: u64,
        result: Result<String, String>,
    ) -> Task<Message> {
        if self.processing_ui.generation != generation
            || self.editor_sound_id.as_deref() != Some(&id)
        {
            return Task::none();
        }
        self.processing_ui.loading = false;
        match result {
            Ok(identity) => {
                self.sound_meta.bind_fingerprint(&id, &identity);
                let meta = self.sound_meta.get(&id);
                self.processing_ui.draft = meta.processing;
                self.editor_draft_volume = meta.volume;
                self.persist_sound_metadata();
            }
            Err(error) => {
                self.notices.push(Notice::error("Sound identity could not load",
                    format!("{id}: {error}. Check the file is readable and complete, then reopen the editor to retry loading its audio preferences.")), Instant::now());
            }
        }
        Task::none()
    }
}
