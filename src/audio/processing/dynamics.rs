use super::DynamicsSettings;

/// Linked peak detector, 5 ms attack / 80 ms release, followed by an optional
/// instantaneous sample-peak ceiling. State belongs to one output stream.
pub struct Dynamics {
    envelope: f32,
}

impl Default for Dynamics {
    fn default() -> Self {
        Self { envelope: 0.0 }
    }
}

impl Dynamics {
    pub fn process(
        &mut self,
        samples: &mut [f32],
        format: (u32, u16),
        settings: DynamicsSettings,
        final_stage: bool,
    ) {
        let (rate, channels) = format;
        let settings = settings.sanitized();
        let attack = (-1.0 / (rate.max(1) as f32 * 0.005)).exp();
        let release = (-1.0 / (rate.max(1) as f32 * 0.080)).exp();
        for frame in samples.chunks_mut(channels.max(1) as usize) {
            for sample in frame.iter_mut() {
                if !sample.is_finite() {
                    *sample = 0.0;
                }
            }
            let peak = frame.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
            let coefficient = if peak > self.envelope {
                attack
            } else {
                release
            };
            self.envelope = coefficient * self.envelope + (1.0 - coefficient) * peak;
            let gain = self.gain(settings);
            let limit = if settings.enabled { 0.98 } else { 1.0 };
            let gain = if final_stage && peak > 0.0 {
                gain.min(limit / peak)
            } else {
                gain
            };
            for sample in frame {
                *sample *= gain;
                if final_stage {
                    // Roundoff in linked gain multiplication can exceed the
                    // ceiling by one ULP; keep the advertised bound exact.
                    *sample = sample.clamp(-limit, limit);
                }
            }
        }
    }

    fn gain(&self, settings: DynamicsSettings) -> f32 {
        if !settings.enabled || self.envelope <= 0.0 {
            return 1.0;
        }
        let above = (20.0 * self.envelope.log10() - settings.threshold_db).max(0.0);
        10.0_f32.powf(-above * (1.0 - 1.0 / settings.ratio) / 20.0)
    }
}
