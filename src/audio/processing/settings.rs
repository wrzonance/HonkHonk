use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DynamicsSettings {
    pub enabled: bool,
    pub threshold_db: f32,
    pub ratio: f32,
}

impl Default for DynamicsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_db: -12.0,
            ratio: 4.0,
        }
    }
}

impl DynamicsSettings {
    pub fn sanitized(self) -> Self {
        Self {
            enabled: self.enabled,
            threshold_db: finite(self.threshold_db, -60.0, 0.0, -12.0),
            ratio: finite(self.ratio, 1.0, 20.0, 4.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalProcessing {
    pub normalize: bool,
    pub dynamics: DynamicsSettings,
}

impl Default for GlobalProcessing {
    fn default() -> Self {
        Self {
            normalize: true,
            dynamics: DynamicsSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputMode {
    #[default]
    Preserve,
    Mono,
    Stereo,
}

impl std::fmt::Display for OutputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Preserve => "Original",
            Self::Mono => "Force mono",
            Self::Stereo => "Force stereo",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundProcessing {
    pub normalize: Option<bool>,
    pub output: OutputMode,
    pub pan: f32,
    pub dynamics: DynamicsSettings,
}

impl Default for SoundProcessing {
    fn default() -> Self {
        Self {
            normalize: None,
            output: OutputMode::Preserve,
            pan: 0.0,
            dynamics: DynamicsSettings {
                enabled: false,
                ..Default::default()
            },
        }
    }
}

impl SoundProcessing {
    pub fn sanitized(self) -> Self {
        Self {
            pan: finite(self.pan, -1.0, 1.0, 0.0),
            dynamics: self.dynamics.sanitized(),
            ..self
        }
    }
}

pub(super) fn finite(value: f32, low: f32, high: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        fallback
    }
}
