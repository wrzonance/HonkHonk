//! Pure DSP helpers for formant-aware pitch shift (issue #35).
//!
//! Factored out of `formant.rs` to keep both files within the 400-line cap.
//! All items are `pub(super)` — visible to `formant.rs` (same parent module)
//! but not re-exported from the crate.
//!
//! # Const placement rationale
//! `WINDOW` and `BINS` are both defined here as `pub(super)` so that their
//! relationship (`BINS = WINDOW / 2 + 1`) lives in one place. `formant.rs`
//! imports them by name (`use super::formant_dsp::{.., BINS, WINDOW};`).
//! `MIN_RATIO` / `MAX_RATIO` and the semitone clamp consts are effect-level
//! concerns and stay in `formant.rs`.

use fundsp::prelude32::*;

/// FFT window length (power of two). 1024 samples ≈ 21.3 ms at 48 kHz, within
/// the <30 ms latency budget (issue #35). Also the effect's algorithmic latency.
pub(super) const WINDOW: usize = 1024;

/// Number of one-sided FFT bins for a `WINDOW`-point real FFT (DC..Nyquist).
pub(super) const BINS: usize = WINDOW / 2 + 1;

/// Symmetric moving-average half-width (in bins) for the formant-envelope
/// estimate. Wide enough to smooth past individual harmonics of a typical
/// voice fundamental (so the envelope tracks the formants, not the pitch), yet
/// narrow enough to keep the formant peaks. At 48 kHz / 1024-pt FFT each bin
/// ≈ 46.9 Hz, so 10 bins ≈ ±470 Hz — roughly four harmonics of a 110 Hz
/// fundamental either side, which flattens the harmonic ripple that would
/// otherwise leak pitch into the envelope and drift the formant centroid.
pub(super) const ENV_SMOOTH_BINS: usize = 10;

/// Floor to avoid divide-by-zero when flattening the spectrum by its envelope.
pub(super) const ENV_EPS: f32 = 1e-6;

/// The two independent resampling ratios for one recombine pass.
pub(super) struct Ratios {
    /// Excitation (and source phase) read position scales by `1 / pitch`.
    pub(super) pitch: f32,
    /// Envelope read position scales by `1 / formant`.
    pub(super) formant: f32,
}

/// Read per-bin magnitudes and phases from the input spectrum into `mag`/`phase`.
/// All slices share length `bins`.
pub(super) fn read_polar(fft: &mut FftWindow, mag: &mut [f32], phase: &mut [f32], bins: usize) {
    for i in 0..bins {
        let c = fft.at(0, i);
        mag[i] = c.norm();
        phase[i] = c.arg();
    }
}

/// Smooth `mag` into `env` with a symmetric moving average of half-width
/// `ENV_SMOOTH_BINS`, edge-clamped. Both slices have length `bins`.
pub(super) fn estimate_envelope(mag: &[f32], env: &mut [f32]) {
    let bins = mag.len();
    let w = ENV_SMOOTH_BINS as isize;
    for (i, slot) in env.iter_mut().enumerate() {
        let mut acc = 0.0f32;
        let mut count = 0.0f32;
        let mut k = -w;
        while k <= w {
            let j = (i as isize + k).clamp(0, bins as isize - 1) as usize;
            acc += mag[j];
            count += 1.0;
            k += 1;
        }
        *slot = acc / count;
    }
}

/// Linear-interpolated read of `src[pos]` for fractional `pos`, edge-clamped,
/// returning 0.0 if `pos` is outside `[0, len-1]` by more than clamping covers.
pub(super) fn lerp_read(src: &[f32], pos: f32) -> f32 {
    let len = src.len();
    if len == 0 || pos < 0.0 || pos > (len - 1) as f32 {
        return 0.0;
    }
    let i = pos.floor() as usize;
    let frac = pos - i as f32;
    if i + 1 < len {
        src[i] * (1.0 - frac) + src[i + 1] * frac
    } else {
        src[i]
    }
}

/// Recombine the shifted excitation with the resampled envelope and the source
/// phase, writing the output spectrum back into `fft`. The excitation is
/// resampled by `1/pitch`, the envelope independently by `1/formant`; the bin
/// count is taken from the equal-length `exc`/`env`/`phase` slices.
pub(super) fn recombine(fft: &mut FftWindow, exc: &[f32], env: &[f32], phase: &[f32], r: &Ratios) {
    debug_assert_eq!(exc.len(), env.len());
    debug_assert_eq!(exc.len(), phase.len());
    for i in 0..exc.len() {
        let src_pos = i as f32 / r.pitch;
        let exc_mag = lerp_read(exc, src_pos);
        let src_phase = lerp_read(phase, src_pos);
        let env_mag = lerp_read(env, i as f32 / r.formant);
        let out_mag = exc_mag * env_mag;
        fft.set(0, i, Complex32::from_polar(out_mag, src_phase));
    }
}

/// Shared signal-generation helpers used by both `formant` and `formant_dsp`
/// test modules. `pub(crate)` + `#[cfg(test)]` so they compile only in test
/// builds and are never part of the release binary.
#[cfg(test)]
pub(crate) mod test_signal {
    /// Generate `n` samples of a pure sine at `freq` Hz.
    pub(crate) fn make_sine(freq: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin())
            .collect()
    }

    /// Zero-crossing frequency estimate (fundamental only).
    pub(crate) fn dominant_freq_hz(samples: &[f32], sr: f32) -> f32 {
        let mut crossings = 0usize;
        for w in samples.windows(2) {
            if (w[0] <= 0.0 && w[1] > 0.0) || (w[0] >= 0.0 && w[1] < 0.0) {
                crossings += 1;
            }
        }
        (crossings as f32 / 2.0) * sr / samples.len() as f32
    }

    /// Crude spectral-centroid estimate via a bank of Goertzel magnitudes.
    /// Used as a proxy for "where the formant energy sits".
    pub(crate) fn spectral_centroid(samples: &[f32], sr: f32) -> f32 {
        let probes = [
            200.0f32, 500.0, 900.0, 1400.0, 2000.0, 2800.0, 3600.0, 4500.0,
        ];
        let mut num = 0.0f32;
        let mut den = 0.0f32;
        for &f in &probes {
            let omega = 2.0 * std::f32::consts::PI * f / sr;
            let coeff = 2.0 * omega.cos();
            let (mut s1, mut s2) = (0.0f32, 0.0f32);
            for &x in samples {
                let s0 = x + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            let mag = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt();
            num += f * mag;
            den += mag;
        }
        if den > 0.0 { num / den } else { 0.0 }
    }

    /// A two-formant synthetic "vowel": a buzz fundamental with energy
    /// concentrated around two resonances, built so its centroid is well-defined.
    pub(crate) fn vowel(f0: f32, sr: f32, n: usize) -> Vec<f32> {
        // Sum of harmonics weighted to peak near 700 Hz and 1200 Hz (an "ah").
        let formants = [700.0f32, 1200.0];
        let mut out = vec![0.0f32; n];
        let mut h = 1;
        loop {
            let fh = f0 * h as f32;
            if fh > sr / 2.0 {
                break;
            }
            let w: f32 = formants
                .iter()
                .map(|&fc| {
                    let bw = 120.0f32;
                    1.0 / (1.0 + ((fh - fc) / bw).powi(2))
                })
                .sum();
            for (i, s) in out.iter_mut().enumerate() {
                *s += w * (2.0 * std::f32::consts::PI * fh * i as f32 / sr).sin();
            }
            h += 1;
        }
        let peak = out.iter().fold(0.0f32, |m, &x| m.max(x.abs())).max(1e-9);
        for s in &mut out {
            *s /= peak;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_read_exact_integer_positions() {
        let src = [1.0f32, 3.0, 5.0];
        assert!((lerp_read(&src, 0.0) - 1.0).abs() < 1e-6);
        assert!((lerp_read(&src, 1.0) - 3.0).abs() < 1e-6);
        assert!((lerp_read(&src, 2.0) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_read_midpoint_interpolates() {
        let src = [0.0f32, 2.0];
        assert!((lerp_read(&src, 0.5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_read_out_of_range_returns_zero() {
        let src = [1.0f32, 2.0, 3.0];
        assert_eq!(lerp_read(&src, -1.0), 0.0);
        assert_eq!(lerp_read(&src, 10.0), 0.0);
        assert_eq!(lerp_read(&[], 0.0), 0.0);
    }

    #[test]
    fn estimate_envelope_flat_spectrum_is_unchanged() {
        // Flat input → envelope ≈ same value everywhere.
        let mag = vec![1.0f32; 64];
        let mut env = vec![0.0f32; 64];
        estimate_envelope(&mag, &mut env);
        for &v in &env {
            assert!((v - 1.0).abs() < 1e-5, "flat envelope: got {v}");
        }
    }

    #[test]
    fn estimate_envelope_smooths_spike() {
        // A single spike in the middle: the envelope should be lower than the spike.
        let mut mag = vec![0.0f32; 64];
        mag[32] = 100.0;
        let mut env = vec![0.0f32; 64];
        estimate_envelope(&mag, &mut env);
        // At the spike bin, the moving average spreads 100 over 2*ENV_SMOOTH_BINS+1 bins.
        let expected = 100.0 / (2 * ENV_SMOOTH_BINS + 1) as f32;
        assert!(
            (env[32] - expected).abs() < 1.0,
            "spike smoothed: env[32]={} expected≈{expected}",
            env[32]
        );
    }
}
