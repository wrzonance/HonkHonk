//! Monitor-only feedback detection; this does not process the routed mix.
//!
//! ADR-007 keeps mixing in PipeWire. Sample clamping is explicitly deferred:
//! detection plus asynchronous link removal is not a limiter and provides no
//! maximum-output guarantee. Loud legitimate content can also trigger these heuristics.
//! Observation is suspended during our own voice playback because the mixed monitor
//! cannot attribute those samples to a routed source. Graph prevention remains active.
mod monitor;
mod observation;
pub(crate) use monitor::FeedbackMonitor;
pub(crate) use observation::FeedbackObservation;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackSource {
    pub node_id: u32,
    pub name: String,
}

/// Fixed ten-millisecond windows keep timing independent of PipeWire quantum.
/// Only observes samples; never alters the monitor or routed output.
pub(crate) struct FeedbackDetector {
    window_samples: usize,
    samples: usize,
    energy: f64,
    history: [f64; 5],
    cursor: usize,
    sustained: usize,
    growth_windows: usize,
    cooldown: usize,
}

impl FeedbackDetector {
    pub fn new(rate: usize, channels: usize) -> Self {
        Self {
            window_samples: (rate / 100 * channels).max(1),
            samples: 0,
            energy: 0.0,
            history: [0.0; 5],
            cursor: 0,
            sustained: 0,
            growth_windows: 0,
            cooldown: 0,
        }
    }

    pub fn observe(&mut self, samples: impl Iterator<Item = f32>) -> bool {
        let mut detected = false;
        for sample in samples {
            if self.cooldown > 0 {
                self.cooldown -= 1;
                continue;
            }
            if !sample.is_finite() {
                self.trip();
                detected = true;
                continue;
            }
            self.energy += f64::from(sample).powi(2);
            self.samples += 1;
            if self.samples == self.window_samples {
                detected |= self.finish_window();
            }
        }
        detected
    }

    fn finish_window(&mut self) -> bool {
        let energy = self.energy / self.window_samples as f64;
        self.samples = 0;
        self.energy = 0.0;
        self.sustained = if energy > 10_f64.powf(-6.0 / 10.0) {
            self.sustained + 1
        } else {
            0
        };
        // Require three consecutive increases from an already-loud baseline.
        let previous = self.history[(self.cursor + self.history.len() - 1) % self.history.len()];
        self.growth_windows = if previous > 0.01 && energy > previous {
            self.growth_windows.saturating_add(1)
        } else {
            0
        };
        let growth = self.growth_windows >= 3
            && self
                .history
                .iter()
                .any(|old| *old > 0.01 && energy > old * 10_f64.powf(1.2));
        self.history[self.cursor] = energy;
        self.cursor = (self.cursor + 1) % self.history.len();
        if self.sustained >= 20 || growth {
            self.trip();
            return true;
        }
        false
    }

    fn trip(&mut self) {
        self.cooldown = self.window_samples * 200;
        self.sustained = 0;
        self.growth_windows = 0;
        self.history.fill(0.0);
        self.energy = 0.0;
        self.samples = 0;
    }
}

#[cfg(test)]
mod tests;
