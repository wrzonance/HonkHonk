//! Boundary tests for slot activation/assignment dispatch, split from `mod.rs`
//! to keep it within the file-size budget. Pins the invariants from #169: a
//! sound slot activates through `request_play`, a macro slot activates
//! through `play_macro` regardless of step count, and every stale reference
//! (missing sound path, unknown macro id) self-clears without ever calling
//! the play path.

use super::{persist_slots_call_count, persist_slots_last_snapshot};
use crate::app::{HonkHonk, Message};
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
        modified_ms: None,
        category: "Test".into(),
    }
}

fn warm_pcm() -> Arc<CachedPcm> {
    Arc::new(CachedPcm {
        analysis: Default::default(),
        samples: Arc::new(vec![0.0_f32; 16]),
        sample_rate: 48_000,
        channels: 1,
        duration: Duration::from_millis(100),
    })
}

/// App pre-loaded with one library sound, no PCM cached — activation takes
/// the cold-miss path, which claims the now-playing highlight unconditionally
/// (independent of whether `self.audio` is wired, unlike the warm-hit path).
fn app_with_cold_sound() -> (HonkHonk, SoundEntry) {
    let mut app = HonkHonk::new_for_test();
    let s = sound("s1", "/s/a.wav");
    app.sounds = vec![s.clone()];
    (app, s)
}

/// App pre-loaded with a macro `id` whose one step fires a warm-cached
/// library sound at t=0 (macro-step dispatch checks `audio_store` directly).
fn app_with_warm_macro() -> (HonkHonk, String) {
    let mut app = HonkHonk::new_for_test();
    let s = sound("s1", "/s/a.wav");
    app.sounds = vec![s.clone()];
    app.audio_store.insert_pcm(s.id.clone(), warm_pcm());
    let id = app.macros.add("m").id.clone();
    app.macros.replace_steps(&id, vec![Step::new(s.path, 0)]);
    (app, id)
}

#[test]
fn activate_slot_never_panics_for_any_idx() {
    let mut app = HonkHonk::new_for_test();
    for idx in 0..=u8::MAX {
        let _ = app.update(Message::ShortcutActivated(idx));
    }
}

#[test]
fn activate_slot_on_empty_slot_is_noop() {
    let mut app = HonkHonk::new_for_test();
    let _ = app.update(Message::ShortcutActivated(5));
    assert!(app.playing().is_none());
    assert!(app.macro_playback.is_none());
    assert!(app.slots().content(5).is_none());
}

#[test]
fn sound_slot_activation_calls_request_play() {
    let (mut app, s) = app_with_cold_sound();
    app.slots.set(2, s.path.clone());

    let _ = app.update(Message::ShortcutActivated(2));

    assert_eq!(
        app.playing(),
        Some(s.id.as_str()),
        "request_play must claim the now-playing highlight"
    );
    assert_eq!(
        app.pending_play_ids.len(),
        1,
        "request_play's cold-miss path must queue exactly one decode"
    );
    // Assignment survives a successful activation.
    assert_eq!(app.slots().get(2), Some(&s.path));
}

#[test]
fn stale_sound_slot_self_clears_on_activation() {
    let mut app = HonkHonk::new_for_test();
    let missing = PathBuf::from("/gone/deleted.wav");
    app.slots.set(4, missing);

    let _ = app.update(Message::ShortcutActivated(4));

    assert!(
        app.slots().content(4).is_none(),
        "a slot pointing at a missing file must self-clear"
    );
    assert!(
        app.playing().is_none(),
        "a stale sound slot must never reach request_play"
    );
}

/// Pins the "clear and persist happen atomically" contract stated on
/// `clear_stale_slot`'s doc comment. `new_for_test()` hardcodes
/// `persist: false`, so the real disk write from `persist_slots` is a
/// guaranteed no-op here — `persist_slots_call_count` observes the call
/// itself, independent of that gate, so deleting the `self.persist_slots();`
/// line from `clear_stale_slot` fails this test (#169 review).
///
/// Also pins ordering, not just occurrence: `persist_slots_last_snapshot`
/// captures `self.slots` from *inside* the persist call, so if
/// `clear_stale_slot` ever persisted before mutating (or the two calls were
/// reordered), the snapshot would still show slot 4 assigned and this test
/// would fail even though a call did happen (#169 review).
#[test]
fn stale_sound_slot_clear_calls_persist_slots() {
    let mut app = HonkHonk::new_for_test();
    let missing = PathBuf::from("/gone/deleted.wav");
    app.slots.set(4, missing);
    let before = persist_slots_call_count();

    let _ = app.update(Message::ShortcutActivated(4));

    assert!(
        persist_slots_call_count() > before,
        "a stale sound slot's self-clear must call persist_slots so the \
         clear is written back to slots.json"
    );
    let snapshot = persist_slots_last_snapshot().expect("persist_slots was called");
    assert!(
        snapshot.content(4).is_none(),
        "the slot must already be cleared by the time persist_slots runs, \
         not persisted first and cleared after"
    );
}

#[test]
fn macro_slot_activation_fires_play_macro() {
    let (mut app, id) = app_with_warm_macro();
    app.slots.set_macro(6, id.clone()).unwrap();

    let _ = app.update(Message::ShortcutActivated(6));

    let run = app
        .macro_playback
        .as_ref()
        .expect("play_macro started a run");
    assert_eq!(run.macro_id, id);
    // Assignment survives a successful activation.
    assert_eq!(app.slots().macro_id(6), Some(id.as_str()));
}

#[test]
fn macro_slot_with_zero_steps_never_self_clears() {
    let mut app = HonkHonk::new_for_test();
    let id = app.macros.add("empty").id.clone();
    app.slots.set_macro(7, id.clone()).unwrap();

    let _ = app.update(Message::ShortcutActivated(7));

    assert_eq!(
        app.slots().macro_id(7),
        Some(id.as_str()),
        "an existing-but-zero-step macro is a valid authoring state and must not self-clear"
    );
    assert!(
        app.macro_playback.is_none(),
        "play_macro is a no-op for an empty macro, but that is not a stale reference"
    );
}

#[test]
fn stale_macro_slot_self_clears_without_calling_play_macro() {
    let mut app = HonkHonk::new_for_test();
    app.slots
        .set_macro(8, "does-not-exist".to_string())
        .unwrap();

    let _ = app.update(Message::ShortcutActivated(8));

    assert!(
        app.slots().content(8).is_none(),
        "a slot pointing at a deleted macro id must self-clear"
    );
    assert!(
        app.macro_playback.is_none(),
        "a stale macro slot must never dispatch into play_macro"
    );
}

/// Macro counterpart of `stale_sound_slot_clear_calls_persist_slots`: the
/// self-clear in `activate_macro_slot`'s stale branch must reach
/// `persist_slots` too, not just `activate_sound_slot`'s.
#[test]
fn stale_macro_slot_clear_calls_persist_slots() {
    let mut app = HonkHonk::new_for_test();
    app.slots
        .set_macro(8, "does-not-exist".to_string())
        .unwrap();
    let before = persist_slots_call_count();

    let _ = app.update(Message::ShortcutActivated(8));

    assert!(
        persist_slots_call_count() > before,
        "a stale macro slot's self-clear must call persist_slots so the \
         clear is written back to slots.json"
    );
}

#[test]
fn macro_activation_replaces_a_running_macro_via_play_macro() {
    // Proves activate_macro_slot dispatches through the real play_macro path
    // (its one-macro-at-a-time enforcement) rather than a parallel bypass.
    let (mut app, a) = app_with_warm_macro();
    let b = app.macros.add("other").id.clone();
    app.macros
        .replace_steps(&b, vec![Step::new(PathBuf::from("/s/a.wav"), 0)]);
    app.slots.set_macro(0, a.clone()).unwrap();
    app.slots.set_macro(1, b.clone()).unwrap();

    let _ = app.update(Message::ShortcutActivated(0));
    let run_a = app.macro_playback.as_ref().unwrap().run_id;
    let _ = app.update(Message::ShortcutActivated(1));
    let run_b = app.macro_playback.as_ref().unwrap();

    assert_eq!(run_b.macro_id, b, "one macro at a time: B replaces A");
    assert_ne!(run_b.run_id, run_a);
}

#[test]
fn assign_macro_slot_with_valid_id_persists_the_assignment() {
    let mut app = HonkHonk::new_for_test();
    let id = app.macros.add("valid").id.clone();

    let _ = app.update(Message::AssignMacroSlot(9, id.clone()));

    assert_eq!(app.slots().macro_id(9), Some(id.as_str()));
}

/// Pins the `Ok(()) => self.persist_slots()` arm of `assign_macro_slot`
/// directly: a successful assignment must call `persist_slots`, independent
/// of the `self.persist` gate that `new_for_test()` disables (#169 review).
#[test]
fn assign_macro_slot_with_valid_id_calls_persist_slots() {
    let mut app = HonkHonk::new_for_test();
    let id = app.macros.add("valid").id.clone();
    let before = persist_slots_call_count();

    let _ = app.update(Message::AssignMacroSlot(9, id));

    assert!(
        persist_slots_call_count() > before,
        "a successful macro-slot assignment must call persist_slots"
    );
}

/// Also pins the other half of the "persists if and only if `set_macro`
/// returns `Ok`" invariant: the `Err` arm in `assign_macro_slot` must not
/// call `persist_slots` at all, not merely leave the slot content unchanged
/// (#169 review).
#[test]
fn assign_macro_slot_with_invalid_id_does_not_mutate() {
    let mut app = HonkHonk::new_for_test();
    let path = PathBuf::from("/s/keep.wav");
    app.slots.set(10, path.clone());
    let before = persist_slots_call_count();

    // Empty macro id fails MacroIdError::Empty validation.
    let _ = app.update(Message::AssignMacroSlot(10, String::new()));

    assert_eq!(
        app.slots().get(10),
        Some(&path),
        "a rejected macro id must not overwrite the existing slot content"
    );
    assert_eq!(
        persist_slots_call_count(),
        before,
        "a rejected macro id must not call persist_slots at all"
    );
}
