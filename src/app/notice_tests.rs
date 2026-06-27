use std::time::Instant;

use super::notices::{Notice, NoticeLevel};
use super::{HonkHonk, Message};
use crate::audio::{AudioEvent, EngineErrorEvent};

#[test]
fn source_first_run_written_queues_persistent_notice() {
    let mut app = HonkHonk::new_for_test();
    assert!(app.notices().is_empty());

    let _ = app.update(Message::AudioEvent(AudioEvent::SourceFirstRun {
        confd_written: true,
    }));

    let notice = app.notices().front().expect("notice set");
    assert_eq!(notice.notice.level, NoticeLevel::Info);
    assert!(notice.notice.body.contains("persist"));
    assert!(notice.notice.body.contains("HonkHonk Mic"));
}

#[test]
fn source_first_run_not_written_queues_session_notice() {
    let mut app = HonkHonk::new_for_test();

    let _ = app.update(Message::AudioEvent(AudioEvent::SourceFirstRun {
        confd_written: false,
    }));

    let notice = app.notices().front().expect("notice set");
    assert_eq!(notice.notice.level, NoticeLevel::Info);
    assert!(notice.notice.body.contains("this session"));
}

#[test]
fn raise_notice_message_queues_notice() {
    let mut app = HonkHonk::new_for_test();

    let _ = app.update(Message::RaiseNotice(Notice::warning(
        "Shortcut unavailable",
        "The portal is not running.",
    )));

    let notice = app.notices().front().expect("notice queued");
    assert_eq!(notice.notice.level, NoticeLevel::Warning);
    assert_eq!(notice.notice.title, "Shortcut unavailable");
}

#[test]
fn audio_error_event_queues_persistent_error_notice() {
    let mut app = HonkHonk::new_for_test();

    let _ = app.update(Message::AudioEvent(AudioEvent::Error(
        EngineErrorEvent::VirtualSinkNotRegistered,
    )));

    let notice = app.notices().front().expect("notice queued");
    assert_eq!(notice.notice.level, NoticeLevel::Error);
    assert_eq!(notice.notice.title, "Audio error");
    assert!(notice.notice.body.contains("virtual sink"));
    let id = notice.id;

    let _ = app.update(Message::NoticeTick(
        Instant::now() + Notice::DEFAULT_TIMEOUT * 4,
    ));
    assert!(app.notices().front().is_some());

    let _ = app.update(Message::DismissNotice(id));
    assert!(app.notices().is_empty());
}
