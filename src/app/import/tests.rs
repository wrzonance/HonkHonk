use super::*;
use crate::state::import::{Analysis, ImportRow};

fn row() -> ImportRow {
    ImportRow {
        source: "/tmp/honk.wav".into(),
        name: "Honk".into(),
        category: "Test".into(),
        color: 0,
        selected: true,
        normalize: false,
        trim: false,
        analysis: Analysis::default(),
        error: None,
    }
}

#[test]
fn cancel_invalidates_scan_without_changing_library_or_config() {
    let mut app = HonkHonk::new_for_test();
    let sounds = app.sounds.clone();
    let directories = app.config.sound_directories.clone();
    let _ = app.update_import(ImportMessage::Open);
    assert!(app.import.open);
    let epoch = app.import.epoch;
    let _ = app.update_import(ImportMessage::Cancel);
    let _ = app.update_import(ImportMessage::Scanned(
        epoch,
        ScanReport {
            rows: vec![row()],
            errors: vec![],
        },
    ));
    assert!(!app.import.open);
    assert!(app.import.report.rows.is_empty());
    assert_eq!(app.sounds, sounds);
    assert_eq!(app.config.sound_directories, directories);
}

#[test]
fn batch_edits_only_apply_to_selected_successful_rows() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Open);
    app.import.report.rows = vec![row(), row()];
    app.import.report.rows[1].selected = false;
    let _ = app.update_import(ImportMessage::Normalize(true));
    let _ = app.update_import(ImportMessage::Trim(true));
    let _ = app.update_import(ImportMessage::BatchColor(3));
    assert!(app.import.report.rows[0].normalize);
    assert!(app.import.report.rows[0].trim);
    assert_eq!(app.import.report.rows[0].color, 3);
    assert!(!app.import.report.rows[1].normalize);
    assert_eq!(app.import.report.rows[1].color, 0);
}

#[test]
fn confirming_freezes_edits_and_duplicate_confirmation() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Open);
    app.import.report.rows = vec![row()];
    app.import.busy = true;
    let _ = app.update_import(ImportMessage::Cancel);
    let _ = app.update_import(ImportMessage::Name(0, "Changed".into()));
    assert!(app.import.open);
    assert_eq!(app.import.report.rows[0].name, "Honk");
}

#[test]
fn preview_identity_survives_closing_and_reopening() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Open);
    app.import.preview = 7;
    let _ = app.update_import(ImportMessage::Cancel);
    let _ = app.update_import(ImportMessage::Open);
    assert!(app.import.preview > 7);
}

#[test]
fn confirmed_import_publishes_metadata_once_and_preserves_excluded_rows() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Open);
    app.import.busy = true;
    app.import.report.rows = vec![row(), row()];
    app.import.report.rows[1].selected = false;
    let mut sound =
        crate::state::library::entry_from_path(std::path::Path::new("/tmp/imported/test.wav"))
            .unwrap();
    sound.category = "Memes".into();
    let id = sound.id.clone();
    let report = ImportReport {
        imported: vec![crate::state::import::Imported {
            source: row().source,
            sound,
            name: "Edited name".into(),
            color: 5,
        }],
        failures: vec![],
    };
    let message = ImportMessage::Confirmed(app.import.epoch, "/tmp/imported".into(), report);
    let before = app.sounds.len();
    let _ = app.update_import(message.clone());
    let _ = app.update_import(message);
    assert_eq!(app.sounds.len(), before + 1);
    assert_eq!(
        app.sound_meta.get(&id).display_name.as_deref(),
        Some("Edited name")
    );
    assert_eq!(app.sound_meta.get(&id).color, Some(5));
    assert_eq!(app.import.report.rows.len(), 1);
    assert!(!app.import.report.rows[0].selected);
}

#[test]
fn worker_failure_preserves_selected_rows_for_retry() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Open);
    app.import.busy = true;
    app.import.report.rows = vec![row()];
    let report = ImportReport {
        imported: vec![],
        failures: vec![(PathBuf::new(), anyhow::anyhow!("worker failed").into())],
    };
    let _ = app.update_import(ImportMessage::Confirmed(
        app.import.epoch,
        "/tmp/imported".into(),
        report,
    ));
    assert_eq!(app.import.report.rows.len(), 1);
    assert!(app.import.report.rows[0].selected);
}

#[test]
fn rescanning_preserves_edits_but_refreshes_analysis() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Open);
    app.import.report.rows = vec![row()];
    app.import.report.rows[0].name = "Edited".into();
    let _ = app.scan_import();
    let first_epoch = app.import.epoch;
    let _ = app.update_import(ImportMessage::Drop("/another.wav".into()));
    let next = app.update_import(ImportMessage::Scanned(first_epoch, ScanReport::default()));
    assert_eq!(next.units(), 1);
    let mut refreshed = row();
    refreshed.analysis.duration_ms = 1500;
    let report = ScanReport {
        rows: vec![refreshed],
        errors: vec![],
    };
    let _ = app.update_import(ImportMessage::Scanned(app.import.epoch, report));
    assert_eq!(app.import.report.rows[0].name, "Edited");
    assert_eq!(app.import.report.rows[0].analysis.duration_ms, 1500);
}

#[test]
fn macro_playback_invalidates_pending_import_preview() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Open);
    app.start_recording_at(std::time::Instant::now());
    app.capture_recording_at(std::path::Path::new("/a.wav"), std::time::Instant::now());
    let _ = app.update(Message::StopRecording);
    let _ = app.update(Message::ShowMacros);
    let id = app.macro_editor.active.clone().unwrap();
    let serial = app.import.preview;
    let _ = app.play_macro(&id);
    let status = app.import.status.clone();
    let error = anyhow::anyhow!("stale preview failure").into();
    let _ = app.update_import(ImportMessage::Previewed(
        serial,
        app.play_generation,
        Err(error),
    ));
    assert_eq!(app.import.status, status);
}
