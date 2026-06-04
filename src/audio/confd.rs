//! Persistent virtual-source `pipewire.conf.d` drop-in (issue #49).
//!
//! Pure, PipeWire-free logic: the canonical conf.d file contents, XDG path
//! resolution for the per-user drop-in, and a write-if-absent helper used by
//! the first-run fallback when no packaged conf.d is present.
//!
//! Packaged installs ship the same contents to
//! `/usr/share/pipewire/pipewire.conf.d/50-honkhonk.conf` (package-manager
//! territory). This module only ever writes the per-user copy under
//! `$XDG_CONFIG_HOME/pipewire/pipewire.conf.d/`.

use std::path::{Path, PathBuf};

use super::error::AudioError;

/// File name for the HonkHonk drop-in. `50-` orders it after PipeWire defaults.
pub const CONFD_FILE_NAME: &str = "50-honkhonk.conf";

/// Canonical drop-in contents. Byte-identical to
/// `packaging/pipewire/50-honkhonk.conf` (asserted in tests).
pub const CONFD_CONTENTS: &str = include_str!("../../packaging/pipewire/50-honkhonk.conf");

/// Resolve the per-user conf.d *directory*:
/// `$XDG_CONFIG_HOME/pipewire/pipewire.conf.d` (default `~/.config/...`).
pub fn user_confd_dir() -> Result<PathBuf, AudioError> {
    let base = directories::BaseDirs::new().ok_or(AudioError::ConfdNoConfigDir)?;
    Ok(base.config_dir().join("pipewire").join("pipewire.conf.d"))
}

/// Write the drop-in to `dir/CONFD_FILE_NAME` unless it already exists.
/// Returns `Ok(true)` if it wrote a new file, `Ok(false)` if one was already
/// present. Creates the directory tree as needed.
pub fn write_user_confd_in(dir: &Path) -> Result<bool, AudioError> {
    use std::io::Write;

    let path = dir.join(CONFD_FILE_NAME);
    std::fs::create_dir_all(dir).map_err(|source| AudioError::ConfdDirCreate {
        path: dir.display().to_string(),
        source,
    })?;
    // Atomic create-if-absent: `create_new(true)` fails with `AlreadyExists`
    // rather than racing a separate `exists()` check, so two concurrent
    // first-run launches can't both report a fresh write. `AlreadyExists` is
    // the idempotent path → `Ok(false)`.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            file.write_all(CONFD_CONTENTS.as_bytes())
                .map_err(|source| AudioError::ConfdWrite {
                    path: path.display().to_string(),
                    source,
                })?;
            Ok(true)
        }
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(source) => Err(AudioError::ConfdWrite {
            path: path.display().to_string(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confd_contents_declares_honkhonk_mic_source() {
        assert!(CONFD_CONTENTS.contains("node.name        = \"honkhonk-mic\""));
        assert!(CONFD_CONTENTS.contains("media.class      = Audio/Source/Virtual"));
    }

    #[test]
    fn confd_contents_sets_object_linger_true() {
        assert!(CONFD_CONTENTS.contains("object.linger     = true"));
    }

    #[test]
    fn confd_contents_uses_null_audio_sink_factory() {
        assert!(CONFD_CONTENTS.contains("factory.name     = support.null-audio-sink"));
    }

    #[test]
    fn user_confd_dir_ends_with_pipewire_confd() {
        // Uses real XDG; only assert the suffix shape, not the absolute prefix.
        let dir = user_confd_dir().expect("BaseDirs resolvable in test env");
        assert!(dir.ends_with("pipewire/pipewire.conf.d"));
    }

    #[test]
    fn write_user_confd_in_creates_file_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("pipewire/pipewire.conf.d");
        let wrote = write_user_confd_in(&target).unwrap();
        assert!(wrote);
        let written = std::fs::read_to_string(target.join(CONFD_FILE_NAME)).unwrap();
        assert_eq!(written, CONFD_CONTENTS);
    }

    #[test]
    fn write_user_confd_in_is_idempotent_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("pipewire/pipewire.conf.d");
        assert!(write_user_confd_in(&target).unwrap());
        // Second call: file exists → returns false, does not overwrite.
        assert!(!write_user_confd_in(&target).unwrap());
    }
}
