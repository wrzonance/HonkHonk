# Formant-Aware Pitch Shifting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Tier-3 `FormantPitchEffect` that shifts pitch while preserving (or independently shifting) the spectral formant envelope, for natural-sounding voice transformation.

**Architecture:** FFT-domain processing via fundsp 0.23's `resynth` opcode (overlap-4 Hann-windowed STFT/IFFT). A per-window closure extracts the spectral envelope from the magnitude spectrum, divides it out to get the excitation, shifts excitation bins by an independent pitch ratio, then re-imposes the envelope scaled by an independent formant ratio. Runtime parameters reach the RT-thread closure through fundsp `Shared` atomics. The effect implements the existing `AudioEffect` trait and is fully self-contained — no `app.rs`/`engine.rs`/`mixer.rs` changes.

**Tech Stack:** Rust, fundsp 0.23 (`resynth`, `Shared`), the existing `AudioEffect` trait (`src/audio/effects/mod.rs`).

## Global Constraints

- **File size 400 lines max; functions ≤50 lines.** Split before adding.
- **clippy.toml:** cognitive-complexity 10 · too-many-arguments 5 · too-many-lines 50 · type-complexity 200. `cargo clippy --all-targets -- -D warnings` MUST pass clean. `cargo fmt`.
- **No `.unwrap()` / `panic!()` in non-test code.** Runs on the RT PipeWire thread — panics are fatal.
- **RT-safety:** no allocation, no locking, no syscalls inside `process` or the resynth closure. All buffers pre-allocated at construction.
- **Typed errors:** `set_param` returns `Result<(), EffectsError>`, `Err(EffectsError::ParamUnknown { param })` for unknown params. Match the existing effects exactly.
- **No new crates.** fundsp is already a dependency. Adding any crate trips the Flatpak `cargo-sources.json` freshness gate (#121) and is forbidden here. **`tdpsola` is AGPL — MUST NOT be added.**
- **TDD mandatory:** failing test first; 80% coverage target (`cargo tarpaulin`).
- **Latency budget:** <30 ms at 48 kHz (acceptance criterion). One FFT window = latency; window length must satisfy `window / 48000 < 0.030` → `window ≤ 1440` → use **1024** (≈21.3 ms), a power of two as `resynth` requires.

---

## File Structure

- **Create `src/audio/effects/formant.rs`** (~360 LOC incl. tests) — the `FormantPitchEffect` (`AudioEffect` impl) + the resynth closure builder + the spectral-envelope/pitch-shift DSP helpers + unit tests.
- **Create `src/audio/effects/formant_preset.rs`** (~110 LOC incl. tests) — the `FormantPreset` enum (Alien, GenderSwap, NaturalDeep) mapping each preset to `(pitch_ratio, formant_ratio)`.
- **Modify `src/audio/effects/mod.rs`** — add `pub mod formant;` / `pub mod formant_preset;` and re-export `FormantPitchEffect`, `FormantPreset`.

Splitting the preset enum into its own file mirrors the existing `pitch.rs` / `preset.rs` split and keeps both files comfortably under the 400-line cap.

---

## Background the implementer needs

### The `AudioEffect` trait (`src/audio/effects/mod.rs`)

```rust
pub trait AudioEffect: Send {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32);
    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError>;
    fn bypass(&self) -> bool;
    fn set_bypass(&mut self, bypass: bool);
    fn latency_samples(&self) -> u32;
}
```
`process`: `input` and `output` have equal length; arbitrary block length. RT-safe.

### fundsp `resynth` (verified in `fundsp-0.23.0/src/resynth.rs` + `prelude32.rs:2405`)

```rust
use fundsp::prelude32::*;            // brings in resynth, FftWindow, shared, Shared, An, AudioNode, U1
let synth: An<Resynth<U1, U1, _>> = resynth::<U1, U1, _>(1024, move |fft: &mut FftWindow| { /* ... */ });
```
`resynth::<I, O, F>(window_length, closure)` where the closure is `FnMut(&mut FftWindow) + Clone + Send + Sync`. The closure runs once per FFT window **on the RT thread** inside `An::tick`, so it must not allocate.

`FftWindow` methods used:
- `fft.bins() -> usize` — number of bins = `window/2 + 1`.
- `fft.frequency(i: usize) -> f32` — center frequency of bin `i` in Hz.
- `fft.at(channel: usize, i: usize) -> Complex32` — **input** spectrum bin (channel 0).
- `fft.set(channel: usize, i: usize, value: Complex32)` — write **output** spectrum bin (channel 0).
- The output starts cleared to zero each window; only bins you `set` are non-zero.

`Box<dyn AudioUnit>` (the dynamic node interface) — **VERIFIED against `fundsp-0.23.0/src/audiounit.rs`**:
- `trait AudioUnit: Send + Sync + DynClone` (line 21) — so a boxed node satisfies `AudioEffect: Send`.
- `fn tick(&mut self, input: &[f32], output: &mut [f32])` (line 39) — process **one sample**: `input`/`output` are 1-element slices for a `U1→U1` node. Call as `let mut out = [0.0f32; 1]; node.tick(&[x], &mut out);` then read `out[0]`.
- `fn set_sample_rate(&mut self, sr: f64)` (line 33) — push sample rate (affects `fft.frequency`).
- `fn reset(&mut self)` (line 24) — clear all window state (RT-safe; no alloc). Use on bypass transition.
- `An<X>` implements `AudioUnit` (line 367), so `Box::new(resynth::<U1,U1,_>(..))` coerces to `Box<dyn AudioUnit>`.

`num_complex::Complex32` (num-complex 0.4.6, **VERIFIED**) — `Complex32::new(re, im)`, fields `.re` / `.im`, `.norm()` (magnitude, line 217), `.arg()` (phase, line 222), `Complex32::from_polar(r, theta)` (line 233). `num_complex` is a resolved transitive dep; `use num_complex::Complex32;` compiles (the resynth module itself uses it). Do **not** add num-complex to `Cargo.toml` — using it through the existing graph introduces no Cargo.lock change.

### fundsp `Shared` (verified in `fundsp-0.23.0/src/shared.rs`)

```rust
let s: Shared = shared(1.0);   // Arc<AtomicU32>-backed f32, Send + Sync + Clone
s.set(2.0);                    // lock-free store (call from set_param / command thread)
let v: f32 = s.value();        // lock-free load (call inside RT closure)
```
Clone gives another handle to the same atomic. This is how the RT closure reads live `pitch_ratio` / `formant_ratio` without locking.

### Reference effect

`src/audio/effects/pitch.rs` is the Tier-1 A/B baseline (phase vocoder, **no** formant preservation). Read it for the house pattern: doc comment block explaining RT-safety, `bypassed` field, exact-passthrough on bypass, `reset` on bypass transition, `set_param` matching named params, latency reported in samples, and the zero-crossing `dominant_freq_hz` test helper (reused below).

### The DSP, precisely

Per FFT window, with `B = fft.bins()`:

1. **Read magnitudes + phases** into pre-allocated scratch arrays:
   `mag[i] = fft.at(0, i).norm()`, `phase[i] = fft.at(0, i).arg()`, for `i in 0..B`.
2. **Spectral envelope** `env[i]`: a smoothed version of `mag` that follows formant peaks but not individual harmonics. Use a symmetric moving average of half-width `ENV_SMOOTH_BINS` (clamped at edges). This is a cheap LPC-free envelope estimate adequate for voice.
3. **Excitation** `exc[i] = mag[i] / max(env[i], EPS)` — the source spectrum flattened of formant coloring.
4. **Pitch-shift the excitation** by `pitch_ratio` (`p`): the shifted excitation at bin `i` samples the source excitation at bin `i / p` (linear interpolation between neighbouring source bins; bins mapping outside `0..B` contribute 0). Carry the source phase from the same fractional source bin. This moves harmonics up/down without moving the envelope.
5. **Re-impose formants** with independent `formant_ratio` (`f`): the envelope applied at output bin `i` is sampled at `i / f` (linear interpolation, edge-clamped). `f == 1.0` keeps formants where they were (preserved); `f != 1.0` slides the vocal-tract resonances independently of pitch.
6. **Recombine:** `out_mag[i] = shifted_exc[i] * sampled_env[i]`; `out_phase[i] = shifted_exc_phase[i]`. Write `fft.set(0, i, Complex32::from_polar(out_mag[i], out_phase[i]))` (or `Complex32::new(out_mag*cos(phase), out_mag*sin(phase))`).

All scratch arrays (`mag`, `phase`, `env`, plus any temporaries) are `Vec<f32>` of length `B`, allocated once when the closure is built and **overwritten** each window — never grown. They are captured by move into the closure; cloning the closure (which `resynth` requires) clones the Vecs, but that happens only at construction, never on the RT thread.

**Ratios vs. semitones:** `pitch_ratio = 2^(semitones/12)`. Public `set_param` accepts both `"pitch_ratio"` (linear, clamp `[0.5, 2.0]`) and `"semitones"` (clamp `[-12, 12]`), plus `"formant_ratio"` / `"formant_shift"` (linear, clamp `[0.5, 2.0]`). Match the dual-unit convention `pitch.rs` already uses.

---

## Task 1: `FormantPreset` enum

**Files:**
- Create: `src/audio/effects/formant_preset.rs`
- Modify: `src/audio/effects/mod.rs`

**Interfaces:**
- Produces: `pub enum FormantPreset { Alien, GenderSwap, NaturalDeep }`; methods `pub fn pitch_ratio(self) -> f32`, `pub fn formant_ratio(self) -> f32`, `pub fn name(self) -> &'static str`, `pub fn all() -> [FormantPreset; 3]`.

Preset values (decided here; document in commit):
- **Alien** — formant shift only: `pitch_ratio = 1.0`, `formant_ratio = 1.6` (resonances pushed up, pitch unchanged → eerie, non-human timbre).
- **GenderSwap** — `pitch_ratio = 1.5` (+7 st up), `formant_ratio = 1.25` (raise vocal-tract resonances toward a higher-voiced character).
- **NaturalDeep** — pitch down, formants preserved: `pitch_ratio = 0.75` (−~5 st), `formant_ratio = 1.0` (no chipmunk/barrel because the envelope stays put).

- [ ] **Step 1: Write the failing tests**

Create `src/audio/effects/formant_preset.rs`:

```rust
//! Named presets for the [`FormantPitchEffect`](super::FormantPitchEffect).
//!
//! Each preset maps to an independent `(pitch_ratio, formant_ratio)` pair, both
//! linear multipliers (`1.0` = unchanged). Pitch moves harmonics; formant moves
//! the vocal-tract resonance envelope. Keeping them independent is what makes a
//! pitch change sound natural instead of "chipmunk"/"barrel".

/// A named, opinionated formant-pitch setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormantPreset {
    /// Formant shift only (pitch unchanged): eerie, non-human timbre.
    Alien,
    /// Pitch up + formants raised toward a higher-voiced character.
    GenderSwap,
    /// Pitch lowered with formants preserved — deep but natural, no barrel effect.
    NaturalDeep,
}

impl FormantPreset {
    /// Linear pitch multiplier (`1.0` = unchanged, `2.0` = +1 octave).
    pub fn pitch_ratio(self) -> f32 {
        match self {
            FormantPreset::Alien => 1.0,
            FormantPreset::GenderSwap => 1.5,
            FormantPreset::NaturalDeep => 0.75,
        }
    }

    /// Linear formant (envelope) multiplier (`1.0` = formants preserved).
    pub fn formant_ratio(self) -> f32 {
        match self {
            FormantPreset::Alien => 1.6,
            FormantPreset::GenderSwap => 1.25,
            FormantPreset::NaturalDeep => 1.0,
        }
    }

    /// Stable identifier string for config/UI.
    pub fn name(self) -> &'static str {
        match self {
            FormantPreset::Alien => "alien",
            FormantPreset::GenderSwap => "gender_swap",
            FormantPreset::NaturalDeep => "natural_deep",
        }
    }

    /// All presets, in display order.
    pub fn all() -> [FormantPreset; 3] {
        [
            FormantPreset::Alien,
            FormantPreset::GenderSwap,
            FormantPreset::NaturalDeep,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alien_shifts_formants_only() {
        assert_eq!(FormantPreset::Alien.pitch_ratio(), 1.0);
        assert!(FormantPreset::Alien.formant_ratio() > 1.0);
    }

    #[test]
    fn gender_swap_shifts_both() {
        assert!(FormantPreset::GenderSwap.pitch_ratio() > 1.0);
        assert!(FormantPreset::GenderSwap.formant_ratio() > 1.0);
    }

    #[test]
    fn natural_deep_lowers_pitch_preserves_formants() {
        assert!(FormantPreset::NaturalDeep.pitch_ratio() < 1.0);
        assert_eq!(FormantPreset::NaturalDeep.formant_ratio(), 1.0);
    }

    #[test]
    fn names_are_stable() {
        assert_eq!(FormantPreset::Alien.name(), "alien");
        assert_eq!(FormantPreset::GenderSwap.name(), "gender_swap");
        assert_eq!(FormantPreset::NaturalDeep.name(), "natural_deep");
    }

    #[test]
    fn all_returns_three_presets() {
        assert_eq!(FormantPreset::all().len(), 3);
    }
}
```

Add to `src/audio/effects/mod.rs` after the `pub mod filter;` / `pub mod flanger;` block (keep alphabetical-ish ordering consistent with the file):

```rust
pub mod formant;
pub mod formant_preset;
```
and in the re-export block:
```rust
pub use formant::FormantPitchEffect;
pub use formant_preset::FormantPreset;
```
(`formant.rs` does not exist yet — Task 1 only adds `formant_preset`; add the `pub mod formant;` and `pub use formant::FormantPitchEffect;` lines in **Task 2** so the crate compiles after each task. For Task 1, add only the `formant_preset` lines.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib formant_preset 2>&1 | tail -20`
Expected: FAIL — `error[E0583]: file not found for module` resolves after the file exists, then tests compile and pass; if you added the `mod` line before the file, it errors. Add `pub mod formant_preset;` + `pub use formant_preset::FormantPreset;` to `mod.rs` **with** the file present, then the tests run. Expected first real run: PASS (this is a pure data enum). If any assert is wrong, FAIL there.

- [ ] **Step 3: (implementation already written in Step 1 — pure data enum)**

No separate implementation step; the enum *is* the implementation.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib formant_preset 2>&1 | tail -20`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/audio/effects/formant_preset.rs src/audio/effects/mod.rs
git commit -m "feat(audio): add FormantPreset (alien, gender swap, natural deep)

Independent (pitch_ratio, formant_ratio) pairs for issue #35.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `FormantPitchEffect` scaffold — construct, bypass passthrough, latency

**Files:**
- Create: `src/audio/effects/formant.rs`
- Modify: `src/audio/effects/mod.rs` (add `pub mod formant;` + `pub use formant::FormantPitchEffect;`)

**Interfaces:**
- Consumes: `AudioEffect`, `EffectsError`, fundsp `resynth`/`Shared`, `FormantPreset` (Task 1).
- Produces:
  - `pub struct FormantPitchEffect` implementing `AudioEffect`.
  - `pub fn new(sample_rate: u32) -> Self` (matches `PitchShiftEffect::new(sample_rate)` convention).
  - `pub fn pitch_ratio(&self) -> f32`, `pub fn formant_ratio(&self) -> f32`, `pub fn set_pitch_ratio(&mut self, f32)`, `pub fn set_formant_ratio(&mut self, f32)`, `pub fn set_semitones(&mut self, f32)`, `pub fn apply_preset(&mut self, FormantPreset)`.
  - Module consts: `WINDOW: usize = 1024`, ratio clamps `MIN_RATIO = 0.5`, `MAX_RATIO = 2.0`, `MIN_SEMITONES = -12.0`, `MAX_SEMITONES = 12.0`.

This task delivers a *transparent* effect (closure copies input spectrum straight to output) so the scaffolding, bypass, and latency are tested before the DSP lands. The DSP fills in at Task 3 by replacing the closure body.

- [ ] **Step 1: Write the failing tests**

Create `src/audio/effects/formant.rs` with the struct + a *forward-only* closure and these tests:

```rust
//! Formant-aware pitch shift (issue #35, PR 6 of 6 of voice effects #18).
//!
//! Shifts pitch while independently controlling the spectral formant envelope,
//! so a voice can be lowered/raised without the "chipmunk" or "barrel" artefact
//! a naive resampling pitch shift produces. This is the Tier-3 quality tier; the
//! Tier-1 [`PitchShiftEffect`](super::PitchShiftEffect) is the A/B baseline.
//!
//! # Approach (fundsp `resynth`)
//! fundsp's `resynth` opcode runs an overlap-4 Hann-windowed STFT/IFFT and calls
//! a user closure once per FFT window with the input spectrum, expecting the
//! output spectrum back. Per window we: extract a smoothed spectral envelope
//! (the formants), divide it out to get the excitation, resample the excitation
//! by `pitch_ratio`, then re-impose the envelope resampled by an independent
//! `formant_ratio`. See the crate-level design notes for the maths.
//!
//! # Real-time safety
//! The fundsp node and every scratch buffer are allocated in [`Self::new`]. The
//! closure that runs on the PipeWire thread reads its live parameters through
//! lock-free [`Shared`] atomics and writes only into pre-sized buffers — no
//! allocation, locking, or syscalls. [`AudioEffect::process`] feeds samples one
//! at a time through the node's `tick`, which is allocation-free.

use super::formant_preset::FormantPreset;
use super::AudioEffect;
use crate::audio::error::EffectsError;
use fundsp::prelude32::*;
use num_complex::Complex32;

/// FFT window length (power of two). 1024 samples ≈ 21.3 ms at 48 kHz, within
/// the <30 ms latency budget (issue #35). Also the effect's algorithmic latency.
const WINDOW: usize = 1024;

/// Linear pitch/formant ratio clamp range (one octave each way).
const MIN_RATIO: f32 = 0.5;
const MAX_RATIO: f32 = 2.0;

/// Semitone clamp range, matching `MIN_RATIO..MAX_RATIO`.
const MIN_SEMITONES: f32 = -12.0;
const MAX_SEMITONES: f32 = 12.0;

/// The resynth node's concrete type is unnameable (it embeds the closure type),
/// so we erase it behind fundsp's dynamic `AudioUnit` interface. `AudioUnit` is
/// `Send`, satisfying `AudioEffect: Send`.
type Node = Box<dyn AudioUnit>;

/// Formant-aware pitch shift effect. See module docs.
pub struct FormantPitchEffect {
    node: Node,
    /// Live pitch multiplier read by the RT closure.
    pitch: Shared,
    /// Live formant multiplier read by the RT closure.
    formant: Shared,
    sample_rate: u32,
    bypassed: bool,
}

impl FormantPitchEffect {
    /// Create a formant-pitch effect at `sample_rate`, no shift (ratios = 1.0),
    /// not bypassed. Allocates the fundsp node and all scratch up front.
    pub fn new(sample_rate: u32) -> Self {
        let pitch = shared(1.0);
        let formant = shared(1.0);
        let node: Node = Box::new(build_node(WINDOW, pitch.clone(), formant.clone()));
        let mut effect = Self {
            node,
            pitch,
            formant,
            sample_rate,
            bypassed: false,
        };
        effect.node.set_sample_rate(f64::from(sample_rate));
        effect
    }

    /// Current linear pitch multiplier.
    pub fn pitch_ratio(&self) -> f32 {
        self.pitch.value()
    }

    /// Current linear formant multiplier.
    pub fn formant_ratio(&self) -> f32 {
        self.formant.value()
    }

    /// Set the pitch multiplier (clamped to `[0.5, 2.0]`).
    pub fn set_pitch_ratio(&mut self, ratio: f32) {
        self.pitch.set(ratio.clamp(MIN_RATIO, MAX_RATIO));
    }

    /// Set the formant multiplier (clamped to `[0.5, 2.0]`).
    pub fn set_formant_ratio(&mut self, ratio: f32) {
        self.formant.set(ratio.clamp(MIN_RATIO, MAX_RATIO));
    }

    /// Set pitch by semitones (clamped to `[-12, 12]`), converted to a ratio.
    pub fn set_semitones(&mut self, semitones: f32) {
        let st = semitones.clamp(MIN_SEMITONES, MAX_SEMITONES);
        self.pitch.set(2.0_f32.powf(st / 12.0));
    }

    /// Apply a [`FormantPreset`], overwriting both ratios.
    pub fn apply_preset(&mut self, preset: FormantPreset) {
        self.set_pitch_ratio(preset.pitch_ratio());
        self.set_formant_ratio(preset.formant_ratio());
    }
}

/// Build the fundsp resynth node. The closure runs on the RT thread; it captures
/// `pitch`/`formant` `Shared` handles (lock-free reads) and pre-sized scratch
/// buffers (overwritten per window, never grown).
///
/// In this scaffold the closure forwards the input spectrum unchanged; Task 3
/// replaces the body with the formant-pitch DSP.
fn build_node(window: usize, _pitch: Shared, _formant: Shared) -> impl AudioUnit {
    resynth::<U1, U1, _>(window, move |fft| {
        for i in 0..fft.bins() {
            fft.set(0, i, fft.at(0, i));
        }
    })
}

impl AudioEffect for FormantPitchEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());
        if sample_rate != self.sample_rate {
            self.node.set_sample_rate(f64::from(sample_rate));
            self.sample_rate = sample_rate;
        }
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        let mut frame_out = [0.0_f32; 1];
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            self.node.tick(&[i], &mut frame_out);
            *o = frame_out[0];
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "pitch_ratio" | "pitch_factor" => {
                self.set_pitch_ratio(value);
                Ok(())
            }
            "semitones" => {
                self.set_semitones(value);
                Ok(())
            }
            "formant_ratio" | "formant_shift" => {
                self.set_formant_ratio(value);
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
        // Flush FFT/overlap state on any transition so audio captured before a
        // bypass window cannot leak out after it (mirrors PitchShiftEffect).
        if self.bypassed != bypass {
            self.node.reset();
        }
        self.bypassed = bypass;
    }

    fn latency_samples(&self) -> u32 {
        // resynth latency == one window length (see resynth.rs `latency()`).
        WINDOW as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn effect() -> FormantPitchEffect {
        FormantPitchEffect::new(SR)
    }

    #[test]
    fn new_defaults_to_unity_ratios() {
        let fx = effect();
        assert!((fx.pitch_ratio() - 1.0).abs() < 1e-6);
        assert!((fx.formant_ratio() - 1.0).abs() < 1e-6);
        assert!(!fx.bypass());
    }

    #[test]
    fn set_ratios_clamp_to_octave() {
        let mut fx = effect();
        fx.set_pitch_ratio(10.0);
        assert!((fx.pitch_ratio() - MAX_RATIO).abs() < 1e-6);
        fx.set_pitch_ratio(0.01);
        assert!((fx.pitch_ratio() - MIN_RATIO).abs() < 1e-6);
        fx.set_formant_ratio(10.0);
        assert!((fx.formant_ratio() - MAX_RATIO).abs() < 1e-6);
    }

    #[test]
    fn semitones_convert_to_ratio() {
        let mut fx = effect();
        fx.set_semitones(12.0);
        assert!((fx.pitch_ratio() - 2.0).abs() < 1e-5);
        fx.set_semitones(-12.0);
        assert!((fx.pitch_ratio() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn set_param_known_and_unknown() {
        let mut fx = effect();
        assert!(fx.set_param("pitch_ratio", 1.5).is_ok());
        assert!(fx.set_param("formant_ratio", 1.2).is_ok());
        assert!(fx.set_param("semitones", 7.0).is_ok());
        let err = fx.set_param("reverb", 0.5);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
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
    fn latency_is_one_window() {
        assert_eq!(effect().latency_samples(), WINDOW as u32);
        // Budget check: < 30 ms at 48 kHz.
        assert!((WINDOW as f32 / SR as f32) < 0.030);
    }

    #[test]
    fn apply_preset_sets_both_ratios() {
        let mut fx = effect();
        fx.apply_preset(FormantPreset::GenderSwap);
        assert!((fx.pitch_ratio() - FormantPreset::GenderSwap.pitch_ratio()).abs() < 1e-6);
        assert!((fx.formant_ratio() - FormantPreset::GenderSwap.formant_ratio()).abs() < 1e-6);
    }

    #[test]
    fn process_output_is_finite_and_length_stable() {
        let mut fx = effect();
        fx.set_semitones(5.0);
        let input = vec![0.1_f32; 2048];
        let mut output = vec![0.0_f32; 2048];
        fx.process(&input, &mut output, SR);
        assert_eq!(output.len(), input.len());
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn composes_inside_effect_chain() {
        use crate::audio::effects::EffectChain;
        let block = 1024;
        let mut chain = EffectChain::new(block);
        let mut fx = effect();
        fx.set_semitones(7.0);
        chain.push_effect(Box::new(fx), block).unwrap();
        let input: Vec<f32> = (0..block)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / SR as f32).sin())
            .collect();
        let mut output = vec![0.0_f32; block];
        chain.process(&input, &mut output, SR);
        assert_eq!(output.len(), input.len());
        assert!(output.iter().all(|s| s.is_finite()));
    }
}
```

Add to `src/audio/effects/mod.rs`:
```rust
pub mod formant;        // alongside the existing pub mod lines
pub use formant::FormantPitchEffect;   // in the re-export block
```

**Implementer notes for this task:**
- `BINS` const is unused until Task 3 — **do not declare it in this task**; introduce it in Task 3 where it is first used, so clippy stays clean. (The Step-1 code above does not reference `BINS`.)
- The node type and `tick` signature are already resolved: `type Node = Box<dyn AudioUnit>`, and `process` uses the verified slice API `node.tick(&[i], &mut frame_out)` shown above. `Box::new(resynth::<U1,U1,_>(WINDOW, closure))` coerces to `Box<dyn AudioUnit>` because `An<X>: AudioUnit`. No exploration needed.

- [ ] **Step 2: Run tests to verify they fail (then compile-fix to green)**

Run: `cargo test --lib formant:: 2>&1 | tail -30`
Expected: first compile may FAIL on the `tick`/node-type detail above. Fix per the implementer note until it compiles, then all 9 tests PASS (the scaffold forwards the spectrum, so `process` is effectively a delayed passthrough; `bypass_is_exact_passthrough` and the finite/length tests pass; the frequency-shift behaviour is *not* asserted yet — that arrives in Task 3).

- [ ] **Step 3: (implementation written in Step 1)** — the scaffold node forwards the spectrum.

- [ ] **Step 4: Verify clippy + fmt + full test**

Run:
```bash
cargo fmt
cargo clippy --all-targets -- -D warnings 2>&1 | tail -20
cargo test --lib formant 2>&1 | tail -20
```
Expected: clippy clean, all `formant` + `formant_preset` tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/audio/effects/formant.rs src/audio/effects/mod.rs
git commit -m "feat(audio): scaffold FormantPitchEffect via fundsp resynth

AudioEffect impl, Shared-atomic params, bypass + latency. Closure forwards
the spectrum unchanged; DSP lands next. Issue #35.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: The formant-pitch DSP

**Files:**
- Modify: `src/audio/effects/formant.rs` (replace `build_node` closure body; add DSP helpers + tests)

**Interfaces:**
- Consumes: everything from Task 2.
- Produces: the real formant-preserving frequency transform. Public API unchanged.

This is the core. Replace the forward-only closure with the envelope/excitation/reimposition pipeline, using captured pre-sized scratch buffers.

- [ ] **Step 1: Write the failing tests** (append to the `tests` module)

Reuse the zero-crossing frequency estimator from `pitch.rs`:

```rust
    fn make_sine(freq: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin())
            .collect()
    }

    fn dominant_freq_hz(samples: &[f32], sr: f32) -> f32 {
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
    fn spectral_centroid(samples: &[f32], sr: f32) -> f32 {
        let probes = [200.0f32, 500.0, 900.0, 1400.0, 2000.0, 2800.0, 3600.0, 4500.0];
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
    fn vowel(f0: f32, sr: f32, n: usize) -> Vec<f32> {
        // Sum of harmonics weighted to peak near 700 Hz and 1200 Hz (an "ah").
        let formants = [700.0f32, 1200.0];
        let mut out = vec![0.0f32; n];
        let mut h = 1;
        loop {
            let fh = f0 * h as f32;
            if fh > sr / 2.0 { break; }
            // Weight = sum of resonance bumps.
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
        // Normalise.
        let peak = out.iter().fold(0.0f32, |m, &x| m.max(x.abs())).max(1e-9);
        for s in &mut out { *s /= peak; }
        out
    }

    const WARMUP: usize = WINDOW * 4; // skip latency + overlap fill before measuring

    #[test]
    fn pitch_up_raises_fundamental_frequency() {
        // pitch_ratio 2.0 with formants preserved: fundamental should roughly double.
        let sr = SR as f32;
        let n = SR as usize; // 1 s
        let input = make_sine(220.0, sr, n);
        let mut fx = effect();
        fx.set_pitch_ratio(2.0);
        fx.set_formant_ratio(1.0);
        let mut output = vec![0.0f32; n];
        fx.process(&input, &mut output, SR);
        let f_in = dominant_freq_hz(&input[WARMUP..], sr);
        let f_out = dominant_freq_hz(&output[WARMUP..], sr);
        assert!(
            f_out > f_in * 1.5,
            "expected pitch up: in≈{f_in:.0} out≈{f_out:.0}"
        );
    }

    #[test]
    fn formant_envelope_preserved_when_pitch_changes() {
        // THE acceptance criterion: change pitch, keep formant_ratio == 1.0,
        // and the spectral centroid (formant location) must stay close to the
        // input's — i.e. no chipmunk shift of the resonances.
        let sr = SR as f32;
        let n = SR as usize;
        let input = vowel(110.0, sr, n);
        let centroid_in = spectral_centroid(&input[WARMUP..], sr);

        let mut fx = effect();
        fx.set_pitch_ratio(1.5); // shift pitch up
        fx.set_formant_ratio(1.0); // preserve formants
        let mut output = vec![0.0f32; n];
        fx.process(&input, &mut output, SR);
        let centroid_out = spectral_centroid(&output[WARMUP..], sr);

        let drift = (centroid_out - centroid_in).abs() / centroid_in.max(1.0);
        assert!(
            drift < 0.25,
            "formant centroid drifted {drift:.2} (in={centroid_in:.0} out={centroid_out:.0}); \
             preserved formants should keep it ~constant"
        );
    }

    #[test]
    fn formant_shift_moves_centroid_without_pitch_change() {
        // Alien-style: pitch_ratio 1.0, formant_ratio > 1.0 → centroid rises,
        // fundamental unchanged.
        let sr = SR as f32;
        let n = SR as usize;
        let input = vowel(110.0, sr, n);
        let centroid_in = spectral_centroid(&input[WARMUP..], sr);
        let f0_in = dominant_freq_hz(&input[WARMUP..], sr);

        let mut fx = effect();
        fx.set_pitch_ratio(1.0);
        fx.set_formant_ratio(1.6);
        let mut output = vec![0.0f32; n];
        fx.process(&input, &mut output, SR);
        let centroid_out = spectral_centroid(&output[WARMUP..], sr);
        let f0_out = dominant_freq_hz(&output[WARMUP..], sr);

        assert!(
            centroid_out > centroid_in * 1.1,
            "formant up should raise centroid: in={centroid_in:.0} out={centroid_out:.0}"
        );
        assert!(
            (f0_out - f0_in).abs() < f0_in * 0.25,
            "pitch should be ~unchanged: in={f0_in:.0} out={f0_out:.0}"
        );
    }

    #[test]
    fn ab_improvement_over_tier1_pitch_shift() {
        // Quality A/B (acceptance criterion): pitch up a vowel by 1.5x.
        // Tier-1 PitchShiftEffect drags the formants up with the pitch (centroid
        // rises a lot). Tier-3 FormantPitchEffect with formant_ratio==1.0 keeps
        // the centroid much closer to the original. Assert the formant-preserving
        // path drifts strictly less.
        use crate::audio::effects::PitchShiftEffect;
        let sr = SR as f32;
        let n = SR as usize;
        let input = vowel(110.0, sr, n);
        let centroid_in = spectral_centroid(&input[WARMUP..], sr);

        let mut tier1 = PitchShiftEffect::new(SR);
        tier1.set_semitones(12.0 * (1.5f32).log2()); // +1.5x in semitones
        let mut t1_out = vec![0.0f32; n];
        tier1.process(&input, &mut t1_out, SR);
        let t1_drift =
            (spectral_centroid(&t1_out[WARMUP..], sr) - centroid_in).abs() / centroid_in.max(1.0);

        let mut tier3 = effect();
        tier3.set_pitch_ratio(1.5);
        tier3.set_formant_ratio(1.0);
        let mut t3_out = vec![0.0f32; n];
        tier3.process(&input, &mut t3_out, SR);
        let t3_drift =
            (spectral_centroid(&t3_out[WARMUP..], sr) - centroid_in).abs() / centroid_in.max(1.0);

        assert!(
            t3_drift < t1_drift,
            "Tier-3 should preserve formants better than Tier-1: \
             tier1 drift={t1_drift:.3} tier3 drift={t3_drift:.3}"
        );
    }

    #[test]
    fn silence_in_silence_out() {
        let mut fx = effect();
        fx.set_pitch_ratio(1.5);
        let input = vec![0.0f32; 4096];
        let mut output = vec![0.0f32; 4096];
        fx.process(&input, &mut output, SR);
        assert!(output.iter().all(|&s| s.abs() < 1e-3));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib formant:: 2>&1 | tail -30`
Expected: FAIL — with the forward-only scaffold closure, `formant_envelope_preserved_when_pitch_changes`, `pitch_up_raises_fundamental_frequency`, `formant_shift_moves_centroid_without_pitch_change`, and `ab_improvement_over_tier1_pitch_shift` fail (no actual pitch/formant transform happens yet). `silence_in_silence_out` passes.

- [ ] **Step 3: Implement the DSP**

Introduce `const BINS: usize = WINDOW / 2 + 1;` (used now), then replace `build_node` with the real pipeline plus small free-function helpers. Keep each function ≤50 lines and cognitive-complexity ≤10 — extract the resampling and envelope steps into helpers so the closure stays short.

```rust
/// Symmetric moving-average half-width (in bins) for the formant-envelope
/// estimate. Wide enough to smooth past individual harmonics of a typical
/// voice fundamental, narrow enough to keep the formant peaks. At 48 kHz /
/// 1024-pt FFT each bin ≈ 46.9 Hz, so 6 bins ≈ ±280 Hz.
const ENV_SMOOTH_BINS: usize = 6;

/// Floor to avoid divide-by-zero when flattening the spectrum by its envelope.
const ENV_EPS: f32 = 1e-6;

/// Smooth `mag` into `env` with a symmetric moving average of half-width
/// `ENV_SMOOTH_BINS`, edge-clamped. Both slices have length `BINS`.
fn estimate_envelope(mag: &[f32], env: &mut [f32]) {
    let bins = mag.len();
    let w = ENV_SMOOTH_BINS as isize;
    for i in 0..bins {
        let mut acc = 0.0f32;
        let mut count = 0.0f32;
        let mut k = -w;
        while k <= w {
            let j = (i as isize + k).clamp(0, bins as isize - 1) as usize;
            acc += mag[j];
            count += 1.0;
            k += 1;
        }
        env[i] = acc / count;
    }
}

/// Linear-interpolated read of `src[pos]` for fractional `pos`, edge-clamped,
/// returning 0.0 if `pos` is outside `[0, len-1]` by more than clamping covers.
fn lerp_read(src: &[f32], pos: f32) -> f32 {
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

fn build_node(window: usize, pitch: Shared, formant: Shared) -> impl AudioUnit {
    // Scratch captured by move; allocated once, overwritten per window.
    let mut mag = vec![0.0f32; BINS];
    let mut phase = vec![0.0f32; BINS];
    let mut env = vec![0.0f32; BINS];
    let mut exc = vec![0.0f32; BINS]; // flattened excitation magnitude

    resynth::<U1, U1, _>(window, move |fft| {
        let bins = fft.bins();
        let p = pitch.value().max(ENV_EPS);
        let f = formant.value().max(ENV_EPS);

        // 1. magnitudes + phases
        for i in 0..bins {
            let c = fft.at(0, i);
            mag[i] = c.norm();
            phase[i] = c.arg();
        }
        // 2. spectral envelope (formants)
        estimate_envelope(&mag[..bins], &mut env[..bins]);
        // 3. excitation = mag / envelope (flat of formant colour)
        for i in 0..bins {
            exc[i] = mag[i] / env[i].max(ENV_EPS);
        }
        // 4 + 5 + 6: per output bin, resample excitation by 1/p, re-impose
        // envelope resampled by 1/f, carry the source phase, write back.
        for i in 0..bins {
            let src_pos = i as f32 / p;
            let exc_mag = lerp_read(&exc[..bins], src_pos);
            let src_phase = lerp_read(&phase[..bins], src_pos);
            let env_mag = lerp_read(&env[..bins], i as f32 / f);
            let out_mag = exc_mag * env_mag;
            fft.set(0, i, Complex32::from_polar(out_mag, src_phase));
        }
    })
}
```

Notes:
- `Complex32::from_polar(r, theta)` is `num_complex`'s constructor; if unavailable in the pinned version, use `Complex32::new(out_mag * src_phase.cos(), out_mag * src_phase.sin())`.
- The closure body is one function; the two helpers keep its cognitive complexity low. If clippy still flags `too-many-lines`/`cognitive-complexity` on the closure, extract the "read magnitudes+phases" and "recombine" loops into named helpers taking slices.
- `mag`/`phase`/`env`/`exc` are sized `BINS` (= max bins for `WINDOW`); `bins` from the runtime window equals `BINS`, but slice with `..bins` defensively.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib formant:: 2>&1 | tail -30`
Expected: PASS for all formant tests including `formant_envelope_preserved_when_pitch_changes` and `ab_improvement_over_tier1_pitch_shift`.

If `formant_envelope_preserved_when_pitch_changes` is borderline, widen `ENV_SMOOTH_BINS` (more smoothing = flatter envelope estimate = better separation) before loosening the test threshold. Do **not** weaken a test to pass; fix the DSP. If A/B is borderline, confirm Tier-1 really drags formants (it does — it resamples) and that the WARMUP skip is long enough.

- [ ] **Step 5: clippy + fmt + commit**

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings 2>&1 | tail -20
git add src/audio/effects/formant.rs
git commit -m "feat(audio): implement formant-preserving pitch-shift DSP

Spectral-envelope extraction, excitation resampling by pitch ratio, and
independent envelope re-imposition by formant ratio inside the fundsp resynth
window. Pins the formant-preserved + A/B-vs-Tier1 properties. Issue #35.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Final verification + file-size/latency audit

**Files:** none (verification only); small doc/const tweaks if the audit demands.

- [ ] **Step 1: Full gate**

Run:
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings 2>&1 | tail -20
cargo test 2>&1 | tail -25
cargo build --release 2>&1 | tail -5
```
Expected: fmt clean, clippy clean, **all** tests pass (not just formant), release builds.

- [ ] **Step 2: File-size + function-length audit**

Run: `wc -l src/audio/effects/formant.rs src/audio/effects/formant_preset.rs`
Expected: both ≤400. If `formant.rs` exceeds 400, extract the DSP helpers (`estimate_envelope`, `lerp_read`, the read/recombine loops) into a sibling `src/audio/effects/formant_dsp.rs` and `use` it.

- [ ] **Step 3: Confirm no Cargo.lock / cargo-sources.json change**

Run: `git status --short Cargo.toml Cargo.lock packaging/flatpak/cargo-sources.json`
Expected: **empty** — no dependency change was introduced, so the Flatpak freshness gate (#121) is not engaged. If `Cargo.lock` changed, you added a crate by mistake — revert it.

- [ ] **Step 4: Commit any audit fixes** (only if Step 2 forced a split)

```bash
git add src/audio/effects/
git commit -m "refactor(audio): keep formant effect within file-size budget

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review (completed against the issue)

**Spec coverage:**
- "Formant-aware pitch shift (no chipmunk/barrel)" → Task 3 `formant_envelope_preserved_when_pitch_changes`.
- "Independent formant control" → Task 2/3 `set_formant_ratio`, Task 3 `formant_shift_moves_centroid_without_pitch_change`.
- "Presets: Alien / Gender Swap / Natural Deep" → Task 1 `FormantPreset` + Task 2 `apply_preset`.
- "Implements AudioEffect trait" → Task 2.
- "Latency <30 ms at 48 kHz" → Task 2 `latency_is_one_window` (1024/48000 ≈ 21.3 ms).
- "Unit tests: formant envelope preserved when pitch changes" → Task 3.
- "A/B vs pitch_shift Tier 1" → Task 3 `ab_improvement_over_tier1_pitch_shift`.
- fundsp resynth + custom phase-vocoder approach → Tasks 2–3. TD-PSOLA/`tdpsola` deliberately avoided (AGPL); documented in PR.

**Placeholder scan:** no TBD/TODO; every code step shows full code.

**Type consistency:** `FormantPitchEffect::new(sample_rate)`, `set_pitch_ratio`/`set_formant_ratio`/`set_semitones`/`apply_preset`, `FormantPreset::{pitch_ratio,formant_ratio,name,all}`, helper signatures `estimate_envelope(&[f32], &mut [f32])` / `lerp_read(&[f32], f32) -> f32` are consistent across Tasks 1–3. Param strings (`pitch_ratio`/`pitch_factor`/`semitones`/`formant_ratio`/`formant_shift`) match between `set_param` and the slot-layout vocabulary.
