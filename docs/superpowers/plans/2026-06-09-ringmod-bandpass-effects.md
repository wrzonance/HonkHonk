# Ring Modulation + Bandpass Filter Effects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a robot-voice (ring modulation) effect and a radio/walkie-talkie (bandpass + pink-noise) effect, both implementing the existing `AudioEffect` trait, built on fundsp operator-composed DSP graphs.

**Architecture:** Each effect owns fundsp nodes constructed in `new()`. `process()` is real-time safe — it only calls fundsp `tick()` (pure arithmetic, no alloc/lock/syscall) per sample, plus a one-time `set_sample_rate()` guarded by an "only when the rate actually changes" check. Parameter changes happen in `set_param()` (the command-handler / cold path, not the RT callback), so the bandpass uses fundsp's RT-safe `Setting::center_q` while the ring modulator rebuilds its tiny `Box<dyn AudioUnit>` graph (cheap, off the RT thread).

**Tech Stack:** Rust, fundsp 0.23 (`prelude32`), existing `AudioEffect` trait in `src/audio/effects/mod.rs`.

---

## File Structure

- `src/audio/effects/modulation.rs` (create) — `RingModEffect`. Owns a boxed `pass() * sine_hz(carrier)` graph. Param: `carrier` / `carrier_hz`.
- `src/audio/effects/filter.rs` (create) — `BandpassFilterEffect`. Owns a `bandpass_hz` SVF node + a `pink()` noise generator. Params: `center`, `bandwidth`, `noise`.
- `src/audio/effects/mod.rs` (modify) — add `pub mod modulation;` + `pub mod filter;` and re-export the two effect types (additive lines only).

Out of scope (explicitly): wiring these into the audio engine / mixer, UI controls, presets, pitch shifting, reverb. This PR only adds the two effect structs + unit tests.

---

## Task 1: RingModEffect skeleton + carrier default

**Files:**
- Create: `src/audio/effects/modulation.rs`
- Modify: `src/audio/effects/mod.rs`

- [ ] **Step 1: Add module declaration + re-export to `mod.rs`**

Add after the existing `pub mod commands;` line:

```rust
pub mod filter;
pub mod modulation;
```

And after the existing `pub use commands::...` line:

```rust
pub use filter::BandpassFilterEffect;
pub use modulation::RingModEffect;
```

- [ ] **Step 2: Write the failing test for construction + default carrier**

Create `src/audio/effects/modulation.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_mod_default_carrier_is_150hz() {
        let effect = RingModEffect::new(1024);
        assert!((effect.carrier_hz() - 150.0).abs() < 1e-3);
    }
}
```

- [ ] **Step 3: Run test — expect compile failure (RingModEffect undefined)**

Run: `cargo test --lib modulation`
Expected: FAIL (cannot find `RingModEffect`).

- [ ] **Step 4: Implement minimal struct**

```rust
use crate::audio::error::EffectsError;
use crate::audio::effects::AudioEffect;
use fundsp::prelude32::*;

const DEFAULT_CARRIER_HZ: f32 = 150.0;

pub struct RingModEffect {
    graph: Box<dyn AudioUnit>,
    carrier_hz: f32,
    sample_rate: u32,
    bypassed: bool,
}

impl RingModEffect {
    #[must_use]
    pub fn new(_block_size: usize) -> Self {
        Self {
            graph: Self::build_graph(DEFAULT_CARRIER_HZ),
            carrier_hz: DEFAULT_CARRIER_HZ,
            sample_rate: 0,
            bypassed: false,
        }
    }

    fn build_graph(carrier_hz: f32) -> Box<dyn AudioUnit> {
        Box::new(pass() * sine_hz::<f32>(carrier_hz))
    }

    #[must_use]
    pub fn carrier_hz(&self) -> f32 {
        self.carrier_hz
    }
}
```

- [ ] **Step 5: Run test — expect PASS**

Run: `cargo test --lib modulation`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/audio/effects/modulation.rs src/audio/effects/mod.rs
git commit -m "feat(audio/effects): RingModEffect skeleton with default carrier"
```

---

## Task 2: RingMod `process()` produces ring-modulated output

**Files:**
- Modify: `src/audio/effects/modulation.rs`

- [ ] **Step 1: Write the failing test — ring mod produces sum/difference frequencies**

Ring modulation of a sine input `f_in` by carrier `f_c` yields energy at `f_in ± f_c` and (ideally) suppresses `f_in`. We test via a Goertzel single-bin magnitude helper: feed a pure input tone, assert the input-frequency bin is attenuated and a sideband bin (`f_in + f_c`) carries energy.

```rust
    fn goertzel_mag(samples: &[f32], freq: f32, sample_rate: f32) -> f32 {
        let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let coeff = 2.0 * omega.cos();
        let (mut s0, mut s1, mut s2) = (0.0f32, 0.0f32, 0.0f32);
        for &x in samples {
            s0 = x + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt()
    }

    fn sine_block(freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn ring_mod_produces_sidebands_and_suppresses_input() {
        let sr = 48_000.0f32;
        let f_in = 1000.0f32;
        let f_c = 150.0f32;
        let n = 4096;
        let mut effect = RingModEffect::new(n);
        let input = sine_block(f_in, sr, n);
        let mut output = vec![0.0f32; n];
        effect.process(&input, &mut output, sr as u32);

        let mag_in = goertzel_mag(&output, f_in, sr);
        let mag_upper = goertzel_mag(&output, f_in + f_c, sr);
        let mag_lower = goertzel_mag(&output, f_in - f_c, sr);

        // Sidebands carry the energy; the original input frequency is suppressed.
        assert!(mag_upper > mag_in * 5.0, "upper sideband should dominate input bin");
        assert!(mag_lower > mag_in * 5.0, "lower sideband should dominate input bin");
    }
```

- [ ] **Step 2: Run test — expect FAIL (process unimplemented / trait not impl)**

Run: `cargo test --lib modulation::tests::ring_mod_produces_sidebands`
Expected: FAIL.

- [ ] **Step 3: Implement `AudioEffect` for `RingModEffect`**

```rust
impl AudioEffect for RingModEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        if sample_rate != self.sample_rate {
            self.graph.set_sample_rate(f64::from(sample_rate));
            self.sample_rate = sample_rate;
        }
        let mut frame_in = [0.0f32; 1];
        let mut frame_out = [0.0f32; 1];
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            frame_in[0] = i;
            self.graph.tick(&frame_in, &mut frame_out);
            *o = frame_out[0];
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "carrier" | "carrier_hz" => {
                self.carrier_hz = value.max(0.0);
                self.graph = Self::build_graph(self.carrier_hz);
                if self.sample_rate != 0 {
                    self.graph.set_sample_rate(f64::from(self.sample_rate));
                }
                Ok(())
            }
            _ => Err(EffectsError::ParamUnknown {
                param: param.to_owned(),
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
        0
    }
}
```

- [ ] **Step 4: Run test — expect PASS**

Run: `cargo test --lib modulation`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/audio/effects/modulation.rs
git commit -m "feat(audio/effects): RingModEffect ring modulation via fundsp"
```

---

## Task 3: RingMod param + bypass behavior tests

**Files:**
- Modify: `src/audio/effects/modulation.rs`

- [ ] **Step 1: Write failing tests for set_param, unknown param, bypass**

```rust
    #[test]
    fn ring_mod_set_carrier_changes_value() {
        let mut effect = RingModEffect::new(64);
        effect.set_param("carrier", 300.0).unwrap();
        assert!((effect.carrier_hz() - 300.0).abs() < 1e-3);
    }

    #[test]
    fn ring_mod_unknown_param_rejected() {
        let mut effect = RingModEffect::new(64);
        let err = effect.set_param("nope", 1.0);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn ring_mod_bypass_passes_through() {
        let mut effect = RingModEffect::new(64);
        effect.set_bypass(true);
        let input = vec![0.1f32, -0.2, 0.3, -0.4];
        let mut output = vec![0.0f32; 4];
        effect.process(&input, &mut output, 48_000);
        assert_eq!(output, input);
    }

    #[test]
    fn ring_mod_latency_is_zero() {
        let effect = RingModEffect::new(64);
        assert_eq!(effect.latency_samples(), 0);
    }
```

- [ ] **Step 2: Run tests — expect PASS (already implemented in Task 2)**

Run: `cargo test --lib modulation`
Expected: PASS. (These pin behavior already implemented; if any fails, fix the impl, not the test.)

- [ ] **Step 3: Commit**

```bash
git add src/audio/effects/modulation.rs
git commit -m "test(audio/effects): pin RingModEffect param + bypass behavior"
```

---

## Task 4: BandpassFilterEffect skeleton + defaults

**Files:**
- Create: `src/audio/effects/filter.rs`

- [ ] **Step 1: Write failing test for construction + defaults**

Create `src/audio/effects/filter.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bandpass_defaults() {
        let effect = BandpassFilterEffect::new(1024);
        assert!((effect.center_hz() - 1500.0).abs() < 1e-3);
        assert!((effect.bandwidth_hz() - 2000.0).abs() < 1e-3);
        assert!(effect.noise_level().abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run test — expect compile failure**

Run: `cargo test --lib filter`
Expected: FAIL (cannot find `BandpassFilterEffect`).

- [ ] **Step 3: Implement minimal struct**

`bandpass_hz(center, q)` takes Q, not bandwidth. Convert: `q = center / bandwidth` (standard relationship Q = f0 / BW). Default center 1500 Hz, bandwidth 2000 Hz → Q = 0.75.

```rust
use crate::audio::effects::AudioEffect;
use crate::audio::error::EffectsError;
use fundsp::prelude32::*;

const DEFAULT_CENTER_HZ: f32 = 1500.0;
const DEFAULT_BANDWIDTH_HZ: f32 = 2000.0;
const MIN_BANDWIDTH_HZ: f32 = 1.0;

pub struct BandpassFilterEffect {
    filter: An<FixedSvf<f32, BandpassMode<f32>>>,
    noise: An<Pipe<Noise, Pinkpass<f32>>>,
    center_hz: f32,
    bandwidth_hz: f32,
    noise_level: f32,
    sample_rate: u32,
    bypassed: bool,
}

impl BandpassFilterEffect {
    #[must_use]
    pub fn new(_block_size: usize) -> Self {
        let q = DEFAULT_CENTER_HZ / DEFAULT_BANDWIDTH_HZ;
        Self {
            filter: bandpass_hz::<f32>(DEFAULT_CENTER_HZ, q),
            noise: pink::<f32>(),
            center_hz: DEFAULT_CENTER_HZ,
            bandwidth_hz: DEFAULT_BANDWIDTH_HZ,
            noise_level: 0.0,
            sample_rate: 0,
            bypassed: false,
        }
    }

    fn q(&self) -> f32 {
        self.center_hz / self.bandwidth_hz.max(MIN_BANDWIDTH_HZ)
    }

    #[must_use]
    pub fn center_hz(&self) -> f32 {
        self.center_hz
    }

    #[must_use]
    pub fn bandwidth_hz(&self) -> f32 {
        self.bandwidth_hz
    }

    #[must_use]
    pub fn noise_level(&self) -> f32 {
        self.noise_level
    }
}
```

- [ ] **Step 4: Run test — expect PASS**

Run: `cargo test --lib filter`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/audio/effects/filter.rs
git commit -m "feat(audio/effects): BandpassFilterEffect skeleton with defaults"
```

---

## Task 5: Bandpass `process()` attenuates out-of-band, passes in-band

**Files:**
- Modify: `src/audio/effects/filter.rs`

- [ ] **Step 1: Write failing test — in-band passes, out-of-band attenuated**

```rust
    fn goertzel_mag(samples: &[f32], freq: f32, sample_rate: f32) -> f32 {
        let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let coeff = 2.0 * omega.cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in samples {
            let s0 = x + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt()
    }

    fn sine_block(freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn bandpass_passes_in_band_attenuates_out_of_band() {
        let sr = 48_000.0f32;
        let n = 8192;
        let warmup = 2048; // skip filter transient

        // In-band tone at the center frequency.
        let mut in_band = BandpassFilterEffect::new(n);
        let inb_input = sine_block(1500.0, sr, n);
        let mut inb_out = vec![0.0f32; n];
        in_band.process(&inb_input, &mut inb_out, sr as u32);

        // Out-of-band low tone well below the passband.
        let mut low = BandpassFilterEffect::new(n);
        let low_input = sine_block(100.0, sr, n);
        let mut low_out = vec![0.0f32; n];
        low.process(&low_input, &mut low_out, sr as u32);

        let inb_rms = rms(&inb_out[warmup..]);
        let low_rms = rms(&low_out[warmup..]);

        assert!(inb_rms > 0.2, "in-band tone should pass, got {inb_rms}");
        assert!(
            low_rms < inb_rms * 0.5,
            "out-of-band tone should be attenuated: low={low_rms} in_band={inb_rms}"
        );
        // Spectral confirmation: center bin energy dominates the low bin.
        let center_bin = goertzel_mag(&inb_out[warmup..], 1500.0, sr);
        let low_bin = goertzel_mag(&low_out[warmup..], 100.0, sr);
        assert!(center_bin > low_bin, "center bin should exceed out-of-band bin");
    }
```

- [ ] **Step 2: Run test — expect FAIL (AudioEffect not impl)**

Run: `cargo test --lib filter::tests::bandpass_passes_in_band`
Expected: FAIL.

- [ ] **Step 3: Implement `AudioEffect` for `BandpassFilterEffect`**

Noise is added post-filter as a runtime-scaled mix (keeps `noise_level` changes RT-safe and rebuild-free). Center/bandwidth use fundsp's RT-safe `Setting::center_q` so no graph rebuild is needed for those either.

```rust
impl AudioEffect for BandpassFilterEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());
        if self.bypassed {
            output.copy_from_slice(input);
            return;
        }
        if sample_rate != self.sample_rate {
            self.filter.set_sample_rate(f64::from(sample_rate));
            self.noise.set_sample_rate(f64::from(sample_rate));
            self.sample_rate = sample_rate;
        }
        let noise_level = self.noise_level;
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            let filtered = self.filter.filter_mono(i);
            let crackle = if noise_level > 0.0 {
                self.noise.get_mono() * noise_level
            } else {
                0.0
            };
            *o = filtered + crackle;
        }
    }

    fn set_param(&mut self, param: &str, value: f32) -> Result<(), EffectsError> {
        match param {
            "center" | "center_hz" => {
                self.center_hz = value.max(0.0);
                self.filter.set(Setting::center_q(self.center_hz, self.q()));
                Ok(())
            }
            "bandwidth" | "bandwidth_hz" => {
                self.bandwidth_hz = value.max(MIN_BANDWIDTH_HZ);
                self.filter.set(Setting::center_q(self.center_hz, self.q()));
                Ok(())
            }
            "noise" | "noise_level" => {
                self.noise_level = value.clamp(0.0, 1.0);
                Ok(())
            }
            _ => Err(EffectsError::ParamUnknown {
                param: param.to_owned(),
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
        0
    }
}
```

- [ ] **Step 4: Run test — expect PASS**

Run: `cargo test --lib filter`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/audio/effects/filter.rs
git commit -m "feat(audio/effects): BandpassFilterEffect bandpass + noise via fundsp"
```

---

## Task 6: Bandpass param, noise-mix, bypass tests

**Files:**
- Modify: `src/audio/effects/filter.rs`

- [ ] **Step 1: Write tests for param changes, noise adds energy, unknown param, bypass**

```rust
    #[test]
    fn bandpass_set_center_and_bandwidth() {
        let mut effect = BandpassFilterEffect::new(64);
        effect.set_param("center", 1000.0).unwrap();
        effect.set_param("bandwidth", 500.0).unwrap();
        assert!((effect.center_hz() - 1000.0).abs() < 1e-3);
        assert!((effect.bandwidth_hz() - 500.0).abs() < 1e-3);
    }

    #[test]
    fn bandpass_noise_adds_energy_to_silence() {
        let n = 2048;
        let mut effect = BandpassFilterEffect::new(n);
        effect.set_param("noise", 0.5).unwrap();
        let input = vec![0.0f32; n];
        let mut output = vec![0.0f32; n];
        effect.process(&input, &mut output, 48_000);
        let energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "noise mix should add energy to a silent input");
    }

    #[test]
    fn bandpass_zero_noise_silent_input_stays_silent() {
        let n = 512;
        let mut effect = BandpassFilterEffect::new(n);
        let input = vec![0.0f32; n];
        let mut output = vec![0.0f32; n];
        effect.process(&input, &mut output, 48_000);
        let energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(energy < 1e-9, "no noise + silence in => silence out");
    }

    #[test]
    fn bandpass_unknown_param_rejected() {
        let mut effect = BandpassFilterEffect::new(64);
        let err = effect.set_param("nope", 1.0);
        assert!(matches!(err, Err(EffectsError::ParamUnknown { .. })));
    }

    #[test]
    fn bandpass_bypass_passes_through() {
        let mut effect = BandpassFilterEffect::new(64);
        effect.set_bypass(true);
        let input = vec![0.1f32, -0.2, 0.3, -0.4];
        let mut output = vec![0.0f32; 4];
        effect.process(&input, &mut output, 48_000);
        assert_eq!(output, input);
    }
```

- [ ] **Step 2: Run tests — expect PASS**

Run: `cargo test --lib filter`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/audio/effects/filter.rs
git commit -m "test(audio/effects): pin BandpassFilterEffect params, noise, bypass"
```

---

## Task 7: Composability test (both effects in EffectChain)

**Files:**
- Modify: `src/audio/effects/filter.rs` (test only — chain is the integration point)

- [ ] **Step 1: Write failing/integration test putting both effects in an EffectChain**

```rust
    #[test]
    fn both_effects_compose_in_chain() {
        use crate::audio::effects::chain::EffectChain;
        use crate::audio::effects::modulation::RingModEffect;

        let block = 256;
        let mut chain = EffectChain::new(block);
        chain
            .push_effect(Box::new(RingModEffect::new(block)), block)
            .unwrap();
        chain
            .push_effect(Box::new(BandpassFilterEffect::new(block)), block)
            .unwrap();

        let input: Vec<f32> = (0..block)
            .map(|i| (2.0 * std::f32::consts::PI * 1500.0 * i as f32 / 48_000.0).sin())
            .collect();
        let mut output = vec![0.0f32; block];
        chain.process(&input, &mut output, 48_000);

        // Output must be finite and not identical to input (effects applied).
        assert!(output.iter().all(|s| s.is_finite()));
        assert!(output != input, "chained effects should transform the signal");
    }
```

- [ ] **Step 2: Run test — expect PASS (both effects already implement the trait)**

Run: `cargo test --lib filter::tests::both_effects_compose_in_chain`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/audio/effects/filter.rs
git commit -m "test(audio/effects): RingMod + Bandpass compose in EffectChain"
```

---

## Task 8: Full verification + format/lint

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt`

- [ ] **Step 2: Verify format clean**

Run: `cargo fmt -- --check`
Expected: no output (clean).

- [ ] **Step 3: Clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings. Fix any (e.g. needless casts, missing `#[must_use]`).

- [ ] **Step 4: Full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit any fmt/clippy fixups**

```bash
git add -A
git commit -m "chore(audio/effects): fmt + clippy cleanup for ringmod/bandpass" || echo "nothing to commit"
```

---

## Self-Review Notes

- **Spec coverage:** modulation.rs ✓ (T1-3), filter.rs ✓ (T4-6), robot ring mod w/ carrier default 150 Hz ✓ (T1), radio bandpass center 1500/bandwidth 2000 + noise ✓ (T4), params exposed ✓ (T3/T6), composable in chain ✓ (T7), unit tests for sidebands + out-of-band attenuation ✓ (T2/T5), operator composition (`pass() * sine_hz`, `pink()` = `white() >> pinkpass`) ✓.
- **Q vs bandwidth:** issue gives bandwidth; fundsp `bandpass_hz` wants Q. Converted via Q = center/bandwidth, documented in code + PR.
- **RT-safety:** `process()` only ticks fundsp nodes + a guarded `set_sample_rate` on rate change. No alloc in the per-sample loop. Graph rebuild (RingMod) only in `set_param` (cold path).
