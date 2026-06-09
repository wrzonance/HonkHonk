//! Phase-vocoder pitch shift effect (issue #31, PR 2 of 6 of voice effects).
//!
//! Wraps the `pitch_shift` crate's [`Shifter`], which processes audio in fixed
//! 128-sample blocks and shifts pitch by a number of semitones (no time stretch
//! when `out_samples == 128`). There is no formant preservation — acceptable for
//! Tier 1 cartoon/dramatic effects.
//!
//! # Real-time safety
//! All buffers — the boxed `Shifter` state, the 128-sample input accumulator, and
//! the output FIFO ring — are pre-allocated in [`PitchShiftEffect::new`]. The
//! [`AudioEffect::process`] callback performs zero allocation, locking, or syscalls.
//!
//! # Block alignment
//! PipeWire delivers blocks of arbitrary length, but the shifter needs exactly
//! 128 input samples per call. We accumulate input into a 128-sample buffer; once
//! full, one `shift` call produces 128 output samples that are pushed into a fixed
//! ring FIFO. Each `process` call then drains `output.len()` samples from the FIFO.
//! Before the first full block is ready the FIFO underflows and we emit silence —
//! this is the algorithmic latency reported by [`AudioEffect::latency_samples`].

use super::preset::PitchPreset;
use super::AudioEffect;
use crate::audio::error::EffectsError;
use pitch_shift::{Shifter, TOTAL_F32};

/// The fixed block size the `pitch_shift` crate requires per `shift` call.
const BLOCK: usize = 128;

/// FFT window size of the phase vocoder; also its algorithmic latency in samples.
const FFT_WINDOW: usize = 1024;

/// Capacity of the output FIFO ring, in samples. Sized to comfortably hold one
/// large PipeWire quantum plus one freshly produced 128-sample block. Allocated
/// once in `new()`; never resized on the audio thread.
const OUT_RING_CAP: usize = 8192;

/// Minimum and maximum pitch shift in semitones. ±12 semitones == one octave ==
/// the 0.5x..2.0x pitch-factor range. Beyond this the phase vocoder produces
/// heavy artifacts, so the range is clamped (see issue #31 technical notes).
const MIN_SEMITONES: f32 = -12.0;
const MAX_SEMITONES: f32 = 12.0;

/// Minimum and maximum pitch factor, matching the semitone clamp range.
const MIN_FACTOR: f32 = 0.5;
const MAX_FACTOR: f32 = 2.0;

/// Heap-allocated state container for the `pitch_shift` [`Shifter`].
type ShifterState = Box<[f32; TOTAL_F32]>;

/// A pitch-shifting [`AudioEffect`] backed by a phase vocoder.
///
/// The source of truth for the shift amount is `semitones`; the pitch factor is a
/// derived view (`factor = 2^(semitones / 12)`).
pub struct PitchShiftEffect {
    shifter: Shifter<ShifterState>,
    sample_rate: f32,

    /// Current shift in semitones (clamped to `[MIN_SEMITONES, MAX_SEMITONES]`).
    semitones: f32,

    bypassed: bool,

    /// Accumulates input until a full 128-sample block is available.
    in_buf: [f32; BLOCK],
    in_fill: usize,

    /// Output FIFO ring holding pitch-shifted samples awaiting drain.
    out_ring: Box<[f32; OUT_RING_CAP]>,
    out_head: usize,
    out_len: usize,
}

impl PitchShiftEffect {
    /// Create a new pitch shift effect for the given `sample_rate`, with no shift
    /// applied (0 semitones) and not bypassed.
    ///
    /// Allocates the boxed shifter state and the output ring up front so the audio
    /// callback never allocates.
    pub fn new(sample_rate: u32) -> Self {
        let state_vec = vec![0.0_f32; TOTAL_F32];
        // Length is exactly TOTAL_F32 by construction, so the conversion to a
        // fixed-size boxed array cannot fail.
        let state_box: ShifterState = state_vec
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| Box::new([0.0_f32; TOTAL_F32]));

        Self {
            shifter: Shifter::new(state_box),
            sample_rate: sample_rate as f32,
            semitones: 0.0,
            bypassed: false,
            in_buf: [0.0; BLOCK],
            in_fill: 0,
            out_ring: Box::new([0.0; OUT_RING_CAP]),
            out_head: 0,
            out_len: 0,
        }
    }

    /// Current shift amount in semitones.
    pub fn semitones(&self) -> f32 {
        self.semitones
    }

    /// Current pitch factor (`2^(semitones / 12)`), in `[0.5, 2.0]`.
    pub fn pitch_factor(&self) -> f32 {
        semitones_to_factor(self.semitones)
    }

    /// Set the shift directly in semitones. Clamped to `[-12, 12]`.
    pub fn set_semitones(&mut self, semitones: f32) {
        self.semitones = semitones.clamp(MIN_SEMITONES, MAX_SEMITONES);
    }

    /// Set the shift as a pitch factor (`0.5`..`2.0`). Converted to semitones.
    pub fn set_pitch_factor(&mut self, factor: f32) {
        let clamped = factor.clamp(MIN_FACTOR, MAX_FACTOR);
        self.semitones = factor_to_semitones(clamped).clamp(MIN_SEMITONES, MAX_SEMITONES);
    }

    /// Apply a named [`PitchPreset`], overwriting the current shift amount.
    pub fn apply_preset(&mut self, preset: PitchPreset) {
        self.set_semitones(preset.semitones());
    }

    /// Run one 128-sample block through the shifter and append the result to the
    /// output ring FIFO. Real-time safe: the returned slice borrows the shifter's
    /// internal state, so we copy it into a fixed stack buffer (no heap) before
    /// pushing to the ring, which releases the borrow on `self.shifter`.
    fn process_block(&mut self) {
        let mut block = [0.0_f32; BLOCK];
        {
            let shifted = self
                .shifter
                .shift(&self.in_buf, self.semitones, BLOCK, self.sample_rate);
            block.copy_from_slice(shifted);
        }
        for &s in &block {
            self.ring_push(s);
        }
    }

    /// Push one sample onto the tail of the output ring. On overflow (should not
    /// happen for sane quanta), drops the oldest sample to stay allocation-free.
    fn ring_push(&mut self, sample: f32) {
        if self.out_len == OUT_RING_CAP {
            // Drop oldest: advance head, keep length at capacity.
            self.out_head = (self.out_head + 1) % OUT_RING_CAP;
            self.out_len -= 1;
        }
        let tail = (self.out_head + self.out_len) % OUT_RING_CAP;
        self.out_ring[tail] = sample;
        self.out_len += 1;
    }

    /// Pop one sample from the head of the output ring, or `0.0` if empty
    /// (startup latency / underflow).
    fn ring_pop(&mut self) -> f32 {
        if self.out_len == 0 {
            return 0.0;
        }
        let sample = self.out_ring[self.out_head];
        self.out_head = (self.out_head + 1) % OUT_RING_CAP;
        self.out_len -= 1;
        sample
    }
}

impl AudioEffect for PitchShiftEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        // Re-sync sample rate if the graph reconfigured (cheap, no alloc).
        let sr = sample_rate as f32;
        if (sr - self.sample_rate).abs() > f32::EPSILON {
            self.sample_rate = sr;
        }

        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }

        for (i, &sample) in input.iter().enumerate() {
            self.in_buf[self.in_fill] = sample;
            self.in_fill += 1;
            if self.in_fill == BLOCK {
                self.in_fill = 0;
                self.process_block();
            }
            output[i] = self.ring_pop();
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "semitones" => {
                self.set_semitones(value);
                Ok(())
            }
            "pitch_factor" => {
                self.set_pitch_factor(value);
                Ok(())
            }
            other => Err(EffectsError::ParamUnknown {
                param: other.to_owned(),
            }),
        }
    }

    fn bypass(&self) -> bool {
        self.bypassed
    }

    fn set_bypass(&mut self, bypass: bool) {
        self.bypassed = bypass;
    }

    fn latency_samples(&self) -> u32 {
        // Phase-vocoder FFT window latency. The 128-sample block-alignment buffer
        // overlaps with this window, so the dominant figure is the FFT window
        // (~21 ms at 48 kHz). See issue #31 technical notes.
        FFT_WINDOW as u32
    }
}

/// Convert semitones to a linear pitch factor: `2^(semitones / 12)`.
fn semitones_to_factor(semitones: f32) -> f32 {
    2.0_f32.powf(semitones / 12.0)
}

/// Convert a linear pitch factor to semitones: `12 * log2(factor)`.
fn factor_to_semitones(factor: f32) -> f32 {
    12.0 * factor.log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn effect() -> PitchShiftEffect {
        PitchShiftEffect::new(SR)
    }

    #[test]
    fn new_defaults_to_no_shift() {
        let fx = effect();
        assert_eq!(fx.semitones(), 0.0);
        assert!((fx.pitch_factor() - 1.0).abs() < 1e-6);
        assert!(!fx.bypass());
    }

    #[test]
    fn set_semitones_round_trips_to_factor() {
        let mut fx = effect();
        fx.set_semitones(12.0);
        assert!((fx.pitch_factor() - 2.0).abs() < 1e-5);
        fx.set_semitones(-12.0);
        assert!((fx.pitch_factor() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn set_pitch_factor_round_trips_to_semitones() {
        let mut fx = effect();
        fx.set_pitch_factor(2.0);
        assert!((fx.semitones() - 12.0).abs() < 1e-4);
        fx.set_pitch_factor(0.5);
        assert!((fx.semitones() + 12.0).abs() < 1e-4);
    }

    #[test]
    fn semitones_clamped_to_octave() {
        let mut fx = effect();
        fx.set_semitones(48.0);
        assert_eq!(fx.semitones(), MAX_SEMITONES);
        fx.set_semitones(-48.0);
        assert_eq!(fx.semitones(), MIN_SEMITONES);
    }

    #[test]
    fn pitch_factor_clamped_to_range() {
        let mut fx = effect();
        fx.set_pitch_factor(10.0);
        assert!((fx.pitch_factor() - MAX_FACTOR).abs() < 1e-5);
        fx.set_pitch_factor(0.01);
        assert!((fx.pitch_factor() - MIN_FACTOR).abs() < 1e-5);
    }

    #[test]
    fn set_param_semitones() {
        let mut fx = effect();
        assert!(fx.set_param("semitones", 7.0).is_ok());
        assert_eq!(fx.semitones(), 7.0);
    }

    #[test]
    fn set_param_pitch_factor() {
        let mut fx = effect();
        assert!(fx.set_param("pitch_factor", 1.5).is_ok());
        assert!((fx.pitch_factor() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn set_param_unknown_is_rejected() {
        let mut fx = effect();
        let err = fx.set_param("reverb", 0.5);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn bypass_toggles() {
        let mut fx = effect();
        assert!(!fx.bypass());
        fx.set_bypass(true);
        assert!(fx.bypass());
        fx.set_bypass(false);
        assert!(!fx.bypass());
    }

    #[test]
    fn bypass_is_exact_passthrough() {
        let mut fx = effect();
        fx.set_semitones(7.0);
        fx.set_bypass(true);
        let input: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut output = vec![0.0_f32; input.len()];
        fx.process(&input, &mut output, SR);
        assert_eq!(output, input);
    }

    #[test]
    fn latency_is_fft_window() {
        let fx = effect();
        assert_eq!(fx.latency_samples(), FFT_WINDOW as u32);
    }

    #[test]
    fn presets_apply_expected_semitones() {
        let mut fx = effect();
        fx.apply_preset(PitchPreset::Deep);
        assert_eq!(fx.semitones(), PitchPreset::Deep.semitones());
        fx.apply_preset(PitchPreset::Chipmunk);
        assert_eq!(fx.semitones(), PitchPreset::Chipmunk.semitones());
        fx.apply_preset(PitchPreset::Anonymous);
        assert_eq!(fx.semitones(), PitchPreset::Anonymous.semitones());
    }

    #[test]
    fn process_handles_block_unaligned_input() {
        // 200 is not a multiple of 128 — exercises the accumulator + ring drain.
        let mut fx = effect();
        fx.set_semitones(5.0);
        let input = vec![0.25_f32; 200];
        let mut output = vec![0.0_f32; 200];
        fx.process(&input, &mut output, SR);
        // No panic, output fully written, all values finite.
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn process_zero_shift_is_finite_and_length_stable() {
        let mut fx = effect();
        let input = vec![0.1_f32; 1024];
        let mut output = vec![0.0_f32; 1024];
        fx.process(&input, &mut output, SR);
        assert_eq!(output.len(), input.len());
        assert!(output.iter().all(|s| s.is_finite()));
    }

    /// Detect the dominant frequency of a mono signal via zero-crossing counting.
    /// Adequate for clean sine waves; returns Hz.
    fn dominant_freq_hz(samples: &[f32], sample_rate: f32) -> f32 {
        let mut crossings = 0usize;
        for w in samples.windows(2) {
            if (w[0] <= 0.0 && w[1] > 0.0) || (w[0] >= 0.0 && w[1] < 0.0) {
                crossings += 1;
            }
        }
        // Two zero crossings per cycle.
        let cycles = crossings as f32 / 2.0;
        cycles * sample_rate / samples.len() as f32
    }

    fn make_sine(freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn upward_shift_raises_output_frequency() {
        // Integration-style: feed a known sine, shift up an octave, verify the
        // output's dominant frequency is meaningfully higher than the input's.
        let sr = SR as f32;
        let in_freq = 440.0_f32;
        let n = 48_000; // 1 second — enough to flush the FFT window latency.
        let input = make_sine(in_freq, sr, n);

        let mut fx = effect();
        fx.set_semitones(12.0); // +1 octave == 2x frequency.
        let mut output = vec![0.0_f32; n];
        fx.process(&input, &mut output, SR);

        // Skip the latency/warm-up region at the front before measuring.
        let measured = dominant_freq_hz(&output[FFT_WINDOW * 4..], sr);
        let input_freq = dominant_freq_hz(&input[FFT_WINDOW * 4..], sr);

        assert!(
            measured > input_freq * 1.5,
            "expected upward shift: in≈{input_freq:.0}Hz out≈{measured:.0}Hz"
        );
    }

    #[test]
    fn downward_shift_lowers_output_frequency() {
        let sr = SR as f32;
        let in_freq = 440.0_f32;
        let n = 48_000;
        let input = make_sine(in_freq, sr, n);

        let mut fx = effect();
        fx.set_semitones(-12.0); // -1 octave == 0.5x frequency.
        let mut output = vec![0.0_f32; n];
        fx.process(&input, &mut output, SR);

        let measured = dominant_freq_hz(&output[FFT_WINDOW * 4..], sr);
        let input_freq = dominant_freq_hz(&input[FFT_WINDOW * 4..], sr);

        assert!(
            measured < input_freq * 0.75,
            "expected downward shift: in≈{input_freq:.0}Hz out≈{measured:.0}Hz"
        );
    }

    #[test]
    fn composes_inside_effect_chain() {
        use crate::audio::effects::EffectChain;

        // The effect must work as a `Box<dyn AudioEffect>` inside the foundation's
        // ping-pong `EffectChain` (issue #30) without panicking or producing NaNs.
        let block = 1024;
        let mut chain = EffectChain::new(block);
        let mut fx = effect();
        fx.set_semitones(7.0);
        chain.push_effect(Box::new(fx), block).unwrap();

        let input = make_sine(440.0, SR as f32, block);
        let mut output = vec![0.0_f32; block];
        chain.process(&input, &mut output, SR);

        assert_eq!(output.len(), input.len());
        assert!(output.iter().all(|s| s.is_finite()));
    }
}
