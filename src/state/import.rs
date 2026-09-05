use std::path::PathBuf;
use std::sync::Arc;

use super::SoundEntry;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0:#}")]
pub struct ImportError(pub Arc<anyhow::Error>);

impl PartialEq for ImportError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl From<anyhow::Error> for ImportError {
    fn from(error: anyhow::Error) -> Self {
        Self(Arc::new(error))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Analysis {
    pub duration_ms: u64,
    pub bytes: u64,
    pub peak: f32,
    pub leading_ms: u64,
    pub unnamed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportRow {
    pub source: PathBuf,
    pub name: String,
    pub category: String,
    pub color: u8,
    pub selected: bool,
    pub normalize: bool,
    pub trim: bool,
    pub analysis: Analysis,
    pub error: Option<ImportError>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanReport {
    pub rows: Vec<ImportRow>,
    pub errors: Vec<ImportError>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub source: PathBuf,
    pub sound: SoundEntry,
    pub name: String,
    pub color: u8,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportReport {
    pub imported: Vec<Imported>,
    pub failures: Vec<(PathBuf, ImportError)>,
}

mod scan;
mod storage;
pub mod transform;
pub use scan::scan;
pub use storage::{confirm, destination};

#[cfg(test)]
mod tests;
