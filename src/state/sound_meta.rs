use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

mod persistence;
mod processing;
use crate::audio::processing::SoundProcessing;
use processing::AudioPreferences;

const META_FILE_NAME: &str = "sound_meta.json";
const CONFIG_DIR_NAME: &str = "honkhonk";
const META_FORMAT_VERSION: u32 = 2;
const MAX_GRAPHIC_REF_BYTES: usize = 255;

/// An application-owned filename for a copied tile graphic.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct GraphicAssetRef(String);

impl GraphicAssetRef {
    pub fn new(filename: impl Into<String>) -> Result<Self, GraphicRefError> {
        let filename = filename.into();
        validate_graphic_ref(&filename)?;
        Ok(Self(filename))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for GraphicAssetRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let filename = String::deserialize(deserializer)?;
        Self::new(filename).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GraphicRefError {
    #[error("graphic filename cannot be empty")]
    Empty,
    #[error("graphic filename exceeds {MAX_GRAPHIC_REF_BYTES} UTF-8 bytes")]
    TooLong,
    #[error("graphic reference must be one ordinary filename component")]
    InvalidComponent,
    #[error("graphic filename cannot contain control characters")]
    ControlCharacter,
}

fn validate_graphic_ref(filename: &str) -> Result<(), GraphicRefError> {
    if filename.is_empty() {
        return Err(GraphicRefError::Empty);
    }
    if filename.len() > MAX_GRAPHIC_REF_BYTES {
        return Err(GraphicRefError::TooLong);
    }
    if filename.chars().any(char::is_control) {
        return Err(GraphicRefError::ControlCharacter);
    }
    if matches!(filename, "." | "..") || filename.contains(['/', '\\']) {
        return Err(GraphicRefError::InvalidComponent);
    }
    Ok(())
}

/// Per-sound user customisations persisted independently of library scan.
/// Keyed by sound ID (deterministic hex hash of file path).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundMeta {
    #[serde(default)]
    pub processing: SoundProcessing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
    /// User tags, normalized on input; absent in older metadata files.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "deserialize_tags"
    )]
    pub tags: Vec<String>,
    /// Star / unstar: included in "Favorites" filtered view.
    #[serde(default)]
    pub favorite: bool,
    /// Per-sound volume multiplier applied on top of master volume.
    /// 1.0 = no change. Range: [0.0, 2.0].
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Optional display-name override. `None` means use the filename stem.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Application-owned filename for an assigned tile graphic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_graphic: Option<GraphicAssetRef>,
}

fn default_volume() -> f32 {
    1.0
}

impl Default for SoundMeta {
    fn default() -> Self {
        Self {
            favorite: false,
            processing: SoundProcessing::default(),
            color: None,
            volume: 1.0,
            display_name: None,
            assigned_graphic: None,
            tags: Vec::new(),
        }
    }
}

impl SoundMeta {
    pub fn is_default(&self) -> bool {
        !self.favorite
            && self.processing == SoundProcessing::default()
            && self.color.is_none()
            && (self.volume - 1.0).abs() < f32::EPSILON
            && self.display_name.is_none()
            && self.assigned_graphic.is_none()
            && self.tags.is_empty()
    }
}

/// In-memory store for all sound metadata, backed by a JSON file.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundMetaStore {
    fingerprints: HashMap<String, String>,
    audio: HashMap<String, AudioPreferences>,
    custom: HashMap<String, SoundMeta>,
    added: BTreeMap<String, u64>,
    writable: bool,
}

impl Default for SoundMetaStore {
    fn default() -> Self {
        Self {
            custom: HashMap::new(),
            fingerprints: HashMap::new(),
            audio: HashMap::new(),
            added: BTreeMap::new(),
            writable: true,
        }
    }
}

impl SoundMetaStore {
    fn read_protected() -> Self {
        Self {
            writable: false,
            ..Self::default()
        }
    }

    /// Returns metadata for a sound, falling back to default if not set.
    pub fn get(&self, id: &str) -> SoundMeta {
        self.custom.get(id).cloned().unwrap_or_default()
    }

    /// Returns a reference to the metadata if it exists.
    pub fn get_ref(&self, id: &str) -> Option<&SoundMeta> {
        self.custom.get(id)
    }

    /// Upserts metadata for a sound. Removes the entry if it becomes default.
    pub fn set(&mut self, id: String, mut meta: SoundMeta) {
        meta.tags = normalize_tags(meta.tags);
        meta.processing = meta.processing.sanitized();
        meta.volume = if meta.volume.is_finite() {
            meta.volume.clamp(0.0, 2.0)
        } else {
            1.0
        };
        self.share_audio_preferences(&id, &meta);
        if meta.is_default() {
            self.custom.remove(&id);
        } else {
            self.custom.insert(id, meta);
        }
    }

    /// Replaces tags, normalizing whitespace and case-insensitive duplicates.
    pub fn set_tags(&mut self, id: &str, tags: Vec<String>) {
        let mut meta = self.get(id);
        meta.tags = tags;
        self.set(id.to_owned(), meta);
    }

    /// Toggles the favorite flag for a sound, returning the new value.
    pub fn toggle_favorite(&mut self, id: &str) -> bool {
        let mut meta = self.get(id);
        meta.favorite = !meta.favorite;
        let new_val = meta.favorite;
        self.set(id.to_owned(), meta);
        new_val
    }

    /// Sets per-sound volume for a sound.
    pub fn set_volume(&mut self, id: &str, volume: f32) {
        let mut meta = self.get(id);
        meta.volume = volume.clamp(0.0, 2.0);
        self.set(id.to_owned(), meta);
    }

    /// Sets the display name override for a sound. Pass `None` to clear.
    pub fn set_display_name(&mut self, id: &str, name: Option<String>) {
        let mut meta = self.get(id);
        meta.display_name = name;
        self.set(id.to_owned(), meta);
    }

    pub fn assigned_graphic(&self, id: &str) -> Option<&GraphicAssetRef> {
        self.custom
            .get(id)
            .and_then(|meta| meta.assigned_graphic.as_ref())
    }

    pub fn set_assigned_graphic(&mut self, id: &str, graphic: GraphicAssetRef) {
        let mut meta = self.get(id);
        meta.assigned_graphic = Some(graphic);
        self.set(id.to_owned(), meta);
    }

    pub fn clear_assigned_graphic(&mut self, id: &str) {
        let mut meta = self.get(id);
        meta.assigned_graphic = None;
        self.set(id.to_owned(), meta);
    }

    /// Returns `true` if the sound is a favorite.
    pub fn is_favorite(&self, id: &str) -> bool {
        self.custom.get(id).map(|m| m.favorite).unwrap_or(false)
    }

    /// Returns the per-sound volume multiplier (defaults to 1.0).
    pub fn volume_for(&self, id: &str) -> f32 {
        self.custom.get(id).map(|m| m.volume).unwrap_or(1.0)
    }

    pub fn added_ms(&self, id: &str) -> Option<u64> {
        self.added.get(id).copied()
    }

    pub fn reconcile_added<I, S>(&mut self, ids: I, observed_at_ms: u64, complete: bool) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let observed: BTreeSet<String> = ids.into_iter().map(|id| id.as_ref().to_owned()).collect();
        let mut changed = false;

        for id in &observed {
            if !self.added.contains_key(id) {
                self.added.insert(id.clone(), observed_at_ms);
                changed = true;
            }
        }

        if complete {
            let previous_len = self.added.len();
            self.added.retain(|id, _| observed.contains(id));
            changed |= self.added.len() != previous_len;
        }

        changed
    }
}

#[cfg(test)]
mod tests;

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    tags.into_iter()
        .map(|tag| tag.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|tag| !tag.is_empty() && seen.insert(tag.to_lowercase()))
        .collect()
}

fn deserialize_tags<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    Vec::<String>::deserialize(deserializer).map(normalize_tags)
}
