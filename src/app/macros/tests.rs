//! Controller tests for the macro playback engine, split from `mod.rs` to keep
//! it within the file-size budget. The pure scheduler has its own tests in
//! [`super::scheduler`].

use super::MACRO_VOICE_FLAG;
use crate::app::HonkHonk;
use crate::audio::CachedPcm;
use crate::state::{AudioFormat, SoundEntry, Step};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn sound(id: &str, path: &str) -> SoundEntry {
    SoundEntry {
        id: id.into(),
        name: id.to_uppercase(),
        path: path.into(),
        format: AudioFormat::Wav,
        duration_ms: Some(100),
        category: "Test".into(),
    }
}

fn warm_pcm() -> Arc<CachedPcm> {
    Arc::new(CachedPcm {
        samples: Arc::new(vec![0.0_f32; 16]),
        sample_rate: 48_000,
        channels: 1,
        duration: Duration::from_millis(100),
    })
}

/// App pre-loaded with one library sound (warm-cached) and a macro that fires it
/// once at t=0.
fn app_with_warm_macro() -> (HonkHonk, String) {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![sound("s1", "/s/a.wav")];
    app.audio_store.insert_pcm("s1".into(), warm_pcm());
    let id = app.macros.add("m").id.clone();
    app.macros
        .replace_steps(&id, vec![Step::new(PathBuf::from("/s/a.wav"), 0)]);
    (app, id)
}

#[test]
fn play_macro_starts_a_run_and_refire_restarts_it() {
    let (mut app, id) = app_with_warm_macro();
    let _ = app.play_macro(&id);
    let first = app.macro_playback.as_ref().expect("run started");
    assert_eq!(first.macro_id, id);
    let run1 = first.run_id;

    // Re-firing the same macro cancels and restarts (new run id).
    let _ = app.play_macro(&id);
    let run2 = app.macro_playback.as_ref().unwrap().run_id;
    assert_ne!(run1, run2, "re-fire must start a fresh run");
}

#[test]
fn playing_another_macro_replaces_the_first() {
    let (mut app, a) = app_with_warm_macro();
    let b = app.macros.add("other").id.clone();
    app.macros
        .replace_steps(&b, vec![Step::new(PathBuf::from("/s/a.wav"), 0)]);

    let _ = app.play_macro(&a);
    let _ = app.play_macro(&b);
    assert_eq!(
        app.macro_playback.as_ref().unwrap().macro_id,
        b,
        "one macro at a time: B replaces A"
    );
}

#[test]
fn empty_or_unknown_macro_starts_no_run() {
    let mut app = HonkHonk::new_for_test();
    let empty = app.macros.add("empty").id.clone();
    let _ = app.play_macro(&empty);
    assert!(app.macro_playback.is_none(), "empty macro: no run");
    let _ = app.play_macro("does-not-exist");
    assert!(app.macro_playback.is_none(), "unknown macro: no run");
}

#[test]
fn stale_step_due_after_cancel_is_ignored() {
    let (mut app, id) = app_with_warm_macro();
    let _ = app.play_macro(&id);
    let run_id = app.macro_playback.as_ref().unwrap().run_id;
    app.cancel_macro(); // run no longer current
    let _ = app.on_macro_step_due(run_id, 0);
    assert!(
        app.macro_playback.is_none(),
        "a due-message from a cancelled run must not dispatch"
    );
}

#[test]
fn step_due_for_superseded_run_does_not_dispatch_into_new_run() {
    let (mut app, a) = app_with_warm_macro();
    let _ = app.play_macro(&a);
    let stale_run = app.macro_playback.as_ref().unwrap().run_id;
    // A second macro supersedes the first; the old run's timer fires late.
    let _ = app.play_macro(&a);
    let _ = app.on_macro_step_due(stale_run, 0);
    assert!(
        app.macro_playback
            .as_ref()
            .unwrap()
            .active_voices()
            .is_empty(),
        "a stale run's step must not fire a voice into the current run"
    );
}

#[test]
fn warm_step_dispatches_then_finish_clears_the_run() {
    let (mut app, id) = app_with_warm_macro();
    let _ = app.play_macro(&id);
    let run_id = app.macro_playback.as_ref().unwrap().run_id;

    let _ = app.on_macro_step_due(run_id, 0);
    let voice_id = app
        .macro_playback
        .as_ref()
        .expect("run still active mid-playback")
        .active_voices()[0];

    // The voice's PlaybackFinished completes the single-step run exactly once.
    app.note_macro_voice_finished(voice_id);
    assert!(
        app.macro_playback.is_none(),
        "run clears once its last voice finishes"
    );
}

#[test]
fn macro_dispatch_does_not_touch_tile_generation() {
    let (mut app, id) = app_with_warm_macro();
    app.play_generation = 5; // a tile press is mid-flight at generation 5
    let _ = app.play_macro(&id);
    let run_id = app.macro_playback.as_ref().unwrap().run_id;
    let _ = app.on_macro_step_due(run_id, 0);

    assert_eq!(
        app.play_generation, 5,
        "a macro step must not advance the tile UI generation"
    );
    let voice_id = app.macro_playback.as_ref().unwrap().active_voices()[0];
    assert!(
        voice_id >= MACRO_VOICE_FLAG,
        "macro voices use the flagged id space, disjoint from tile voices"
    );
}

#[test]
fn noop_play_macro_does_not_cancel_a_running_macro() {
    let (mut app, a) = app_with_warm_macro();
    let _ = app.play_macro(&a);
    let run_id = app.macro_playback.as_ref().unwrap().run_id;

    // An unknown id and an empty macro are both no-ops — neither may stop A.
    let empty = app.macros.add("empty").id.clone();
    let _ = app.play_macro("does-not-exist");
    let _ = app.play_macro(&empty);

    let still = app
        .macro_playback
        .as_ref()
        .expect("A must still be running");
    assert_eq!(still.macro_id, a);
    assert_eq!(still.run_id, run_id, "a no-op request must not restart A");
}

#[test]
fn foreign_finished_voice_does_not_clear_run() {
    let (mut app, id) = app_with_warm_macro();
    let _ = app.play_macro(&id);
    let run_id = app.macro_playback.as_ref().unwrap().run_id;
    let _ = app.on_macro_step_due(run_id, 0);

    app.note_macro_voice_finished(999_999); // a tile press, not ours
    assert!(
        app.macro_playback.is_some(),
        "an unrelated voice finishing must not clear the macro run"
    );
}
