//! Background library preparation orchestration and progress state.

use std::sync::Arc;

use iced::futures::stream::BoxStream;

use super::{HonkHonk, Message};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PreparationRequest {
    pub(super) generation: u64,
    pub(super) files: Vec<(String, std::path::PathBuf)>,
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
            let result =
                tokio::task::spawn_blocking(move || crate::audio::preparation::prepare(&path))
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(|error| error.to_string()));
            if let Err(error) = &result {
                failures.push((id.clone(), error.clone()));
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
        self.preparation = None;
        self.preparation_done = 0;
        self.preparation_total = self.sounds.len();
        self.preparation_failures.clear();
        if self.sounds.is_empty() {
            return;
        }
        self.preparation_generation = self.preparation_generation.wrapping_add(1);
        let generation = self.preparation_generation;
        self.preparation = Some(Arc::new(PreparationRequest {
            generation,
            files: self
                .sounds
                .iter()
                .map(|sound| (sound.id.clone(), sound.path.clone()))
                .collect(),
        }));
    }

    pub(super) fn library_preparation_item(
        &mut self,
        generation: u64,
        id: String,
        result: Result<crate::audio::CachedPcm, String>,
    ) {
        if self.preparation.as_ref().map(|request| request.generation) != Some(generation) {
            return;
        }
        if let Ok(pcm) = result {
            self.now_playing
                .cache_envelope(&id, pcm.samples.as_ref(), pcm.channels);
            let evicted = self.audio_store.insert_pcm(id, Arc::new(pcm));
            self.evict_waveform_envelopes(evicted);
        }
        self.preparation_done = self.preparation_done.saturating_add(1);
    }

    pub(super) fn library_preparation_finished(
        &mut self,
        generation: u64,
        failures: Vec<(String, String)>,
    ) {
        if self.preparation.as_ref().map(|request| request.generation) != Some(generation) {
            return;
        }
        self.preparation_failures = failures;
        self.preparation = None;
        if self.preparation_failures.is_empty() {
            tracing::info!(
                count = self.preparation_total,
                "library preparation complete"
            );
        } else {
            self.notify_preparation_failures();
        }
    }

    fn notify_preparation_failures(&mut self) {
        tracing::warn!(
            count = self.preparation_failures.len(),
            "library preparation found unreadable files"
        );
        let details = self
            .preparation_failures
            .iter()
            .map(|(id, error)| format!("{id}: {error}"))
            .collect::<Vec<_>>()
            .join("\n");
        self.notices.push(
            crate::app::notices::Notice::warning("Some sounds could not be prepared", details),
            std::time::Instant::now(),
        );
    }

    pub(crate) fn library_preparation_progress(&self) -> Option<(usize, usize)> {
        self.preparation
            .as_ref()
            .map(|_| (self.preparation_done, self.preparation_total))
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
        let generation = app.preparation.as_ref().unwrap().generation;
        app.library_preparation_item(generation.wrapping_sub(1), "missing".into(), Ok(test_pcm()));
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
        let generation = app.preparation.as_ref().unwrap().generation;
        app.library_preparation_item(generation, "ready".into(), Ok(test_pcm()));

        assert!(app.audio_store.get_pcm("ready").is_some());
        assert!(app.now_playing.envelope("ready").is_some());
        assert_eq!(app.library_preparation_progress(), Some((1, 1)));
    }

    fn test_pcm() -> crate::audio::CachedPcm {
        crate::audio::CachedPcm {
            analysis: Default::default(),
            samples: Arc::new(vec![0.0]),
            sample_rate: 8_000,
            channels: 1,
            duration: std::time::Duration::from_millis(1),
        }
    }
}
