use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::state::error::ConfigError;

const SLOTS_FILE_NAME: &str = "slots.json";
const CONFIG_DIR_NAME: &str = "honkhonk";
const SLOT_COUNT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlotMap(pub [Option<PathBuf>; SLOT_COUNT]);

impl Default for SlotMap {
    fn default() -> Self {
        Self(std::array::from_fn(|_| None))
    }
}

impl SlotMap {
    pub fn get(&self, idx: u8) -> Option<&PathBuf> {
        self.0.get(idx as usize)?.as_ref()
    }

    pub fn set(&mut self, idx: u8, path: PathBuf) {
        if let Some(slot) = self.0.get_mut(idx as usize) {
            *slot = Some(path);
        }
    }

    pub fn clear(&mut self, idx: u8) {
        if let Some(slot) = self.0.get_mut(idx as usize) {
            *slot = None;
        }
    }

    pub fn slot_for(&self, path: &Path) -> Option<u8> {
        self.0
            .iter()
            .position(|slot| slot.as_deref() == Some(path))
            .map(|i| i as u8)
    }

    fn slots_path() -> Result<PathBuf, ConfigError> {
        let proj = directories::ProjectDirs::from("", "", CONFIG_DIR_NAME)
            .ok_or(ConfigError::NoConfigDir)?;
        Ok(proj.config_dir().join(SLOTS_FILE_NAME))
    }

    pub fn load() -> Self {
        Self::slots_path()
            .ok()
            .and_then(|path| std::fs::read_to_string(&path).ok())
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&Self::slots_path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::DirectoryCreation {
                path: parent.display().to_string(),
                source: e,
            })?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| ConfigError::Serialize {
            path: path.display().to_string(),
            source: e,
        })?;
        std::fs::write(path, json).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_has_all_empty_slots() {
        let slots = SlotMap::default();
        for i in 0u8..20 {
            assert!(slots.get(i).is_none(), "slot {i} should be empty");
        }
    }

    #[test]
    fn set_and_get_round_trip() {
        let mut slots = SlotMap::default();
        let path = PathBuf::from("/sounds/honk.mp3");
        slots.set(0, path.clone());
        assert_eq!(slots.get(0), Some(&path));
    }

    #[test]
    fn set_last_slot() {
        let mut slots = SlotMap::default();
        let path = PathBuf::from("/sounds/boom.mp3");
        slots.set(19, path.clone());
        assert_eq!(slots.get(19), Some(&path));
        assert!(slots.get(18).is_none());
    }

    #[test]
    fn set_out_of_bounds_is_silent_noop() {
        let mut slots = SlotMap::default();
        slots.set(20, PathBuf::from("/sounds/boom.mp3"));
        slots.set(255, PathBuf::from("/sounds/boom.mp3"));
    }

    #[test]
    fn clear_removes_slot() {
        let mut slots = SlotMap::default();
        slots.set(3, PathBuf::from("/sounds/vine.mp3"));
        slots.clear(3);
        assert!(slots.get(3).is_none());
    }

    #[test]
    fn slot_for_returns_correct_index() {
        let mut slots = SlotMap::default();
        let path = PathBuf::from("/sounds/boom.mp3");
        slots.set(5, path.clone());
        assert_eq!(slots.slot_for(&path), Some(5));
    }

    #[test]
    fn slot_for_returns_none_when_unassigned() {
        let slots = SlotMap::default();
        assert!(slots.slot_for(Path::new("/sounds/boom.mp3")).is_none());
    }

    #[test]
    fn slot_for_returns_first_match() {
        let mut slots = SlotMap::default();
        let path = PathBuf::from("/sounds/honk.mp3");
        slots.set(2, path.clone());
        slots.set(7, path.clone());
        let found = slots.slot_for(&path).unwrap();
        assert_eq!(found, 2);
    }

    #[test]
    fn save_to_and_load_from_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("slots.json");

        let mut slots = SlotMap::default();
        slots.set(0, PathBuf::from("/sounds/a.mp3"));
        slots.set(9, PathBuf::from("/sounds/b.flac"));
        slots.set(19, PathBuf::from("/sounds/c.wav"));

        slots.save_to(&path).unwrap();
        let loaded = SlotMap::load_from(&path);

        assert_eq!(loaded.get(0), Some(&PathBuf::from("/sounds/a.mp3")));
        assert_eq!(loaded.get(9), Some(&PathBuf::from("/sounds/b.flac")));
        assert_eq!(loaded.get(19), Some(&PathBuf::from("/sounds/c.wav")));
        assert!(loaded.get(1).is_none());
    }

    #[test]
    fn load_from_missing_file_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let loaded = SlotMap::load_from(&path);
        for i in 0u8..20 {
            assert!(loaded.get(i).is_none());
        }
    }

    #[test]
    fn load_from_corrupt_file_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrupt.json");
        std::fs::write(&path, b"not valid json !!!").unwrap();
        let loaded = SlotMap::load_from(&path);
        for i in 0u8..20 {
            assert!(loaded.get(i).is_none());
        }
    }
}
