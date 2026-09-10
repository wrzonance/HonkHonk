//! Background library preparation orchestration and progress state.

use std::sync::Arc;

use iced::futures::stream::BoxStream;

use super::{HonkHonk, Message};

#[derive(Clone)]
pub(super) struct PreparationRequest {
    pub(super) generation: u64,
    pub(super) files: Vec<(String, std::path::PathBuf)>,
    pub(super) coordinator: Arc<crate::audio::preparation::PreparationCoordinator>,
}

impl PartialEq for PreparationRequest {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation && self.files == other.files
    }
}

impl Eq for PreparationRequest {}

impl std::hash::Hash for PreparationRequest {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.generation.hash(state);
        self.files.hash(state);
    }
}

#[derive(Default)]
pub(super) struct PreparationState {
    pub(super) request: Option<Arc<PreparationRequest>>,
    pub(super) coordinator: Arc<crate::audio::preparation::PreparationCoordinator>,
    pub(super) generation: u64,
    pub(super) done: usize,
    pub(super) total: usize,
    pub(super) failures: Vec<(String, String)>,
}

pub(super) fn preparation_builder(
    request: &Arc<PreparationRequest>,
) -> BoxStream<'static, Message> {
    let request = Arc::clone(request);
    Box::pin(iced::stream::channel(16, async move |mut tx| {
        use iced::futures::SinkExt;

        let mut failures = Vec::new();
        for (id, path) in &request.files {
            let id = id.clone();
            let path = path.clone();
            let failure_path = path.clone();
            let result = request
                .coordinator
                .prepare(&path)
                .await
                .map_err(|error| error.to_string());
            if let Err(error) = &result {
                failures.push((failure_path.display().to_string(), error.clone()));
            }
            if tx
                .send(Message::LibraryPreparationItem {
                    generation: request.generation,
                    id,
                    result,
                })
                .await
                .is_err()
            {
                return;
            }
        }
        let _ = tx
            .send(Message::LibraryPreparationFinished {
                generation: request.generation,
                failures,
            })
            .await;
    }))
}

impl HonkHonk {
    pub(super) fn start_library_preparation(&mut self) {
        self.preparation.request = None;
        self.preparation.done = 0;
        self.preparation.total = self.sounds.len();
        self.preparation.failures.clear();
        if self.sounds.is_empty() {
            return;
        }
        self.preparation.generation = self.preparation.generation.wrapping_add(1);
        let generation = self.preparation.generation;
        self.preparation.request = Some(Arc::new(PreparationRequest {
            generation,
            files: self
                .sounds
                .iter()
                .map(|sound| (sound.id.clone(), sound.path.clone()))
                .collect(),
            coordinator: Arc::clone(&self.preparation.coordinator),
        }));
    }

    pub(super) fn library_preparation_item(
        &mut self,
        generation: u64,
        id: String,
        result: Result<Arc<crate::audio::preparation::PreparedAudio>, String>,
    ) {
        if self
            .preparation
            .request
            .as_ref()
            .map(|request| request.generation)
            != Some(generation)
        {
            return;
        }
        if let Ok(prepared) = result {
            let current_path = self
                .sounds
                .iter()
                .find(|sound| sound.id == id)
                .map(|sound| sound.path.as_path());
            let Some(current_path) = current_path else {
                self.preparation.done = self.preparation.done.saturating_add(1);
                return;
            };
            if prepared.has_source_identity() && !prepared.matches_path(current_path) {
                self.preparation.done = self.preparation.done.saturating_add(1);
                return;
            }
            self.now_playing
                .cache_envelope_arc(&id, Arc::clone(&prepared.envelope));
            let evicted = self.audio_store.insert_pcm(id, Arc::clone(&prepared.pcm));
            self.evict_waveform_envelopes(evicted);
        }
        self.preparation.done = self.preparation.done.saturating_add(1);
    }

    pub(super) fn library_preparation_finished(
        &mut self,
        generation: u64,
        failures: Vec<(String, String)>,
    ) {
        if self
            .preparation
            .request
            .as_ref()
            .map(|request| request.generation)
            != Some(generation)
        {
            return;
        }
        self.preparation.failures = failures;
        self.preparation.request = None;
        if self.preparation.failures.is_empty() {
            tracing::info!(
                count = self.preparation.total,
                "library preparation complete"
            );
        } else {
            self.notify_preparation_failures();
        }
    }

    fn notify_preparation_failures(&mut self) {
        tracing::warn!(
            count = self.preparation.failures.len(),
            "library preparation found unreadable files"
        );
        self.log_preparation_failures();
        self.notices.push(
            crate::app::notices::Notice::warning(
                "Some sounds could not be prepared",
                self.preparation_failure_details(),
            ),
            std::time::Instant::now(),
        );
    }

    fn preparation_failure_details(&self) -> String {
        self.preparation
            .failures
            .iter()
            .map(|(id, error)| format!("{id}: {error}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn log_preparation_failures(&self) {
        for (file, error) in &self.preparation.failures {
            tracing::warn!(file = %file, error = %error, "library preparation failed");
        }
    }

    pub(crate) fn library_preparation_progress(&self) -> Option<(usize, usize)> {
        self.preparation
            .request
            .as_ref()
            .map(|_| (self.preparation.done, self.preparation.total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_progress_is_scoped_to_current_generation() {
        let mut app = HonkHonk::new_for_test();
        app.sounds.push(crate::state::SoundEntry {
            id: "missing".into(),
            name: "missing".into(),
            path: "/does/not/exist.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            modified_ms: None,
            category: "test".into(),
        });
        app.start_library_preparation();
        let generation = app.preparation.request.as_ref().unwrap().generation;
        app.library_preparation_item(
            generation.wrapping_sub(1),
            "missing".into(),
            Ok(test_prepared()),
        );
        assert_eq!(app.library_preparation_progress(), Some((0, 1)));
    }

    #[test]
    fn prepared_pcm_and_waveform_share_the_existing_caches() {
        let mut app = HonkHonk::new_for_test();
        app.sounds.push(crate::state::SoundEntry {
            id: "ready".into(),
            name: "ready".into(),
            path: "/tmp/ready.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            modified_ms: None,
            category: "test".into(),
        });
        app.start_library_preparation();
        let generation = app.preparation.request.as_ref().unwrap().generation;
        app.library_preparation_item(generation, "ready".into(), Ok(test_prepared()));

        assert!(app.audio_store.get_pcm("ready").is_some());
        assert!(app.now_playing.envelope("ready").is_some());
        assert_eq!(app.library_preparation_progress(), Some((1, 1)));
    }

    #[test]
    fn mixed_results_continue_and_emit_one_end_summary() {
        let mut app = HonkHonk::new_for_test();
        app.sounds.push(crate::state::SoundEntry {
            id: "broken".into(),
            name: "broken".into(),
            path: "/tmp/broken.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            modified_ms: None,
            category: "test".into(),
        });
        app.start_library_preparation();
        let generation = app.preparation.request.as_ref().unwrap().generation;
        app.library_preparation_item(generation, "broken".into(), Err("corrupt".into()));
        app.library_preparation_finished(generation, vec![("broken".into(), "corrupt".into())]);

        assert!(app.preparation.request.is_none());
        assert_eq!(app.notices.len(), 1);
        assert!(app.notices.front().unwrap().notice.body.contains("corrupt"));
    }

    #[test]
    fn stale_completion_after_rescan_to_empty_is_ignored() {
        let mut app = HonkHonk::new_for_test();
        app.sounds.push(crate::state::SoundEntry {
            id: "old".into(),
            name: "old".into(),
            path: "/tmp/old.wav".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            modified_ms: None,
            category: "test".into(),
        });
        app.start_library_preparation();
        let generation = app.preparation.request.as_ref().unwrap().generation;
        app.sounds.clear();
        app.start_library_preparation();
        app.library_preparation_item(generation, "old".into(), Ok(test_prepared()));
        app.library_preparation_finished(generation, vec![("old".into(), "late".into())]);

        assert!(app.preparation.request.is_none());
        assert!(app.audio_store.get_pcm("old").is_none());
        assert!(app.notices.is_empty());
    }

    fn test_prepared() -> Arc<crate::audio::preparation::PreparedAudio> {
        let pcm = crate::audio::CachedPcm {
            analysis: Default::default(),
            samples: Arc::new(vec![0.0]),
            sample_rate: 8_000,
            channels: 1,
            duration: std::time::Duration::from_millis(1),
        };
        crate::audio::preparation::wrap_for_test(pcm)
    }
}
