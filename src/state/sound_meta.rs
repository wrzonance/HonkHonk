use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::state::error::ConfigError;

const META_FILE_NAME: &str = "sound_meta.json";
const CONFIG_DIR_NAME: &str = "honkhonk";

/// Per-sound user customisations persisted independently of library scan.
/// Keyed by sound ID (deterministic hex hash of file path).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundMeta {
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
}

fn default_volume() -> f32 {
    1.0
}

impl Default for SoundMeta {
    fn default() -> Self {
        Self {
            favorite: false,
            volume: 1.0,
            display_name: None,
        }
    }
}

impl SoundMeta {
    pub fn is_default(&self) -> bool {
        !self.favorite && (self.volume - 1.0).abs() < f32::EPSILON && self.display_name.is_none()
    }
}

/// In-memory store for all sound metadata, backed by a JSON file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SoundMetaStore(HashMap<String, SoundMeta>);

impl SoundMetaStore {
    /// Returns metadata for a sound, falling back to default if not set.
    pub fn get(&self, id: &str) -> SoundMeta {
        self.0.get(id).cloned().unwrap_or_default()
    }

    /// Returns a reference to the metadata if it exists.
    pub fn get_ref(&self, id: &str) -> Option<&SoundMeta> {
        self.0.get(id)
    }

    /// Upserts metadata for a sound. Removes the entry if it becomes default.
    pub fn set(&mut self, id: String, meta: SoundMeta) {
        if meta.is_default() {
            self.0.remove(&id);
        } else {
            self.0.insert(id, meta);
        }
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

    /// Returns `true` if the sound is a favorite.
    pub fn is_favorite(&self, id: &str) -> bool {
        self.0.get(id).map(|m| m.favorite).unwrap_or(false)
    }

    /// Returns the per-sound volume multiplier (defaults to 1.0).
    pub fn volume_for(&self, id: &str) -> f32 {
        self.0.get(id).map(|m| m.volume).unwrap_or(1.0)
    }

    fn meta_path() -> Result<std::path::PathBuf, ConfigError> {
        let proj = directories::ProjectDirs::from("", "", CONFIG_DIR_NAME)
            .ok_or(ConfigError::NoConfigDir)?;
        Ok(proj.config_dir().join(META_FILE_NAME))
    }

    /// Loads the store from the default XDG path, returning an empty store on
    /// any error (missing file, corrupt JSON).
    pub fn load() -> Self {
        Self::meta_path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<HashMap<String, SoundMeta>>(&s).ok())
            .map(SoundMetaStore)
            .unwrap_or_default()
    }

    /// Persists the store to the default XDG path.
    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&Self::meta_path()?)
    }

    /// Persists the store to an arbitrary path (used in tests).
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::DirectoryCreation {
                path: parent.display().to_string(),
                source: e,
            })?;
        }
        let json =
            serde_json::to_string_pretty(&self.0).map_err(|e| ConfigError::Serialize {
                path: path.display().to_string(),
                source: e,
            })?;
        std::fs::write(path, json).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    /// Loads from an arbitrary path (used in tests).
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<HashMap<String, SoundMeta>>(&s).ok())
            .map(SoundMetaStore)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_meta_is_not_favorite() {
        let store = SoundMetaStore::default();
        assert!(!store.is_favorite("any-id"));
    }

    #[test]
    fn default_volume_is_one() {
        let store = SoundMetaStore::default();
        let eps = f32::EPSILON;
        assert!((store.volume_for("any-id") - 1.0).abs() < eps);
    }

    #[test]
    fn toggle_favorite_sets_true_then_false() {
        let mut store = SoundMetaStore::default();
        assert!(store.toggle_favorite("id1"));
        assert!(!store.toggle_favorite("id1"));
    }

    #[test]
    fn set_volume_clamps_to_range() {
        let mut store = SoundMetaStore::default();
        store.set_volume("id1", 3.0);
        let eps = f32::EPSILON;
        assert!((store.volume_for("id1") - 2.0).abs() < eps);
        store.set_volume("id1", -0.5);
        assert!((store.volume_for("id1") - 0.0).abs() < eps);
    }

    #[test]
    fn set_volume_in_range_is_preserved() {
        let mut store = SoundMetaStore::default();
        store.set_volume("id1", 1.5);
        let eps = 1e-5_f32;
        assert!((store.volume_for("id1") - 1.5).abs() < eps);
    }

    #[test]
    fn set_cleans_up_default_entries() {
        let mut store = SoundMetaStore::default();
        store.set_volume("id1", 1.5);
        // Reset to default
        store.set("id1".to_owned(), SoundMeta::default());
        assert!(store.0.is_empty(), "default meta should be pruned from map");
    }

    #[test]
    fn set_display_name_stores_override() {
        let mut store = SoundMetaStore::default();
        store.set_display_name("id1", Some("My Honk".to_owned()));
        assert_eq!(
            store.get("id1").display_name.as_deref(),
            Some("My Honk")
        );
    }

    #[test]
    fn set_display_name_none_clears_override() {
        let mut store = SoundMetaStore::default();
        store.set_display_name("id1", Some("Override".to_owned()));
        store.set_display_name("id1", None);
        assert!(store.get("id1").display_name.is_none());
    }

    #[test]
    fn save_and_load_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("meta.json");

        let mut store = SoundMetaStore::default();
        store.toggle_favorite("abc");
        store.set_volume("abc", 1.25);
        store.save_to(&path).unwrap();

        let loaded = SoundMetaStore::load_from(&path);
        assert!(loaded.is_favorite("abc"));
        let eps = 1e-5_f32;
        assert!((loaded.volume_for("abc") - 1.25).abs() < eps);
    }

    #[test]
    fn load_from_missing_file_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let store = SoundMetaStore::load_from(&path);
        assert!(!store.is_favorite("any"));
    }

    #[test]
    fn load_from_corrupt_file_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"not json!!!").unwrap();
        let store = SoundMetaStore::load_from(&path);
        assert!(!store.is_favorite("any"));
    }

    #[test]
    fn is_default_detects_all_fields_at_default() {
        assert!(SoundMeta::default().is_default());
        assert!(!SoundMeta {
            favorite: true,
            ..Default::default()
        }
        .is_default());
    }
}
