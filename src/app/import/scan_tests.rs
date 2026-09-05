use super::*;
use iced::futures::StreamExt;

async fn complete(task: Task<Message>) -> ImportMessage {
    let mut stream = iced_runtime::task::into_stream(task).unwrap();
    let action = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .unwrap();
    match action {
        iced_runtime::Action::Output(Message::Import(message)) => message,
        _ => panic!("expected import worker completion"),
    }
}

#[test]
fn burst_drops_start_one_worker_then_scan_all_accepted_sources() {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(async {
            let mut app = HonkHonk::new_for_test();
            let dir = tempfile::tempdir().unwrap();
            let paths: Vec<_> = (0..20)
                .map(|n| dir.path().join(format!("{n}.wav")))
                .collect();
            for path in &paths {
                std::fs::write(path, b"invalid audio still appears in review").unwrap();
            }
            let first = app.update_import(ImportMessage::Drop(paths[0].clone()));
            let mut work = first.units();
            for path in &paths[1..] {
                work += app.update_import(ImportMessage::Drop(path.clone())).units();
            }
            assert_eq!(work, 1, "burst must not fan out blocking scan workers");
            let message = complete(first).await;
            let next = app.update_import(message);
            assert_eq!(next.units(), 1);
            let message = complete(next).await;
            assert_eq!(app.update_import(message).units(), 0);
            let mut found: Vec<_> = app
                .import
                .report
                .rows
                .iter()
                .map(|r| r.source.clone())
                .collect();
            found.sort();
            let mut expected = paths;
            expected.sort();
            assert_eq!(found, expected);
            assert!(!app.import.scanning);
        });
}

#[test]
fn closing_and_reopening_waits_for_the_cancelled_worker() {
    let mut app = HonkHonk::new_for_test();
    let first = app.update_import(ImportMessage::Drop("/first.wav".into()));
    let epoch = app.import.epoch;
    assert_eq!(first.units(), 1);
    let _ = app.update_import(ImportMessage::Cancel);
    let next = app.update_import(ImportMessage::Drop("/second.wav".into()));
    assert_eq!(next.units(), 0);
    let next = app.update_import(ImportMessage::Scanned(epoch, ScanReport::default()));
    assert_eq!(next.units(), 1);
    assert_eq!(app.import.sources, vec![PathBuf::from("/second.wav")]);
    assert_eq!(
        app.update_import(ImportMessage::Scanned(epoch, ScanReport::default()))
            .units(),
        0
    );
}

#[test]
fn repeated_drops_bound_sources_and_show_a_limit() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Drop("/0.wav".into()));
    let first_epoch = app.import.epoch;
    for n in 1..1001 {
        let _ = app.update_import(ImportMessage::Drop(format!("/{n}.wav").into()));
    }
    assert_eq!(app.import.sources.len(), 1000);
    assert!(app.import.status.contains("limit"));
    assert!(app.import.status.contains("1000"));
    let _ = app.update_import(ImportMessage::Scanned(first_epoch, ScanReport::default()));
    let _ = app.update_import(ImportMessage::Scanned(
        app.import.epoch,
        ScanReport::default(),
    ));
    assert!(app.import.status.contains("limit"));
}

#[test]
fn cancelled_completion_drains_without_reopening_or_publishing() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update_import(ImportMessage::Drop("/first.wav".into()));
    let epoch = app.import.epoch;
    let cancel = app.import.cancel.clone();
    let _ = app.update_import(ImportMessage::Cancel);
    assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
    let task = app.update_import(ImportMessage::Scanned(epoch, ScanReport::default()));
    assert_eq!(task.units(), 0);
    assert!(!app.import.open);
    assert_eq!(
        app.update_import(ImportMessage::Drop("/second.wav".into()))
            .units(),
        1
    );
}
