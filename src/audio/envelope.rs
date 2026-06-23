//! Peak-amplitude envelope of a decoded sound, for the now-playing waveform and
//! the future trim editor. Computed once per sound from the decoded PCM; stored
//! hi-res and downsampled for display (#138, PR-B). Pure — no audio I/O.

/// Hi-res bucket count stored per sound. Downsampled to the display bar count by
/// [`Envelope::bars`]; the future trim editor reads the full resolution.
pub const ENVELOPE_BUCKETS: usize = 1024;

/// Normalized peak-amplitude envelope; every value in `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    peaks: Vec<f32>,
}

impl Envelope {
    /// Builds an envelope from interleaved `samples` with `channels` lanes.
    /// Mono-sums each frame, splits the frames into `buckets` contiguous groups,
    /// takes the peak `|amplitude|` per group, then normalizes by the global
    /// peak so the tallest bar is `1.0`. Silent or empty input → all-zero peaks
    /// (no divide-by-zero).
    pub fn from_samples(samples: &[f32], channels: u16, buckets: usize) -> Self {
        let ch = channels as usize;
        if buckets == 0 || ch == 0 || samples.len() < ch {
            return Self {
                peaks: vec![0.0; buckets],
            };
        }
        let frames = samples.len() / ch;

        let mut peaks = vec![0.0_f32; buckets];
        for (frame_idx, frame) in samples.chunks_exact(ch).enumerate() {
            let mono = frame.iter().copied().sum::<f32>() / ch as f32;
            // u64 math avoids overflow on long inputs (frame_idx * buckets).
            let bucket = (frame_idx as u64 * buckets as u64 / frames as u64) as usize;
            let mag = mono.abs();
            if mag > peaks[bucket] {
                peaks[bucket] = mag;
            }
        }

        let max = peaks.iter().copied().fold(0.0_f32, f32::max);
        if max > f32::EPSILON {
            for p in &mut peaks {
                *p /= max;
            }
        }
        Self { peaks }
    }

    /// Max-pools the hi-res peaks down to `n` display bars. When `n >=
    /// peaks.len()` the peaks are returned padded with zeros to length `n`.
    /// Never panics; always returns exactly `n` values.
    pub fn bars(&self, n: usize) -> Vec<f32> {
        if n == 0 {
            return Vec::new();
        }
        let len = self.peaks.len();
        if len == 0 {
            return vec![0.0; n];
        }
        if n >= len {
            let mut out = self.peaks.clone();
            out.resize(n, 0.0);
            return out;
        }
        (0..n)
            .map(|i| {
                let start = i * len / n;
                let end = (((i + 1) * len / n).max(start + 1)).min(len);
                self.peaks[start..end]
                    .iter()
                    .copied()
                    .fold(0.0_f32, f32::max)
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn peaks(&self) -> &[f32] {
        &self.peaks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_amplitude_is_flat_after_normalize() {
        let samples = vec![0.5_f32; 4096];
        let env = Envelope::from_samples(&samples, 1, 64);
        assert_eq!(env.peaks().len(), 64);
        for p in env.peaks() {
            assert!((p - 1.0).abs() < 1e-6, "got {p}");
        }
    }

    #[test]
    fn silence_is_all_zero() {
        let env = Envelope::from_samples(&vec![0.0_f32; 4096], 1, 64);
        assert!(env.peaks().iter().all(|&p| p == 0.0));
    }

    #[test]
    fn empty_input_is_zero_filled() {
        let env = Envelope::from_samples(&[], 2, 32);
        assert_eq!(env.peaks().len(), 32);
        assert!(env.peaks().iter().all(|&p| p == 0.0));
    }

    #[test]
    fn half_and_full_regions_preserve_ratio() {
        let mut samples = vec![0.25_f32; 2048];
        samples.extend(vec![0.5_f32; 2048]);
        let env = Envelope::from_samples(&samples, 1, 2);
        assert!((env.peaks()[0] - 0.5).abs() < 1e-6, "got {}", env.peaks()[0]);
        assert!((env.peaks()[1] - 1.0).abs() < 1e-6, "got {}", env.peaks()[1]);
    }

    #[test]
    fn stereo_is_mono_summed_not_concatenated() {
        // L full, R silent → mono 0.5 everywhere → normalized 1.0; length == frames.
        let mut samples = Vec::new();
        for _ in 0..2048 {
            samples.push(1.0_f32);
            samples.push(0.0_f32);
        }
        let env = Envelope::from_samples(&samples, 2, 16);
        assert_eq!(env.peaks().len(), 16);
        for p in env.peaks() {
            assert!((p - 1.0).abs() < 1e-6, "got {p}");
        }
    }

    #[test]
    fn bars_max_pools_a_loud_region() {
        let mut samples = vec![1.0_f32; 1024]; // loud first quarter
        samples.extend(vec![0.0_f32; 3072]); // silent rest
        let env = Envelope::from_samples(&samples, 1, 64);
        let bars = env.bars(4);
        assert_eq!(bars.len(), 4);
        assert!((bars[0] - 1.0).abs() < 1e-6, "loud bar: {}", bars[0]);
        assert!(bars[3] < 1e-6, "silent bar: {}", bars[3]);
    }

    #[test]
    fn bars_pads_when_n_exceeds_resolution() {
        let env = Envelope::from_samples(&vec![0.5; 100], 1, 4);
        assert_eq!(env.bars(10).len(), 10);
    }

    #[test]
    fn bars_zero_n_is_empty() {
        let env = Envelope::from_samples(&vec![0.5; 100], 1, 4);
        assert!(env.bars(0).is_empty());
    }
}
