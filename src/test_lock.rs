//! Serializes every test that builds an `iced_test::simulator`.
//!
//! `simulator()` reaches into iced's process-global font/text-shaping state,
//! which is not safe to drive from two threads at once. Built concurrently,
//! the test binary intermittently wedges: running
//! `cargo test --lib ui::slot_manager::grid_tests` (4 simulator tests, default
//! parallel harness) hung on 3 of 20 runs, while the same 20 runs under
//! `--test-threads=1` passed every time.
//!
//! Rather than force the whole suite single-threaded, every simulator-based
//! test holds this lock for its entire body — so at most one simulator exists
//! at a time and the other ~800 tests still run in parallel.

use std::sync::{Mutex, MutexGuard};

static GUI_LOCK: Mutex<()> = Mutex::new(());

/// Acquires the simulator lock for the remainder of the caller's scope.
///
/// Bind it (`let _gui = gui_lock();`) rather than dropping it immediately —
/// `let _ = gui_lock()` releases the guard at once and serializes nothing.
pub(crate) fn gui_lock() -> MutexGuard<'static, ()> {
    // The guarded value is `()`, so a panicking test leaves nothing corrupt
    // behind. Recover from poisoning instead of letting one failed test
    // cascade into every other simulator test.
    GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_reentrant_across_sequential_scopes() {
        drop(gui_lock());
        drop(gui_lock());
    }

    #[test]
    fn lock_recovers_from_poisoning() {
        let poisoner = std::thread::spawn(|| {
            let _guard = gui_lock();
            panic!("poison the lock");
        });
        assert!(
            poisoner.join().is_err(),
            "setup: thread should have panicked"
        );

        // Would panic instead of returning if poisoning were not handled.
        drop(gui_lock());
    }
}
