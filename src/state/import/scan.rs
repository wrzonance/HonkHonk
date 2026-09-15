use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::{Analysis, ImportError, ImportRow, ScanReport, transform};
use anyhow::Context;

pub fn scan(paths: &[PathBuf], cancel: &AtomicBool) -> ScanReport {
    let mut report = ScanReport::default();
    let mut seen = HashSet::new();
    let entries = paths
        .iter()
        .flat_map(|path| walkdir::WalkDir::new(path).follow_links(false));
    for (index, entry) in entries.enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if index >= 10000 || report.rows.len() >= 1000 {
            report.errors.push(
                anyhow::anyhow!(
                    "Scan limit reached (1000 sounds / 10000 entries); import smaller folders"
                )
                .into(),
            );
            break;
        }
        match entry {
            Ok(entry) if entry.file_type().is_file() => {
                if let Some(sound) = crate::state::library::entry_from_path(entry.path())
                    && seen.insert(sound.path.clone())
                {
                    report.rows.push(analyze(sound));
                }
            }
            Err(error) => report.errors.push(
                anyhow::Error::new(error)
                    .context("scanning import folder")
                    .into(),
            ),
            _ => {}
        }
    }
    report.rows.sort_by(|a, b| a.source.cmp(&b.source));
    report
}

fn analyze(sound: crate::state::SoundEntry) -> ImportRow {
    let name = sound
        .name
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut row = ImportRow {
        source: sound.path,
        name,
        category: sound.category,
        color: 0,
        selected: false,
        normalize: false,
        trim: false,
        analysis: Analysis::default(),
        error: None,
    };
    match analysis(&row.source, &row.name) {
        Ok(analysis) => {
            row.analysis = analysis;
            row.selected = true;
        }
        Err(error) => row.error = Some(error),
    }
    row
}

fn analysis(path: &Path, name: &str) -> Result<Analysis, ImportError> {
    let bytes = std::fs::metadata(path)
        .with_context(|| format!("reading {}", path.display()))?
        .len();
    let audio = transform::decode(path)?;
    let start = transform::audible_range(&audio).start;
    let lower = name.to_lowercase();
    Ok(Analysis {
        bytes,
        duration_ms: audio.duration.as_millis() as u64,
        peak: audio
            .samples
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs())),
        leading_ms: start as u64 * 1000 / u64::from(audio.channels) / u64::from(audio.sample_rate),
        unnamed: lower.is_empty()
            || ["untitled", "recording", "audio", "sound"]
                .iter()
                .any(|prefix| lower.starts_with(prefix)),
    })
}
