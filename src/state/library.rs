use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use lofty::prelude::AudioFile;
use lofty::probe::Probe;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::state::error::ConfigError;

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "ogg", "flac", "wav", "aac", "m4a"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    Mp3,
    Ogg,
    Flac,
    Wav,
    Aac,
    Unknown,
}

impl AudioFormat {
    /// Determines audio format from a file extension string.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "mp3" => Self::Mp3,
            "ogg" => Self::Ogg,
            "flac" => Self::Flac,
            "wav" => Self::Wav,
            "aac" | "m4a" => Self::Aac,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub format: AudioFormat,
    pub duration_ms: Option<u64>,
    pub category: String,
}

/// Generates a deterministic hex ID from a file path.
fn path_to_id(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Checks whether a file extension is a supported audio format.
fn is_audio_extension(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS
        .iter()
        .any(|&supported| supported.eq_ignore_ascii_case(ext))
}

/// Builds a SoundEntry from a validated audio file path.
fn entry_from_path(path: &Path) -> Option<SoundEntry> {
    let ext = path.extension()?.to_str()?;
    if !is_audio_extension(ext) {
        return None;
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let category = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "General".into());

    Some(SoundEntry {
        id: path_to_id(path),
        name,
        path: path.to_path_buf(),
        format: AudioFormat::from_extension(ext),
        duration_ms: None,
        category,
    })
}

fn probe_duration(path: &Path) -> Option<u64> {
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    Some(tagged_file.properties().duration().as_millis() as u64)
}

pub fn probe_durations(pairs: Vec<(String, PathBuf)>) -> HashMap<String, u64> {
    pairs
        .into_iter()
        .filter_map(|(id, path)| probe_duration(&path).map(|ms| (id, ms)))
        .collect()
}

pub fn apply_durations(
    sounds: Vec<SoundEntry>,
    durations: &HashMap<String, u64>,
) -> Vec<SoundEntry> {
    sounds
        .into_iter()
        .map(|mut sound| {
            if let Some(&ms) = durations.get(&sound.id) {
                sound.duration_ms = Some(ms);
            }
            sound
        })
        .collect()
}

pub struct Library;

impl Library {
    /// Recursively scans directories for audio files and returns
    /// a list of SoundEntry items.
    pub fn scan(dirs: &[PathBuf]) -> Result<Vec<SoundEntry>, ConfigError> {
        let mut entries = Vec::new();

        for dir in dirs {
            if !dir.exists() {
                continue;
            }

            let walker = WalkDir::new(dir).follow_links(true);
            for result in walker {
                let Ok(dir_entry) = result else {
                    continue;
                };

                if !dir_entry.file_type().is_file() {
                    continue;
                }

                if let Some(entry) = entry_from_path(dir_entry.path()) {
                    entries.push(entry);
                }
            }
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_wav_1sec() -> Vec<u8> {
        let sample_rate: u32 = 8000;
        let num_samples: u32 = 8000;
        let data_size = num_samples;
        let mut v = Vec::with_capacity(44 + data_size as usize);
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36u32 + data_size).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1u16.to_le_bytes()); // mono
        v.extend_from_slice(&sample_rate.to_le_bytes());
        v.extend_from_slice(&sample_rate.to_le_bytes()); // byte_rate
        v.extend_from_slice(&1u16.to_le_bytes()); // block_align
        v.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_size.to_le_bytes());
        v.extend(vec![128u8; data_size as usize]);
        v
    }

    #[test]
    fn probe_duration_returns_some_for_valid_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wav");
        fs::write(&path, make_wav_1sec()).unwrap();
        let ms = probe_duration(&path).unwrap();
        assert!((900..=1100).contains(&ms), "expected ~1000ms, got {ms}");
    }

    #[test]
    fn probe_duration_returns_none_for_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        fs::write(&path, b"").unwrap();
        assert!(probe_duration(&path).is_none());
    }

    #[test]
    fn probe_duration_returns_none_for_non_audio_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("readme.txt");
        fs::write(&path, b"hello world").unwrap();
        assert!(probe_duration(&path).is_none());
    }

    #[test]
    fn format_from_extension_detects_all_types() {
        assert_eq!(AudioFormat::from_extension("mp3"), AudioFormat::Mp3);
        assert_eq!(AudioFormat::from_extension("MP3"), AudioFormat::Mp3);
        assert_eq!(AudioFormat::from_extension("ogg"), AudioFormat::Ogg);
        assert_eq!(AudioFormat::from_extension("flac"), AudioFormat::Flac);
        assert_eq!(AudioFormat::from_extension("wav"), AudioFormat::Wav);
        assert_eq!(AudioFormat::from_extension("aac"), AudioFormat::Aac);
        assert_eq!(AudioFormat::from_extension("m4a"), AudioFormat::Aac);
        assert_eq!(AudioFormat::from_extension("txt"), AudioFormat::Unknown);
    }

    #[test]
    fn path_to_id_is_deterministic() {
        let path = Path::new("/home/user/sounds/honk.mp3");
        let id1 = path_to_id(path);
        let id2 = path_to_id(path);
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn path_to_id_differs_for_different_paths() {
        let id1 = path_to_id(Path::new("/a/b.mp3"));
        let id2 = path_to_id(Path::new("/a/c.mp3"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn scan_finds_audio_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("honk.mp3"), b"fake mp3").unwrap();
        fs::write(dir.path().join("quack.ogg"), b"fake ogg").unwrap();
        fs::write(dir.path().join("boom.flac"), b"fake flac").unwrap();

        let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn scan_ignores_non_audio_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), b"not audio").unwrap();
        fs::write(dir.path().join("image.png"), b"not audio").unwrap();
        fs::write(dir.path().join("sound.mp3"), b"audio").unwrap();

        let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "sound");
        assert_eq!(entries[0].format, AudioFormat::Mp3);
    }

    #[test]
    fn scan_handles_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn scan_handles_nonexistent_directory() {
        let entries = Library::scan(&[PathBuf::from("/nonexistent/path/12345")]).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn scan_finds_files_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(dir.path().join("top.wav"), b"top").unwrap();
        fs::write(sub.join("nested.aac"), b"nested").unwrap();

        let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn scan_multiple_directories() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        fs::write(dir1.path().join("a.mp3"), b"a").unwrap();
        fs::write(dir2.path().join("b.flac"), b"b").unwrap();

        let entries =
            Library::scan(&[dir1.path().to_path_buf(), dir2.path().to_path_buf()]).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn entry_has_correct_fields() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("my_sound.wav");
        fs::write(&file_path, b"wav data").unwrap();

        let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.name, "my_sound");
        assert_eq!(entry.path, file_path);
        assert_eq!(entry.format, AudioFormat::Wav);
        assert!(entry.duration_ms.is_none());
        assert!(!entry.id.is_empty());
        assert!(!entry.category.is_empty());
    }

    #[test]
    fn scan_derives_category_from_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let memes = dir.path().join("Memes");
        fs::create_dir(&memes).unwrap();
        fs::write(memes.join("honk.mp3"), b"data").unwrap();

        let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, "Memes");
    }

    #[test]
    fn scan_file_at_scan_root_uses_dir_name_as_category() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("toplevel.mp3"), b"data").unwrap();

        let entries = Library::scan(&[dir.path().to_path_buf()]).unwrap();
        assert_eq!(entries.len(), 1);

        // A file at the root of the scan dir has no subdirectory parent,
        // so category falls back to the scan directory's own name.
        let expected = dir
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(entries[0].category, expected);
    }
}
