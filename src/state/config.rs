use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ui::theme::Theme;

mod persistence;
mod sort;
mod types;

pub use sort::SortPref;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Compact,
    #[default]
    Regular,
    Comfy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Renderer {
    #[default]
    Wgpu,
    TinySkia,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OverlapMode {
    #[default]
    Concurrent,
    Interrupt,
}

fn default_true() -> bool {
    true
}

fn default_level() -> f32 {
    1.0
}

const DEFAULT_VOLUME: f32 = 0.85;
const DEFAULT_WIDTH: u32 = 900;
const DEFAULT_HEIGHT: u32 = 600;
const SOUND_SUBDIR: &str = "HonkHonk";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub processing: crate::audio::processing::GlobalProcessing,
    pub sound_directories: Vec<PathBuf>,
    pub volume: f32,
    pub window_width: u32,
    pub window_height: u32,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub density: Density,
    #[serde(default = "default_true")]
    pub mic_passthrough: bool,
    #[serde(default = "default_level")]
    pub mic_passthrough_level: f32,
    #[serde(default)]
    pub renderer: Renderer,
    #[serde(default)]
    pub monitor_device: Option<String>,
    /// Selected microphone (input) device `node.name`; `None` = Auto (follow the
    /// system default, excluding HonkHonk's own virtual source).
    #[serde(default)]
    pub input_device: Option<String>,
    #[serde(default)]
    pub overlap_mode: OverlapMode,
    #[serde(default = "default_true")]
    pub panel_animations: bool,
    #[serde(default, deserialize_with = "sort::deserialize_sort_prefs")]
    pub sort_prefs: BTreeMap<String, SortPref>,
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
            processing: Default::default(),
            sound_directories,
            volume: DEFAULT_VOLUME,
            window_width: DEFAULT_WIDTH,
            window_height: DEFAULT_HEIGHT,
            theme: Theme::Dark,
            density: Density::Regular,
            mic_passthrough: default_true(),
            mic_passthrough_level: default_level(),
            renderer: Renderer::Wgpu,
            monitor_device: None,
            input_device: None,
            overlap_mode: OverlapMode::Concurrent,
            panel_animations: default_true(),
            sort_prefs: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests;
