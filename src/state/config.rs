use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::error::ConfigError;
use crate::ui::theme::Theme;

const DEFAULT_VOLUME: f32 = 0.85;
const DEFAULT_WIDTH: u32 = 900;
const DEFAULT_HEIGHT: u32 = 600;
const SOUND_SUBDIR: &str = "HonkHonk";
const CONFIG_DIR_NAME: &str = "honkhonk";
const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub sound_directories: Vec<PathBuf>,
    pub volume: f32,
    pub window_width: u32,
    pub window_height: u32,
    #[serde(default)]
    pub theme: Theme,
}

impl Default for AppConfig {
    fn default() -> Self {
        let sound_directories = directories::UserDirs::new()
            .and_then(|dirs| dirs.audio_dir().map(|p| p.join(SOUND_SUBDIR)))
            .or_else(|| {
                directories::BaseDirs::new().map(|b| b.home_dir().join("Music").join(SOUND_SUBDIR))
            })
            .into_iter()
            .collect();

        Self {
            sound_directories,
            volume: DEFAULT_VOLUME,
            window_width: DEFAULT_WIDTH,
            window_height: DEFAULT_HEIGHT,
            theme: Theme::Dark,
        }
    }
}

impl AppConfig {
    /// Returns the config file path under XDG_CONFIG_HOME.
    fn config_path() -> Result<PathBuf, ConfigError> {
        let proj_dirs = directories::ProjectDirs::from("", "", CONFIG_DIR_NAME)
            .ok_or(ConfigError::NoConfigDir)?;
        Ok(proj_dirs.config_dir().join(CONFIG_FILE_NAME))
    }

    /// Loads config from disk, creating defaults if the file is missing.
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;

        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let config: Self =
            serde_json::from_str(&contents).map_err(|e| ConfigError::Deserialize {
                path: path.display().to_string(),
                source: e,
            })?;
        Ok(config)
    }

    /// Persists config to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;

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
        std::fs::write(&path, &json).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    /// Loads config from a specific path (for testing).
    pub fn load_from(path: &std::path::Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            let config = Self::default();
            config.save_to(path)?;
            return Ok(config);
        }

        let contents = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let config: Self =
            serde_json::from_str(&contents).map_err(|e| ConfigError::Deserialize {
                path: path.display().to_string(),
                source: e,
            })?;
        Ok(config)
    }

    /// Saves config to a specific path (for testing).
    pub fn save_to(&self, path: &std::path::Path) -> Result<(), ConfigError> {
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
        std::fs::write(path, &json).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_config_has_expected_values() {
        let config = AppConfig::default();
        assert_eq!(config.volume, 0.85);
        assert_eq!(config.window_width, 900);
        assert_eq!(config.window_height, 600);
    }

    #[test]
    fn round_trip_serialize_deserialize() {
        let config = AppConfig {
            sound_directories: vec![PathBuf::from("/tmp/sounds")],
            volume: 0.5,
            window_width: 1024,
            window_height: 768,
            theme: Theme::Dark,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config, deserialized);
    }

    #[test]
    fn save_and_load_from_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = AppConfig {
            sound_directories: vec![PathBuf::from("/home/user/sounds")],
            volume: 0.7,
            window_width: 800,
            window_height: 500,
            theme: Theme::Dark,
        };

        config.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path).unwrap();

        assert_eq!(config, loaded);
    }

    #[test]
    fn load_missing_creates_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir/config.json");

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded, AppConfig::default());
        assert!(path.exists());
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/c/config.json");

        let config = AppConfig::default();
        config.save_to(&path).unwrap();

        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("volume"));
    }
}
