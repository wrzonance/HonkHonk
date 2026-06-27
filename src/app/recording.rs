use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::state::{Macro, Step};

use super::HonkHonk;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Recording {
    pub(crate) start: Instant,
    pub(crate) steps: Vec<Step>,
}

impl Recording {
    pub(crate) fn started_at(start: Instant) -> Self {
        Self {
            start,
            steps: Vec::new(),
        }
    }

    pub(crate) fn capture_at(&mut self, sound: &Path, now: Instant) {
        let elapsed = now.saturating_duration_since(self.start);
        self.steps.push(capture_step(sound.to_path_buf(), elapsed));
    }

    pub(crate) fn steps(&self) -> &[Step] {
        &self.steps
    }

    fn into_steps(self) -> Vec<Step> {
        self.steps
    }
}

pub(crate) fn capture_step(sound: PathBuf, elapsed: Duration) -> Step {
    Step::new(sound, elapsed_ms(elapsed))
}

pub(crate) fn draft_macro(number: u64, steps: Vec<Step>) -> Macro {
    Macro {
        id: format!("draft-macro-{number}"),
        name: format!("Macro {number}"),
        steps,
    }
}

fn elapsed_ms(elapsed: Duration) -> u64 {
    elapsed.as_millis().min(u128::from(u64::MAX)) as u64
}

impl HonkHonk {
    pub(crate) fn start_recording_at(&mut self, start: Instant) {
        self.recording = Some(Recording::started_at(start));
        self.macro_editor_draft = None;
    }

    pub(crate) fn stop_recording(&mut self) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        self.macro_draft_seq = self.macro_draft_seq.wrapping_add(1);
        let saved_count = self.macros.iter().count() as u64;
        let draft_number = saved_count.saturating_add(self.macro_draft_seq);
        self.macro_editor_draft = Some(draft_macro(draft_number, recording.into_steps()));
    }

    pub(crate) fn capture_recording_at(&mut self, sound: &Path, now: Instant) {
        if let Some(recording) = &mut self.recording {
            recording.capture_at(sound, now);
        }
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn recording_steps(&self) -> Option<&[Step]> {
        self.recording.as_ref().map(Recording::steps)
    }

    pub fn macro_editor_draft(&self) -> Option<&Macro> {
        self.macro_editor_draft.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::{Recording, capture_step, draft_macro};
    use crate::app::{HonkHonk, Message};
    use crate::audio::effects::EffectSettings;
    use crate::state::{AudioFormat, SoundEntry, Step};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

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

    #[test]
    fn capture_step_uses_elapsed_offset_and_defaults() {
        let path = PathBuf::from("/sounds/honk.wav");
        let step = capture_step(path.clone(), Duration::from_millis(42));

        assert_eq!(step.sound, path);
        assert_eq!(step.start_offset_ms, 42);
        assert_eq!(step.gain, 1.0);
        assert_eq!(step.effects, EffectSettings::default());
    }

    #[test]
    fn recording_appends_steps_in_capture_order() {
        let start = Instant::now();
        let mut recording = Recording::started_at(start);

        recording.capture_at(
            Path::new("/sounds/a.wav"),
            start + Duration::from_millis(10),
        );
        recording.capture_at(
            Path::new("/sounds/b.wav"),
            start + Duration::from_millis(42),
        );

        let steps = recording.steps();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].sound, PathBuf::from("/sounds/a.wav"));
        assert_eq!(steps[0].start_offset_ms, 10);
        assert_eq!(steps[1].sound, PathBuf::from("/sounds/b.wav"));
        assert_eq!(steps[1].start_offset_ms, 42);
    }

    #[test]
    fn rapid_capture_preserves_overlapping_steps() {
        let start = Instant::now();
        let mut recording = Recording::started_at(start);

        recording.capture_at(Path::new("/sounds/a.wav"), start);
        recording.capture_at(
            Path::new("/sounds/b.wav"),
            start + Duration::from_millis(30),
        );

        let steps = recording.steps();
        assert_eq!(steps.len(), 2);
        assert!(
            steps[0].start_offset_ms + 100 > steps[1].start_offset_ms,
            "100ms sounds fired 30ms apart overlap in the captured timeline"
        );
    }

    #[test]
    fn draft_macro_uses_auto_name_and_capture_order() {
        let steps = vec![
            Step::new(PathBuf::from("/sounds/a.wav"), 0),
            Step::new(PathBuf::from("/sounds/b.wav"), 120),
        ];

        let draft = draft_macro(3, steps.clone());

        assert_eq!(draft.id, "draft-macro-3");
        assert_eq!(draft.name, "Macro 3");
        assert_eq!(draft.steps, steps);
    }

    #[test]
    fn app_capture_is_noop_when_not_recording() {
        let mut app = HonkHonk::new_for_test();

        app.capture_recording_at(Path::new("/sounds/a.wav"), Instant::now());
        let _ = app.update(Message::StopRecording);

        assert!(!app.is_recording());
        assert!(app.recording_steps().is_none());
        assert!(app.macro_editor_draft().is_none());
    }

    #[test]
    fn starting_recording_resets_buffer_and_previous_draft() {
        let mut app = HonkHonk::new_for_test();
        let start = Instant::now();
        app.start_recording_at(start);
        app.capture_recording_at(
            Path::new("/sounds/a.wav"),
            start + Duration::from_millis(20),
        );
        app.stop_recording();
        assert!(app.macro_editor_draft().is_some());

        app.start_recording_at(start + Duration::from_secs(1));

        assert!(app.is_recording());
        assert_eq!(app.recording_steps().unwrap(), &[]);
        assert!(app.macro_editor_draft().is_none());
    }

    #[test]
    fn stopping_recording_produces_draft_with_steps() {
        let mut app = HonkHonk::new_for_test();
        let start = Instant::now();
        app.start_recording_at(start);
        app.capture_recording_at(Path::new("/sounds/a.wav"), start + Duration::from_millis(5));
        app.capture_recording_at(
            Path::new("/sounds/b.wav"),
            start + Duration::from_millis(40),
        );

        app.stop_recording();

        assert!(!app.is_recording());
        let draft = app.macro_editor_draft().expect("draft macro");
        assert_eq!(draft.name, "Macro 1");
        assert_eq!(draft.steps.len(), 2);
        assert_eq!(draft.steps[0].sound, PathBuf::from("/sounds/a.wav"));
        assert_eq!(draft.steps[1].sound, PathBuf::from("/sounds/b.wav"));
    }

    #[test]
    fn request_play_captures_step_and_queues_live_playback() {
        let mut app = HonkHonk::new_for_test();
        let sound = sound("honk", "/sounds/honk.wav");
        let start = Instant::now() - Duration::from_millis(15);
        app.start_recording_at(start);

        let _ = app.request_play(&sound, false);

        let steps = app.recording_steps().expect("recording steps");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].sound, PathBuf::from("/sounds/honk.wav"));
        assert!(steps[0].start_offset_ms >= 15);
        assert_eq!(app.playing(), None);
        assert_eq!(app.pending_play_ids.len(), 1);
    }

    #[test]
    fn slot_activation_captures_assigned_sound() {
        let mut app = HonkHonk::new_for_test();
        let sound = sound("slot", "/sounds/slot.wav");
        app.sounds = vec![sound.clone()];
        app.slots.set(0, sound.path.clone());
        app.start_recording_at(Instant::now() - Duration::from_millis(25));

        let _ = app.update(Message::ShortcutActivated(0));

        let steps = app.recording_steps().expect("recording steps");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].sound, sound.path);
        assert!(steps[0].start_offset_ms >= 25);
        assert_eq!(app.playing(), None);
        assert_eq!(app.pending_play_ids.len(), 1);
    }
}
