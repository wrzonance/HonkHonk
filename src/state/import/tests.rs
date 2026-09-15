use super::*;
use std::fs;
use std::sync::atomic::AtomicBool;

fn wav(samples: &[i16]) -> Vec<u8> {
    let bytes = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + bytes).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&8000_u32.to_le_bytes());
    out.extend_from_slice(&16000_u32.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&bytes.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

fn fixture() -> (tempfile::TempDir, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let mut samples = vec![0; 1600];
    samples.extend(vec![32700; 800]);
    samples.extend(vec![0; 800]);
    let bytes = wav(&samples);
    fs::write(dir.path().join("untitled_1.wav"), &bytes).unwrap();
    (dir, bytes)
}

#[test]
fn scan_previews_warnings_and_leaves_sources_unchanged() {
    let (dir, original) = fixture();
    let report = scan(&[dir.path().into()], &AtomicBool::new(false));
    assert_eq!(report.rows.len(), 1);
    let row = &report.rows[0];
    assert_eq!(row.name, "untitled 1");
    assert_eq!(row.analysis.bytes, original.len() as u64);
    assert_eq!(row.analysis.duration_ms, 400);
    assert_eq!(row.analysis.leading_ms, 200);
    assert!(row.analysis.peak > 0.98);
    assert!(row.analysis.unnamed);
    assert_eq!(fs::read(&row.source).unwrap(), original);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn confirm_transforms_copies_and_preserves_sources() {
    let (source, original) = fixture();
    let destination = tempfile::tempdir().unwrap();
    let mut report = scan(&[source.path().into()], &AtomicBool::new(false));
    let row = &mut report.rows[0];
    row.category = "Memes".into();
    row.name = "Honk".into();
    row.normalize = true;
    row.trim = true;
    let first = confirm(&report.rows, destination.path());
    assert_eq!(first.failures.len(), 0);
    assert_eq!(first.imported.len(), 1);
    let path = &first.imported[0].sound.path;
    let decoded = crate::audio::decode(path).unwrap();
    assert_eq!(decoded.samples.len(), 800);
    assert!((decoded.samples[0] - 0.9).abs() < 0.001);
    assert_eq!(first.imported[0].sound.category, "Memes");
    assert_eq!(first.imported[0].name, "Honk");
    assert_eq!(
        fs::read(source.path().join("untitled_1.wav")).unwrap(),
        original
    );
}

#[test]
fn confirmation_never_overwrites_and_honors_exclusions() {
    let (source, _) = fixture();
    let destination = tempfile::tempdir().unwrap();
    let mut report = scan(&[source.path().into()], &AtomicBool::new(false));
    let first = confirm(&report.rows, destination.path());
    let path = &first.imported[0].sound.path;
    let copied = fs::read(path).unwrap();
    let second = confirm(&report.rows, destination.path());
    assert_ne!(second.imported[0].sound.path, *path);
    assert_eq!(fs::read(path).unwrap(), copied);
    report.rows[0].selected = false;
    assert!(
        confirm(&report.rows, destination.path())
            .imported
            .is_empty()
    );
}

#[test]
fn malformed_audio_is_a_visible_unselected_row_and_cancel_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.wav"), b"bad").unwrap();
    let report = scan(&[dir.path().into()], &AtomicBool::new(false));
    assert_eq!(report.rows.len(), 1);
    assert!(report.rows[0].error.is_some());
    assert!(!report.rows[0].selected);
    assert!(
        scan(&[dir.path().into()], &AtomicBool::new(true))
            .rows
            .is_empty()
    );
}

#[test]
fn invalid_category_cannot_escape_destination() {
    let (source, _) = fixture();
    let destination = tempfile::tempdir().unwrap();
    let mut report = scan(&[source.path().into()], &AtomicBool::new(false));
    report.rows[0].category = "../escape".into();
    let result = confirm(&report.rows, destination.path());
    assert!(result.imported.is_empty());
    assert_eq!(result.failures.len(), 1);
}

#[test]
fn copied_audio_and_color_metadata_round_trip_without_processing() {
    let (source, original) = fixture();
    let destination = tempfile::tempdir().unwrap();
    let rows = scan(&[source.path().into()], &AtomicBool::new(false)).rows;
    let imported = confirm(&rows, destination.path());
    assert_eq!(
        fs::read(&imported.imported[0].sound.path).unwrap(),
        original
    );
    let mut store = crate::state::SoundMetaStore::default();
    let meta = crate::state::SoundMeta {
        color: Some(5),
        ..Default::default()
    };
    store.set("sound".into(), meta.clone());
    let path = destination.path().join("meta.json");
    store.save_to(&path).unwrap();
    assert_eq!(
        crate::state::SoundMetaStore::load_from(&path).get("sound"),
        meta
    );
}

#[test]
fn bounded_decode_rejects_audio_over_sample_budget() {
    let (source, _) = fixture();
    assert!(matches!(
        crate::audio::decode_limited(&source.path().join("untitled_1.wav"), 16),
        Err(crate::audio::AudioError::SampleLimit)
    ));
}

#[test]
fn silent_and_stereo_processing_keeps_valid_frame_boundaries() {
    use crate::audio::DecodedAudio;
    let audio = DecodedAudio {
        repaired_channel: false,
        samples: vec![0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.0],
        sample_rate: 8000,
        channels: 2,
        duration: Default::default(),
    };
    let processed = transform::prepare(audio, false, true);
    assert_eq!(processed.samples, vec![0.0, 0.5, 0.5, 0.0]);
    let silent = DecodedAudio {
        repaired_channel: false,
        samples: vec![0.0; 80],
        sample_rate: 8000,
        channels: 1,
        duration: Default::default(),
    };
    let processed = transform::prepare(silent, true, true);
    assert_eq!(processed.samples, vec![0.0; 80]);
}

#[test]
fn symlink_category_is_rejected_and_missing_sources_report_errors() {
    let (source, _) = fixture();
    let destination = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let mut rows = scan(&[source.path().into()], &AtomicBool::new(false)).rows;
    rows[0].category = "link".into();
    std::os::unix::fs::symlink(outside.path(), destination.path().join("link")).unwrap();
    let report = confirm(&rows, destination.path());
    assert_eq!(report.failures.len(), 1);
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    let report = scan(&[source.path().join("missing")], &AtomicBool::new(false));
    assert!(!report.errors.is_empty());
}
