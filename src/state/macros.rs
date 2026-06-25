//! Macro data model + persistence. A macro chains library sounds into a timed,
//! editable sequence (epic #170). Mirrors the one-JSON-file-per-collection
//! pattern of [`crate::state::slots`] / [`crate::state::sound_meta`]
//! (`ProjectDirs` + serde + [`ConfigError`]). State only — playback/scheduling
//! is #166, the timeline editor is #168.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::audio::effects::EffectSettings;
use crate::state::error::ConfigError;

const MACROS_FILE_NAME: &str = "macros.json";
const CONFIG_DIR_NAME: &str = "honkhonk";
const MIN_GAIN: f32 = 0.0;
const MAX_GAIN: f32 = 2.0;

fn default_gain() -> f32 {
    1.0
}

fn clamp_gain(gain: f32) -> f32 {
    gain.clamp(MIN_GAIN, MAX_GAIN)
}

/// One timed step: a library `sound` fired `start_offset_ms` after the macro
/// starts, at `gain`, through `effects`. Sound identity is a `PathBuf`, matching
/// [`crate::state::SlotMap`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub sound: PathBuf,
    #[serde(default)]
    pub start_offset_ms: u64,
    #[serde(default = "default_gain")]
    pub gain: f32,
    #[serde(default)]
    pub effects: EffectSettings,
}

impl Step {
    /// A full-volume, unprocessed step at `start_offset_ms`.
    pub fn new(sound: PathBuf, start_offset_ms: u64) -> Self {
        Self {
            sound,
            start_offset_ms,
            gain: 1.0,
            effects: EffectSettings::default(),
        }
    }

    /// Sets `gain`, clamped to `[0.0, 2.0]`.
    pub fn with_gain(mut self, gain: f32) -> Self {
        self.gain = clamp_gain(gain);
        self
    }
}

/// A named, ordered set of steps, addressed by a stable `id`. Slots reference
/// macros by this id (#169).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Macro {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub steps: Vec<Step>,
}

/// The full macro collection, persisted as `macros.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MacroStore(pub Vec<Macro>);

impl MacroStore {
    /// Appends a new, empty macro and returns it. The id is a stable hex hash of
    /// `name` + wall-clock nanos (+ a salt on the rare same-instant collision),
    /// so no new crate is needed for unique ids.
    pub fn add(&mut self, name: impl Into<String>) -> &Macro {
        let name = name.into();
        let mut salt = 0u64;
        let mut id = generate_id(&name, salt);
        while self.0.iter().any(|m| m.id == id) {
            salt += 1;
            id = generate_id(&name, salt);
        }
        self.0.push(Macro {
            id,
            name,
            steps: Vec::new(),
        });
        self.0.last().expect("just pushed a macro")
    }

    /// Removes the macro with `id`. Unknown id is a no-op.
    pub fn remove(&mut self, id: &str) {
        self.0.retain(|m| m.id != id);
    }

    /// Renames the macro with `id`. Unknown id is a no-op.
    pub fn rename(&mut self, id: &str, name: impl Into<String>) {
        if let Some(m) = self.get_mut(id) {
            m.name = name.into();
        }
    }

    pub fn get(&self, id: &str) -> Option<&Macro> {
        self.0.iter().find(|m| m.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Macro> {
        self.0.iter_mut().find(|m| m.id == id)
    }

    /// Replaces a macro's steps, clamping every step's `gain` to `[0.0, 2.0]`.
    /// Unknown id is a no-op.
    pub fn replace_steps(&mut self, id: &str, steps: Vec<Step>) {
        if let Some(m) = self.get_mut(id) {
            m.steps = steps
                .into_iter()
                .map(|mut s| {
                    s.gain = clamp_gain(s.gain);
                    s
                })
                .collect();
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Macro> {
        self.0.iter()
    }

    fn macros_path() -> Result<PathBuf, ConfigError> {
        let proj = directories::ProjectDirs::from("", "", CONFIG_DIR_NAME)
            .ok_or(ConfigError::NoConfigDir)?;
        Ok(proj.config_dir().join(MACROS_FILE_NAME))
    }

    /// Loads from the user config dir; an absent or corrupt file yields an empty
    /// store (matching `slots.rs`).
    pub fn load() -> Self {
        Self::macros_path()
            .ok()
            .and_then(|path| std::fs::read_to_string(&path).ok())
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&Self::macros_path()?)
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

/// Stable hex id from `name` + wall-clock nanos + `salt`. No new crate: reuses
/// the std `DefaultHasher` already used by [`crate::state::library`].
fn generate_id(name: &str, salt: u64) -> String {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    nanos.hash(&mut hasher);
    salt.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn step(sound: &str, offset: u64) -> Step {
        Step::new(PathBuf::from(sound), offset)
    }

    #[test]
    fn add_returns_unique_ids() {
        let mut store = MacroStore::default();
        let a = store.add("Air horn").id.clone();
        let b = store.add("Air horn").id.clone();
        assert_ne!(a, b, "same-name macros must still get distinct ids");
        assert_eq!(store.iter().count(), 2);
    }

    #[test]
    fn rename_and_get_mut_work() {
        let mut store = MacroStore::default();
        let id = store.add("old").id.clone();
        store.rename(&id, "new");
        assert_eq!(store.get(&id).unwrap().name, "new");
        store.get_mut(&id).unwrap().name = "newer".into();
        assert_eq!(store.get(&id).unwrap().name, "newer");
    }

    #[test]
    fn remove_drops_macro_and_unknown_id_is_noop() {
        let mut store = MacroStore::default();
        let id = store.add("doomed").id.clone();
        store.remove("does-not-exist");
        assert_eq!(store.iter().count(), 1);
        store.remove(&id);
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn unknown_id_setters_do_not_panic() {
        let mut store = MacroStore::default();
        store.rename("nope", "x");
        store.replace_steps("nope", vec![step("/s/a.wav", 0)]);
        assert!(store.get("nope").is_none());
        assert!(store.get_mut("nope").is_none());
    }

    #[test]
    fn replace_steps_clamps_gain() {
        let mut store = MacroStore::default();
        let id = store.add("clamp").id.clone();
        store.replace_steps(
            &id,
            vec![
                step("/s/loud.wav", 0).with_gain(9.0),
                step("/s/neg.wav", 100).with_gain(-3.0),
            ],
        );
        let steps = &store.get(&id).unwrap().steps;
        assert_eq!(steps[0].gain, MAX_GAIN);
        assert_eq!(steps[1].gain, MIN_GAIN);
    }

    #[test]
    fn with_gain_clamps() {
        assert_eq!(step("/s/a.wav", 0).with_gain(5.0).gain, MAX_GAIN);
        assert_eq!(step("/s/a.wav", 0).with_gain(-1.0).gain, MIN_GAIN);
        assert_eq!(step("/s/a.wav", 0).with_gain(1.5).gain, 1.5);
    }

    #[test]
    fn save_to_load_from_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("macros.json");

        let mut store = MacroStore::default();
        let id = store.add("Reverb riff").id.clone();
        let effects = EffectSettings {
            chain_bypass: false,
            ..EffectSettings::default()
        };
        store.replace_steps(
            &id,
            vec![Step {
                sound: PathBuf::from("/s/riff.wav"),
                start_offset_ms: 250,
                gain: 1.25,
                effects,
            }],
        );

        store.save_to(&path).unwrap();
        let loaded = MacroStore::load_from(&path);
        assert_eq!(
            loaded, store,
            "round-trip must preserve steps, gain, effects"
        );
    }

    #[test]
    fn load_from_missing_file_is_empty() {
        let dir = tempdir().unwrap();
        let loaded = MacroStore::load_from(&dir.path().join("nope.json"));
        assert_eq!(loaded, MacroStore::default());
    }

    #[test]
    fn load_from_corrupt_file_is_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrupt.json");
        std::fs::write(&path, b"} not json {").unwrap();
        assert_eq!(MacroStore::load_from(&path), MacroStore::default());
    }

    #[test]
    fn forward_compat_missing_fields_use_defaults() {
        // An older file: a step with only `sound` — no gain/offset/effects.
        let dir = tempdir().unwrap();
        let path = dir.path().join("old.json");
        let json = r#"[{"id":"abc","name":"Legacy","steps":[{"sound":"/s/a.wav"}]}]"#;
        std::fs::write(&path, json).unwrap();

        let loaded = MacroStore::load_from(&path);
        let step = &loaded.get("abc").unwrap().steps[0];
        assert_eq!(step.start_offset_ms, 0);
        assert_eq!(step.gain, 1.0);
        assert_eq!(step.effects, EffectSettings::default());
    }
}
