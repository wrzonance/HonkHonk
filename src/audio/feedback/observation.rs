use super::FeedbackDetector;
use crate::audio::voices::VoicePool;
use std::cell::Cell;

/// Coordinates monitor observations with soundboard playback and route polling.
pub(crate) struct FeedbackObservation {
    detector: FeedbackDetector,
    pending: Cell<bool>,
}

impl FeedbackObservation {
    pub fn new() -> Self {
        Self {
            detector: FeedbackDetector::new(48_000, 2),
            pending: Cell::new(false),
        }
    }

    pub fn observe(&mut self, samples: impl Iterator<Item = f32>, voices: &VoicePool) {
        if self.suspend_for_playback(voices) {
            return;
        }
        if self.detector.observe(samples) {
            self.pending.set(true);
        }
    }

    pub fn take_report(&mut self, voices: &VoicePool) -> bool {
        self.suspend_for_playback(voices);
        self.pending.replace(false)
    }

    fn suspend_for_playback(&mut self, voices: &VoicePool) -> bool {
        if voices.is_empty() {
            return false;
        }
        self.detector = FeedbackDetector::new(48_000, 2);
        self.pending.set(false);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::voices::VoiceSpec;
    use std::sync::Arc;

    fn play(pool: &mut VoicePool) {
        pool.push(VoiceSpec {
            processing: Default::default(),
            id: 1,
            sound_id: "clip".into(),
            generation: 1,
            samples: Arc::new(vec![0.7; 48_000]),
            sample_rate: 48_000,
            channels: 1,
            gain: 1.0,
            master_volume: 1.0,
            effects: Default::default(),
            monitor_enabled: true,
        });
    }

    #[test]
    fn own_voice_suppresses_reports_and_resets_observation_history() {
        let mut observation = FeedbackObservation::new();
        let mut voices = VoicePool::new();
        observation.observe(std::iter::repeat_n(0.7, 96 * 190), &voices);
        play(&mut voices);
        observation.observe(std::iter::repeat_n(0.7, 96 * 300), &voices);
        assert!(!observation.take_report(&voices));
        voices.stop_all();
        observation.observe(std::iter::repeat_n(0.7, 96 * 190), &voices);
        assert!(!observation.take_report(&voices));
        observation.observe(std::iter::repeat_n(0.7, 96 * 10), &voices);
        assert!(observation.take_report(&voices));
    }

    #[test]
    fn own_voice_clears_a_report_queued_before_playback_started() {
        let mut observation = FeedbackObservation::new();
        let mut voices = VoicePool::new();
        observation.observe(std::iter::repeat_n(0.7, 96 * 200), &voices);
        play(&mut voices);
        assert!(!observation.take_report(&voices));
        voices.stop_all();
        assert!(!observation.take_report(&voices));
    }
}
