//! Pure, time-injected macro scheduling and run bookkeeping. No clock, no
//! engine — so it tests without sleeping. The Iced `Task` plumbing that turns
//! these into real dispatches lives in the parent module.

use std::time::Duration;

use crate::state::Macro;

/// One scheduled dispatch: fire `step` (index into the macro) `delay` after the
/// macro starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledStep {
    pub delay: Duration,
    pub step: usize,
}

/// Build the dispatch schedule: every step at its own `start_offset_ms`, in
/// declaration order. Overlap is implicit — two steps at the same offset fire
/// together.
pub fn schedule(macro_def: &Macro) -> Vec<ScheduledStep> {
    macro_def
        .steps
        .iter()
        .enumerate()
        .map(|(step, s)| ScheduledStep {
            delay: Duration::from_millis(s.start_offset_ms),
            step,
        })
        .collect()
}

/// Tracks the single in-flight macro run. The app holds this as
/// `Option<MacroPlayback>`; that `Option` is what enforces one macro at a time
/// (starting a new run replaces it). `run_id` lets a stale `MacroStepDue` from a
/// cancelled run be ignored when it arrives late.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroPlayback {
    pub run_id: u64,
    pub macro_id: String,
    /// Voices still playing (assigned at dispatch, removed on `Finished`).
    voice_ids: Vec<u64>,
    steps_total: usize,
    /// Steps that reached a terminal outcome — a voice started, or a decode
    /// failed. Completion needs every step resolved, not just fired, so a failed
    /// step can't hang the run forever.
    steps_resolved: usize,
}

impl MacroPlayback {
    pub fn new(run_id: u64, macro_id: String, steps_total: usize) -> Self {
        Self {
            run_id,
            macro_id,
            voice_ids: Vec::new(),
            steps_total,
            steps_resolved: 0,
        }
    }

    /// A step fired a voice: record it and count the step resolved.
    pub fn record_dispatch(&mut self, voice_id: u64) {
        self.voice_ids.push(voice_id);
        self.steps_resolved += 1;
    }

    /// A step produced no voice (e.g. decode failed / sound missing): count it
    /// resolved so the run can still complete.
    pub fn record_failed_step(&mut self) {
        self.steps_resolved += 1;
    }

    /// Remove a finished voice. Returns `true` once the run is fully complete
    /// (every step resolved and no voices left) — the caller then clears the
    /// `Option<MacroPlayback>`. A voice this run never owned is ignored.
    pub fn on_voice_finished(&mut self, voice_id: u64) -> bool {
        self.voice_ids.retain(|&v| v != voice_id);
        self.is_complete()
    }

    /// True when every step has resolved and no voice is still playing.
    pub fn is_complete(&self) -> bool {
        self.steps_resolved >= self.steps_total && self.voice_ids.is_empty()
    }

    /// The voices to `StopVoice` when cancelling this run.
    pub fn active_voices(&self) -> &[u64] {
        &self.voice_ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MacroStore, Step};
    use std::path::PathBuf;

    fn macro_with(offsets: &[u64]) -> Macro {
        let mut store = MacroStore::default();
        let id = store.add("m").id.clone();
        let steps = offsets
            .iter()
            .map(|&off| Step::new(PathBuf::from("/s/a.wav"), off))
            .collect();
        store.replace_steps(&id, steps);
        store.get(&id).unwrap().clone()
    }

    #[test]
    fn schedule_maps_each_step_to_its_offset_in_order() {
        let m = macro_with(&[0, 500, 250]);
        assert_eq!(
            schedule(&m),
            vec![
                ScheduledStep {
                    delay: Duration::from_millis(0),
                    step: 0
                },
                ScheduledStep {
                    delay: Duration::from_millis(500),
                    step: 1
                },
                ScheduledStep {
                    delay: Duration::from_millis(250),
                    step: 2
                },
            ]
        );
    }

    #[test]
    fn schedule_allows_overlap_at_same_offset() {
        let m = macro_with(&[100, 100]);
        let sched = schedule(&m);
        assert_eq!(sched.len(), 2);
        assert_eq!(sched[0].delay, sched[1].delay);
    }

    #[test]
    fn run_completes_only_after_all_steps_resolved_and_voices_done() {
        let mut run = MacroPlayback::new(1, "m".into(), 2);
        run.record_dispatch(10);
        assert!(!run.is_complete(), "1 of 2 steps dispatched");
        assert!(!run.on_voice_finished(10), "step 2 not dispatched yet");
        run.record_dispatch(11);
        assert!(!run.is_complete(), "voice 11 still playing");
        assert!(run.on_voice_finished(11), "all resolved, no voices left");
    }

    #[test]
    fn failed_step_still_lets_run_complete() {
        let mut run = MacroPlayback::new(1, "m".into(), 2);
        run.record_dispatch(10);
        run.record_failed_step(); // step 2 had no voice
        assert!(!run.is_complete(), "voice 10 still playing");
        assert!(run.on_voice_finished(10), "resolved 2/2, no voices");
    }

    #[test]
    fn unknown_finished_voice_is_ignored() {
        let mut run = MacroPlayback::new(1, "m".into(), 1);
        run.record_dispatch(10);
        assert!(
            !run.on_voice_finished(999),
            "foreign voice must not complete run"
        );
        assert_eq!(run.active_voices(), &[10]);
    }

    #[test]
    fn active_voices_lists_outstanding_for_cancellation() {
        let mut run = MacroPlayback::new(7, "m".into(), 3);
        run.record_dispatch(1);
        run.record_dispatch(2);
        run.on_voice_finished(1);
        assert_eq!(run.active_voices(), &[2]);
    }
}
