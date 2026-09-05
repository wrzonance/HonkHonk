use super::*;
use crate::audio::{AudioCommand, CachedPcm, PlayMode};
use crate::state::import::transform;

const PREVIEW_VOICE: u64 = u64::MAX;

impl HonkHonk {
    pub(in crate::app) fn stop_import_preview(&mut self) {
        self.import.preview = self.import.preview.wrapping_add(1);
        self.send_audio_commands([AudioCommand::StopVoice(PREVIEW_VOICE)]);
    }

    pub(super) fn preview_import(&mut self, index: usize) -> Task<Message> {
        if self.import.scanning {
            return Task::none();
        }
        let Some(row) = self
            .import
            .report
            .rows
            .get(index)
            .filter(|r| r.error.is_none())
            .cloned()
        else {
            return Task::none();
        };
        let _ = self.stop_all();
        self.stop_import_preview();
        let serial = self.import.preview;
        let generation = self.play_generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let audio = transform::prepare(
                        transform::decode(&row.source)?,
                        row.normalize,
                        row.trim,
                    );
                    measured_preview(audio)
                })
                .await
                .map_err(|e| {
                    ImportError::from(anyhow::Error::new(e).context("preview worker failed"))
                })
                .and_then(|r| r)
            },
            move |result| Message::Import(ImportMessage::Previewed(serial, generation, result)),
        )
    }

    pub(super) fn import_previewed(
        &mut self,
        serial: u64,
        generation: u64,
        result: Result<CachedPcm, ImportError>,
    ) {
        if self.import.preview != serial || self.play_generation != generation {
            return;
        }
        match result {
            Ok(pcm) => self.send_audio_commands([AudioCommand::Play {
                processing: crate::audio::processing::VoiceProcessing {
                    normalization_gain: if self.config.processing.normalize {
                        pcm.analysis.normalization_gain
                    } else {
                        1.0
                    },
                    ..Default::default()
                },
                voice_id: PREVIEW_VOICE,
                sound_id: "import-preview".into(),
                samples: pcm.samples,
                sample_rate: pcm.sample_rate,
                channels: pcm.channels,
                generation: generation.wrapping_sub(1),
                gain: 1.0,
                effects: Default::default(),
                mode: PlayMode::Interrupt,
            }]),
            Err(error) => self.import.status = error.to_string(),
        }
    }
}

fn measured_preview(audio: crate::audio::DecodedAudio) -> Result<CachedPcm, ImportError> {
    let normalization_gain =
        crate::audio::processing::measure_gain(&audio.samples, audio.sample_rate, audio.channels)
            .map_err(|e| {
            ImportError::from(anyhow::Error::new(e).context("measure preview loudness"))
        })?;
    Ok(CachedPcm {
        analysis: crate::audio::processing::AudioAnalysis {
            normalization_gain,
            repaired_channel: audio.repaired_channel,
            ..Default::default()
        },
        samples: Arc::new(audio.samples),
        sample_rate: audio.sample_rate,
        channels: audio.channels,
        duration: audio.duration,
    })
}
