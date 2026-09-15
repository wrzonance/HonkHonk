use super::{SoundMeta, SoundMetaStore};
use crate::audio::processing::SoundProcessing;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct AudioPreferences {
    volume: f32,
    processing: SoundProcessing,
}

impl AudioPreferences {
    fn from_meta(meta: &SoundMeta) -> Self {
        Self {
            volume: meta.volume,
            processing: meta.processing,
        }
    }

    fn apply(&self, meta: &mut SoundMeta) {
        meta.volume = self.volume;
        meta.processing = self.processing;
    }
}

impl SoundMetaStore {
    /// Bind a background-computed content hash. Audio preferences follow bytes;
    /// names, tags, artwork and favorites remain attached to the library path.
    pub fn bind_fingerprint(&mut self, id: &str, fingerprint: &str) -> bool {
        if fingerprint.is_empty()
            || self
                .fingerprints
                .get(id)
                .is_some_and(|old| old == fingerprint)
        {
            return false;
        }
        let mut meta = self.get(id);
        if let Some(audio) = self.audio.get(fingerprint) {
            audio.apply(&mut meta);
        } else if self.fingerprints.contains_key(id) {
            // New bytes at a previously bound path must not inherit the old audio.
            AudioPreferences::from_meta(&SoundMeta::default()).apply(&mut meta);
        }
        self.fingerprints
            .insert(id.to_owned(), fingerprint.to_owned());
        self.set(id.to_owned(), meta);
        true
    }

    pub(super) fn share_audio_preferences(&mut self, id: &str, meta: &SoundMeta) {
        let Some(fingerprint) = self.fingerprints.get(id) else {
            return;
        };
        let preferences = AudioPreferences::from_meta(meta);
        self.audio.insert(fingerprint.clone(), preferences.clone());
        for (other_id, other_fingerprint) in &self.fingerprints {
            if other_id != id && other_fingerprint == fingerprint {
                let other = self.custom.entry(other_id.clone()).or_default();
                preferences.apply(other);
            }
        }
    }
}
