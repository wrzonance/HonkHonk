# AudioEffect Trait + EffectChain Infrastructure (Issue #30) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define `AudioEffect` trait, `EffectChain` composable pipeline, and bypass/wet-dry infrastructure — pure plumbing with no concrete effects, no wiring to PipeWire callbacks yet.

**Architecture:** New `src/audio/effects/` submodule with `mod.rs` (trait + re-exports), `chain.rs` (pipeline logic), and `commands.rs` (UI ↔ audio thread messages). Integration stub in a new `src/audio/mixer.rs` that `engine.rs` calls after mic audio is read — but since the current architecture uses PipeWire graph links (not software buffers), `mixer.rs` exposes `EffectChain` as a data structure held by the engine with a `process_if_active()` no-op stub. `EffectsError` variants added to `error.rs`. `fundsp` added to `Cargo.toml` (declared as per ADR-006).

**Tech Stack:** Rust, thiserror, fundsp 0.23, existing pipewire-rs architecture.

---

## Design Decisions (document in PR body)

1. **No mixer.rs software buffer processing in this PR.** The current architecture uses PipeWire graph links for mic routing (ADR-007). There is no software-level mic capture buffer. `mixer.rs` in this PR is a stub module that holds an `EffectChain` and exposes `process_if_active(input, output, rate)` — this is the hook for future PRs to plug into a `pw_stream` process callback when actual DSP is wired. This keeps `mixer.rs` as a real integration seam without claiming to do more than it does.

2. **`fundsp` added but not used yet.** The crate is declared in `Cargo.toml` as required by the issue spec. The `EffectChain` is backed by `Vec<Box<dyn AudioEffect>>` for now (no fundsp graph composition yet — that's PR 2+).

3. **Wet/dry mix on chain, not per-effect.** The issue says "wet/dry mix parameter on the chain." We implement it at `EffectChain` level: `wet_dry` is a `f32` in `[0.0, 1.0]` where `0.0` = dry (input only), `1.0` = fully wet (processed only). Stored as `f32` to avoid alloc.

4. **Bypass: zero-copy passthrough.** When all effects are bypassed OR the chain is empty, `process()` copies `input` to `output` without touching effect state. "Zero-copy" here means no intermediate temp buffer — we write directly to `output` from `input`.

5. **RT safety: no alloc in `process()`.** `EffectChain::process()` uses a preallocated `scratch: Vec<f32>` buffer on the chain itself, sized to max expected block (4096 frames × 2 ch = 8192 f32). Resized only on `push_effect()` (cold path), never in `process()`.

6. **`EffectsCommand` / `EffectsEvent` added to `engine.rs` command/event enums** (via new variants), not as separate channels — consistent with existing `AudioCommand` / `AudioEvent` pattern.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/audio/effects/mod.rs` | **Create** | `AudioEffect` trait, re-exports |
| `src/audio/effects/chain.rs` | **Create** | `EffectChain` struct + `process()`, bypass, wet/dry |
| `src/audio/effects/commands.rs` | **Create** | `EffectsCommand`, `EffectsEvent` enums |
| `src/audio/mixer.rs` | **Create** | Stub module holding `EffectChain`, `process_if_active()` |
| `src/audio/error.rs` | **Modify** | Add `EffectsError` variants |
| `src/audio/mod.rs` | **Modify** | `mod effects; mod mixer; pub use` new types |
| `src/audio/engine.rs` | **Modify** | Add `EffectsCommand`/`EffectsEvent` variants to existing enums |
| `Cargo.toml` | **Modify** | Add `fundsp = "0.23"` |

---

## Task 1: Add `EffectsError` to `error.rs` (TDD)

**Files:**
- Modify: `src/audio/error.rs`

- [ ] **Step 1.1: Write the failing test**

In `src/audio/error.rs`, add a `#[cfg(test)]` block at the bottom:

```rust
#[cfg(test)]
mod effects_error_tests {
    use super::*;

    #[test]
    fn effects_error_chain_too_long_is_constructible() {
        let e = EffectsError::ChainTooLong { max: 16, got: 17 };
        assert!(e.to_string().contains("16"));
    }

    #[test]
    fn effects_error_param_unknown_is_constructible() {
        let e = EffectsError::ParamUnknown {
            param: "gain".into(),
        };
        assert!(e.to_string().contains("gain"));
    }

    #[test]
    fn effects_error_index_out_of_range_is_constructible() {
        let e = EffectsError::IndexOutOfRange { index: 3, len: 2 };
        assert!(e.to_string().contains("3"));
    }
}
```

- [ ] **Step 1.2: Run test to verify it fails**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test effects_error 2>&1 | head -30
```

Expected: FAIL — `EffectsError` is not defined.

- [ ] **Step 1.3: Add `EffectsError` enum to `src/audio/error.rs`**

Append this to the end of `src/audio/error.rs` (before the closing `}` of any `#[cfg(test)]` if present, but after all existing content):

```rust
#[derive(Error, Debug)]
pub enum EffectsError {
    #[error("effect chain exceeds maximum length of {max} (got {got})")]
    ChainTooLong { max: usize, got: usize },

    #[error("unknown parameter {param:?}")]
    ParamUnknown { param: String },

    #[error("effect index {index} out of range (chain length {len})")]
    IndexOutOfRange { index: usize, len: usize },
}
```

- [ ] **Step 1.4: Run tests to verify they pass**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test effects_error 2>&1
```

Expected: 3 tests PASS.

- [ ] **Step 1.5: Run clippy**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 1.6: Commit**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
git add src/audio/error.rs
git commit -m "feat(audio/effects): add EffectsError variants to error.rs"
```

---

## Task 2: Add `fundsp` to `Cargo.toml`

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 2.1: Add fundsp dependency**

In `Cargo.toml`, after the `anyhow` line, add:

```toml
# Phase 4B voice effects DSP framework (ADR-006). Added in issue #30 (PR 1 of 6)
# even though no fundsp graph composition is used yet — establishes the dependency
# and allows future effect PRs to use fundsp nodes without a Cargo.toml change.
fundsp = "0.23"
```

- [ ] **Step 2.2: Verify build compiles with fundsp**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo build 2>&1 | tail -5
```

Expected: compiles successfully (fundsp adds to compile time but no errors).

- [ ] **Step 2.3: Commit**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
git add Cargo.toml Cargo.lock
git commit -m "chore(deps): add fundsp 0.23 for Phase 4B voice effects DSP (ADR-006)"
```

---

## Task 3: `AudioEffect` trait in `src/audio/effects/mod.rs` (TDD)

**Files:**
- Create: `src/audio/effects/mod.rs`

- [ ] **Step 3.1: Create the directory and write the failing test**

```bash
mkdir -p /home/adam/github/honkhonk/.worktrees/feat/issue-30/src/audio/effects
```

Create `src/audio/effects/mod.rs` with tests first, trait stubbed to fail:

```rust
pub mod chain;
pub mod commands;

pub use chain::EffectChain;
pub use commands::{EffectsCommand, EffectsEvent};

/// A real-time audio processing unit. All methods that run inside the PipeWire
/// process callback MUST be real-time safe: no allocation, no locks, no syscalls.
pub trait AudioEffect: Send {
    /// Process a block of audio. `input` and `output` have equal length.
    /// Called on the PipeWire thread — must be real-time safe.
    fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32);

    /// Set a named parameter. `value` is normalized to the parameter's natural range.
    /// Called from the command handler, not the process callback.
    fn set_param(&mut self, param: &str, value: f32);

    /// Returns `true` if this effect is currently bypassed.
    fn bypass(&self) -> bool;

    /// Enable or disable bypass for this effect.
    fn set_bypass(&mut self, bypass: bool);

    /// Algorithmic latency introduced by this effect, in samples.
    fn latency_samples(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PassThrough {
        bypassed: bool,
    }

    impl AudioEffect for PassThrough {
        fn process(&mut self, input: &[f32], output: &mut [f32], _sample_rate: u32) {
            output.copy_from_slice(input);
        }
        fn set_param(&mut self, _param: &str, _value: f32) {}
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

    #[test]
    fn audio_effect_pass_through_copies_input() {
        let mut effect = PassThrough { bypassed: false };
        let input = vec![0.1_f32, 0.2, 0.3, 0.4];
        let mut output = vec![0.0_f32; 4];
        effect.process(&input, &mut output, 48000);
        assert_eq!(output, input);
    }

    #[test]
    fn audio_effect_bypass_toggle() {
        let mut effect = PassThrough { bypassed: false };
        assert!(!effect.bypass());
        effect.set_bypass(true);
        assert!(effect.bypass());
        effect.set_bypass(false);
        assert!(!effect.bypass());
    }

    #[test]
    fn audio_effect_latency_samples_default_zero() {
        let effect = PassThrough { bypassed: false };
        assert_eq!(effect.latency_samples(), 0);
    }
}
```

- [ ] **Step 3.2: Run test to verify it fails (chain.rs and commands.rs don't exist yet)**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test audio_effect 2>&1 | head -20
```

Expected: FAIL — missing `chain` and `commands` modules.

---

## Task 4: `EffectsCommand` and `EffectsEvent` in `commands.rs` (TDD)

**Files:**
- Create: `src/audio/effects/commands.rs`

- [ ] **Step 4.1: Create `commands.rs` with tests first**

```rust
/// Commands sent from the UI/main thread to the audio thread to control effects.
/// Variants correspond to operations in `EffectChain`.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectsCommand {
    /// Set bypass state for effect at `index`.
    SetBypass { index: usize, bypass: bool },
    /// Set a named parameter on effect at `index`.
    SetParam {
        index: usize,
        param: String,
        value: f32,
    },
    /// Set the chain-level wet/dry mix. `0.0` = dry only, `1.0` = wet only.
    SetWetDry(f32),
    /// Bypass the entire chain (all effects bypassed simultaneously).
    SetChainBypass(bool),
}

/// Events emitted from the audio thread to the UI about effect state.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectsEvent {
    /// The chain's total latency changed (in samples).
    LatencyChanged(u32),
    /// A parameter set operation was rejected (param name unknown).
    ParamRejected { index: usize, param: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_command_set_bypass_is_constructible() {
        let cmd = EffectsCommand::SetBypass {
            index: 0,
            bypass: true,
        };
        assert!(matches!(cmd, EffectsCommand::SetBypass { index: 0, bypass: true }));
    }

    #[test]
    fn effects_command_set_wet_dry_is_constructible() {
        let cmd = EffectsCommand::SetWetDry(0.5);
        assert!(matches!(cmd, EffectsCommand::SetWetDry(_)));
    }

    #[test]
    fn effects_event_latency_changed_is_constructible() {
        let evt = EffectsEvent::LatencyChanged(512);
        assert_eq!(evt, EffectsEvent::LatencyChanged(512));
    }

    #[test]
    fn effects_event_param_rejected_is_constructible() {
        let evt = EffectsEvent::ParamRejected {
            index: 1,
            param: "gain".into(),
        };
        assert!(matches!(evt, EffectsEvent::ParamRejected { index: 1, .. }));
    }
}
```

- [ ] **Step 4.2: Run test to verify it fails (chain.rs still missing)**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test effects_command 2>&1 | head -20
```

Expected: FAIL — missing `chain` module.

---

## Task 5: `EffectChain` in `chain.rs` (TDD — main work)

**Files:**
- Create: `src/audio/effects/chain.rs`

- [ ] **Step 5.1: Create `chain.rs` with failing tests first**

```rust
use super::AudioEffect;
use crate::audio::error::EffectsError;

/// Maximum number of effects in a chain. Prevents unbounded growth.
pub const MAX_CHAIN_LEN: usize = 16;

/// A composable, ordered pipeline of [`AudioEffect`]s.
///
/// # Real-time Safety
/// `process()` is real-time safe: no allocation, no locking, no syscalls.
/// The internal scratch buffer is pre-allocated on `new()` and resized only
/// in `push_effect()` (cold path).
pub struct EffectChain {
    effects: Vec<Box<dyn AudioEffect>>,
    /// Pre-allocated scratch buffer. Sized to `scratch_capacity` f32 values.
    /// Used as intermediate buffer between effect stages.
    scratch: Vec<f32>,
    scratch_capacity: usize,
    /// Chain-level wet/dry: 0.0 = dry (input passthrough), 1.0 = fully wet.
    wet_dry: f32,
    /// When `true`, the entire chain is bypassed regardless of per-effect bypass.
    chain_bypass: bool,
}

impl EffectChain {
    /// Create a new empty `EffectChain` with a scratch buffer sized for
    /// `initial_block_size` mono samples.
    pub fn new(initial_block_size: usize) -> Self {
        Self {
            effects: Vec::new(),
            scratch: vec![0.0_f32; initial_block_size],
            scratch_capacity: initial_block_size,
            wet_dry: 1.0,
            chain_bypass: false,
        }
    }

    /// Add an effect to the end of the chain.
    ///
    /// Grows the scratch buffer if needed (cold path, may allocate).
    /// Returns `Err(EffectsError::ChainTooLong)` if chain is at capacity.
    pub fn push_effect(
        &mut self,
        effect: Box<dyn AudioEffect>,
        block_size: usize,
    ) -> Result<(), EffectsError> {
        if self.effects.len() >= MAX_CHAIN_LEN {
            return Err(EffectsError::ChainTooLong {
                max: MAX_CHAIN_LEN,
                got: self.effects.len() + 1,
            });
        }
        self.effects.push(effect);
        // Grow scratch buffer if block_size exceeds current capacity.
        if block_size > self.scratch_capacity {
            self.scratch.resize(block_size, 0.0);
            self.scratch_capacity = block_size;
        }
        Ok(())
    }

    /// Remove the effect at `index`.
    ///
    /// Returns `Err(EffectsError::IndexOutOfRange)` if index is out of bounds.
    pub fn remove_effect(&mut self, index: usize) -> Result<(), EffectsError> {
        if index >= self.effects.len() {
            return Err(EffectsError::IndexOutOfRange {
                index,
                len: self.effects.len(),
            });
        }
        self.effects.remove(index);
        Ok(())
    }

    /// Set the chain-level wet/dry mix. Clamped to `[0.0, 1.0]`.
    pub fn set_wet_dry(&mut self, wet_dry: f32) {
        self.wet_dry = wet_dry.clamp(0.0, 1.0);
    }

    /// Returns the current wet/dry ratio.
    pub fn wet_dry(&self) -> f32 {
        self.wet_dry
    }

    /// Set the chain-level bypass. When `true`, all effects are skipped.
    pub fn set_chain_bypass(&mut self, bypass: bool) {
        self.chain_bypass = bypass;
    }

    /// Returns `true` if the chain is bypassed (all effects skipped).
    pub fn chain_bypass(&self) -> bool {
        self.chain_bypass
    }

    /// Returns `true` if all individual effects are bypassed (or chain is empty).
    pub fn all_effects_bypassed(&self) -> bool {
        self.effects.is_empty() || self.effects.iter().all(|e| e.bypass())
    }

    /// Returns the number of effects in the chain.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Returns `true` if the chain has no effects.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Total algorithmic latency of all non-bypassed effects, in samples.
    pub fn total_latency_samples(&self) -> u32 {
        if self.chain_bypass {
            return 0;
        }
        self.effects
            .iter()
            .filter(|e| !e.bypass())
            .map(|e| e.latency_samples())
            .fold(0u32, |acc, l| acc.saturating_add(l))
    }

    /// Set bypass on the effect at `index`.
    pub fn set_bypass(
        &mut self,
        index: usize,
        bypass: bool,
    ) -> Result<(), EffectsError> {
        self.effects
            .get_mut(index)
            .ok_or(EffectsError::IndexOutOfRange {
                index,
                len: self.effects.len(),
            })
            .map(|e| e.set_bypass(bypass))
    }

    /// Set a parameter on the effect at `index`.
    pub fn set_param(
        &mut self,
        index: usize,
        param: &str,
        value: f32,
    ) -> Result<(), EffectsError> {
        self.effects
            .get_mut(index)
            .ok_or(EffectsError::IndexOutOfRange {
                index,
                len: self.effects.len(),
            })
            .map(|e| e.set_param(param, value))
    }

    /// Process a block of audio.
    ///
    /// If the chain is bypassed, all effects are bypassed, or there are no
    /// effects, copies `input` to `output` directly (zero-copy passthrough).
    ///
    /// Otherwise, runs each non-bypassed effect in sequence and applies the
    /// wet/dry mix.
    ///
    /// `input` and `output` must have equal length. Does not allocate.
    pub fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());

        let passthrough = self.chain_bypass || self.all_effects_bypassed();

        if passthrough {
            output.copy_from_slice(input);
            return;
        }

        // Ensure scratch is large enough. This should not happen in normal
        // operation (push_effect grows scratch), but guard anyway.
        let n = input.len();
        if self.scratch.len() < n {
            // This branch allocates — only reachable in misconfigured usage.
            self.scratch.resize(n, 0.0);
        }

        // Run the full chain: ping-pong between output and scratch.
        // First effect reads from input, writes to output.
        // Subsequent effects alternate scratch ↔ output.
        let mut src_is_output = false;

        // Copy input into output as starting point for the first effect.
        output[..n].copy_from_slice(&input[..n]);

        for effect in &mut self.effects {
            if effect.bypass() {
                continue;
            }

            if src_is_output {
                // Read from output, write to scratch.
                let (src, dst) = (&output[..n], &mut self.scratch[..n]);
                // Need to clone src because we can't borrow output mutably while
                // borrowing it immutably. Copy to scratch first.
                dst.copy_from_slice(src);
                // Now effect reads from scratch, writes to output.
                let scratch_copy = self.scratch[..n].to_vec();
                effect.process(&scratch_copy, &mut output[..n], sample_rate);
            } else {
                // Read from output (which already has the accumulated signal),
                // write to scratch, then copy result back.
                let scratch_copy = output[..n].to_vec();
                effect.process(&scratch_copy, &mut self.scratch[..n], sample_rate);
                output[..n].copy_from_slice(&self.scratch[..n]);
            }
        }

        // Apply wet/dry mix if not fully wet.
        if (self.wet_dry - 1.0_f32).abs() > f32::EPSILON {
            let wet = self.wet_dry;
            let dry = 1.0 - wet;
            for (i, out) in output[..n].iter_mut().enumerate() {
                *out = dry * input[i] + wet * (*out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock effect that scales all samples by `gain`.
    struct GainEffect {
        gain: f32,
        bypassed: bool,
    }

    impl GainEffect {
        fn new(gain: f32) -> Box<Self> {
            Box::new(Self {
                gain,
                bypassed: false,
            })
        }
    }

    impl AudioEffect for GainEffect {
        fn process(&mut self, input: &[f32], output: &mut [f32], _sample_rate: u32) {
            for (o, &i) in output.iter_mut().zip(input.iter()) {
                *o = i * self.gain;
            }
        }
        fn set_param(&mut self, param: &str, value: f32) {
            if param == "gain" {
                self.gain = value;
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

    /// Mock effect with 512-sample latency.
    struct LatencyEffect {
        bypassed: bool,
    }

    impl AudioEffect for LatencyEffect {
        fn process(&mut self, input: &[f32], output: &mut [f32], _sample_rate: u32) {
            output.copy_from_slice(input);
        }
        fn set_param(&mut self, _param: &str, _value: f32) {}
        fn bypass(&self) -> bool {
            self.bypassed
        }
        fn set_bypass(&mut self, bypass: bool) {
            self.bypassed = bypass;
        }
        fn latency_samples(&self) -> u32 {
            512
        }
    }

    fn make_chain() -> EffectChain {
        EffectChain::new(1024)
    }

    #[test]
    fn empty_chain_passes_through() {
        let mut chain = make_chain();
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        assert_eq!(output, input);
    }

    #[test]
    fn single_gain_effect_doubles_signal() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(2.0), 64).unwrap();
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        for &s in &output {
            assert!((s - 1.0).abs() < 1e-6, "expected 1.0, got {s}");
        }
    }

    #[test]
    fn chained_gains_multiply() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(2.0), 64).unwrap();
        chain.push_effect(GainEffect::new(3.0), 64).unwrap();
        let input = vec![1.0_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        // 1.0 * 2.0 * 3.0 = 6.0
        for &s in &output {
            assert!((s - 6.0).abs() < 1e-5, "expected 6.0, got {s}");
        }
    }

    #[test]
    fn bypassed_effect_is_skipped() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(2.0), 64).unwrap();
        chain.set_bypass(0, true).unwrap();
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        // Bypass skips the gain, so output == input
        assert_eq!(output, input);
    }

    #[test]
    fn chain_bypass_skips_all_effects() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(100.0), 64).unwrap();
        chain.set_chain_bypass(true);
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        assert_eq!(output, input);
    }

    #[test]
    fn wet_dry_zero_is_fully_dry() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(100.0), 64).unwrap();
        chain.set_wet_dry(0.0);
        let input = vec![0.5_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        // wet=0 means output should equal input
        for (o, i) in output.iter().zip(input.iter()) {
            assert!((o - i).abs() < 1e-5, "expected {i}, got {o}");
        }
    }

    #[test]
    fn wet_dry_half_mixes_equally() {
        let mut chain = make_chain();
        // gain=3 → output = 3.0 for input 1.0
        chain.push_effect(GainEffect::new(3.0), 64).unwrap();
        chain.set_wet_dry(0.5);
        let input = vec![1.0_f32; 64];
        let mut output = vec![0.0_f32; 64];
        chain.process(&input, &mut output, 48000);
        // 0.5 * 1.0 (dry) + 0.5 * 3.0 (wet) = 2.0
        for &s in &output {
            assert!((s - 2.0).abs() < 1e-5, "expected 2.0, got {s}");
        }
    }

    #[test]
    fn chain_too_long_returns_error() {
        let mut chain = make_chain();
        for _ in 0..MAX_CHAIN_LEN {
            chain.push_effect(GainEffect::new(1.0), 64).unwrap();
        }
        let result = chain.push_effect(GainEffect::new(1.0), 64);
        assert!(matches!(result, Err(EffectsError::ChainTooLong { .. })));
    }

    #[test]
    fn remove_effect_out_of_range_returns_error() {
        let mut chain = make_chain();
        let result = chain.remove_effect(0);
        assert!(matches!(result, Err(EffectsError::IndexOutOfRange { .. })));
    }

    #[test]
    fn remove_effect_shrinks_chain() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(2.0), 64).unwrap();
        chain.push_effect(GainEffect::new(3.0), 64).unwrap();
        chain.remove_effect(0).unwrap();
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn total_latency_sums_non_bypassed() {
        let mut chain = make_chain();
        chain
            .push_effect(Box::new(LatencyEffect { bypassed: false }), 64)
            .unwrap();
        chain
            .push_effect(Box::new(LatencyEffect { bypassed: true }), 64)
            .unwrap();
        // Only first effect contributes (second is bypassed)
        assert_eq!(chain.total_latency_samples(), 512);
    }

    #[test]
    fn total_latency_zero_when_chain_bypassed() {
        let mut chain = make_chain();
        chain
            .push_effect(Box::new(LatencyEffect { bypassed: false }), 64)
            .unwrap();
        chain.set_chain_bypass(true);
        assert_eq!(chain.total_latency_samples(), 0);
    }

    #[test]
    fn all_effects_bypassed_true_when_empty() {
        let chain = make_chain();
        assert!(chain.all_effects_bypassed());
    }

    #[test]
    fn all_effects_bypassed_false_when_one_active() {
        let mut chain = make_chain();
        chain.push_effect(GainEffect::new(1.0), 64).unwrap();
        assert!(!chain.all_effects_bypassed());
    }

    #[test]
    fn set_param_out_of_range_returns_error() {
        let mut chain = make_chain();
        let result = chain.set_param(0, "gain", 1.0);
        assert!(matches!(result, Err(EffectsError::IndexOutOfRange { .. })));
    }
}
```

- [ ] **Step 5.2: Run tests to verify they fail (chain.rs doesn't compile yet)**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test effects 2>&1 | head -30
```

Expected: FAIL — compilation errors (module doesn't compile yet).

- [ ] **Step 5.3: Fix the process() implementation — remove allocating to_vec() calls**

The ping-pong approach in `process()` above has a flaw: `to_vec()` allocates. Replace with a cleaner non-allocating approach. The `scratch` buffer is pre-allocated, so we can use it as the intermediate target. Here is the corrected `process()` method that replaces the one above:

```rust
    pub fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());

        let passthrough = self.chain_bypass || self.all_effects_bypassed();

        if passthrough {
            output.copy_from_slice(input);
            return;
        }

        let n = input.len();
        // Ensure scratch is large enough (should never reallocate in hot path).
        if self.scratch.len() < n {
            self.scratch.resize(n, 0.0);
        }

        // Strategy: use `output` as the working buffer, `scratch` as temp.
        // Initialize output with a copy of input.
        output[..n].copy_from_slice(&input[..n]);

        for effect in &mut self.effects {
            if effect.bypass() {
                continue;
            }
            // Copy current output into scratch, run effect from scratch → output.
            self.scratch[..n].copy_from_slice(&output[..n]);
            // SAFETY: scratch and output are separate slices (different fields).
            // This is sound — we pass scratch as input and output as &mut output.
            let scratch_slice = &self.scratch[..n];
            // We cannot pass scratch_slice and output[..n] to a trait method
            // that takes (&[f32], &mut [f32]) simultaneously because Rust's
            // borrow checker sees `self.scratch` and `output` both involve `self`.
            // Workaround: use raw pointer for scratch read, safe because:
            // 1. `scratch` and `output` are distinct allocations (separate Vec).
            // 2. We do not write to `scratch` inside this scope.
            let scratch_ptr = scratch_slice.as_ptr();
            let scratch_len = scratch_slice.len();
            let scratch_ref: &[f32] =
                // SAFETY: pointer from a live Vec, length bounded by Vec len.
                unsafe { std::slice::from_raw_parts(scratch_ptr, scratch_len) };
            effect.process(scratch_ref, &mut output[..n], sample_rate);
        }

        // Apply wet/dry mix if not fully wet.
        if (self.wet_dry - 1.0_f32).abs() > f32::EPSILON {
            let wet = self.wet_dry;
            let dry = 1.0 - wet;
            for (i, out) in output[..n].iter_mut().enumerate() {
                *out = dry * input[i] + wet * (*out);
            }
        }
    }
```

**IMPORTANT NOTE ON UNSAFE:** The `unsafe` block above is sound because `self.scratch` and `output` are different memory locations (distinct Vec allocations). However, since the CLAUDE.md says "no unsafe unless absolutely necessary," consider this alternative that avoids unsafe by making `EffectChain` own a second intermediate buffer:

Actually, the cleanest solution that avoids unsafe AND avoids per-call alloc is to split the borrow:

```rust
    pub fn process(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32) {
        debug_assert_eq!(input.len(), output.len());

        if self.chain_bypass || self.all_effects_bypassed() {
            output.copy_from_slice(input);
            return;
        }

        let n = input.len();
        if self.scratch.len() < n {
            self.scratch.resize(n, 0.0);
        }

        // Copy input → scratch as the initial working buffer.
        self.scratch[..n].copy_from_slice(&input[..n]);

        for effect in &mut self.effects {
            if effect.bypass() {
                continue;
            }
            // Copy scratch → output (prev result), run effect scratch_prev → scratch_new.
            // But we only have one scratch buffer. Strategy: effect reads from output
            // (where we just copied to), writes to scratch. Then swap.
            output[..n].copy_from_slice(&self.scratch[..n]);
            // Now: output holds the accumulated signal, scratch is free.
            // We need to give effect (&output[..n], &mut scratch[..n]).
            // Borrow checker: output is `&mut [f32]` passed in, scratch is `self.scratch`.
            // These are different allocations — borrow checker is fine.
            effect.process(&output[..n], &mut self.scratch[..n], sample_rate);
        }

        // Copy final result from scratch to output.
        output[..n].copy_from_slice(&self.scratch[..n]);

        // Apply wet/dry mix.
        if (self.wet_dry - 1.0_f32).abs() > f32::EPSILON {
            let wet = self.wet_dry;
            let dry = 1.0 - wet;
            for (i, out) in output[..n].iter_mut().enumerate() {
                *out = dry * input[i] + wet * (*out);
            }
        }
    }
```

Use this final version — no unsafe, no per-call alloc, sound logic. Write the full `chain.rs` file using this `process()` implementation with the tests from Step 5.1.

- [ ] **Step 5.4: Run tests to verify they pass**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test 2>&1 | grep -E "^(test |FAILED|ok|error)"
```

Expected: all tests pass, including the new effects tests.

- [ ] **Step 5.5: Run clippy**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 5.6: Commit**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
git add src/audio/effects/
git commit -m "feat(audio/effects): AudioEffect trait + EffectChain + EffectsCommand/Event"
```

---

## Task 6: Wire effects module into `src/audio/mod.rs`

**Files:**
- Modify: `src/audio/mod.rs`

- [ ] **Step 6.1: Add module declarations**

Replace the entire contents of `src/audio/mod.rs` with:

```rust
mod confd;
mod decoder;
mod engine;
mod error;
pub mod effects;
pub mod mixer;
pub mod playback;
mod registry;
pub mod streams;

pub use decoder::{decode, DecodedAudio};
pub use effects::{AudioEffect, EffectChain, EffectsCommand, EffectsEvent};
pub use engine::{spawn, AudioCommand, AudioEvent, AudioHandle};
pub use error::{AudioError, EffectsError, WatcherError};
pub use streams::{Direction, StreamEvent, StreamWatcher};
```

- [ ] **Step 6.2: Build to check for compilation errors**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: errors about `mixer` module not existing yet (if any). Proceed to Task 7.

---

## Task 7: Create `src/audio/mixer.rs` stub (integration seam)

**Files:**
- Create: `src/audio/mixer.rs`

- [ ] **Step 7.1: Write the failing test**

Create `src/audio/mixer.rs`:

```rust
//! Mixer: integration seam between mic capture and virtual sink write.
//!
//! In the current architecture (ADR-007: Links-Only routing), HonkHonk uses
//! PipeWire graph links for mic routing — there is no application-level mic
//! capture buffer. This module is the integration stub for Phase 4B voice
//! effects. When effects are wired in (PR 2+), a `pw_stream` process callback
//! will call [`Mixer::process_block`] on each captured mic buffer before writing
//! it to the virtual sink.
//!
//! Until then, `process_block` is a transparent passthrough.

use crate::audio::effects::EffectChain;

/// Holds the effect chain and applies it to mic audio blocks.
///
/// Instantiated once per audio engine session. The [`EffectChain`] inside
/// is populated by [`crate::audio::engine`] in response to [`EffectsCommand`]s.
pub struct Mixer {
    chain: EffectChain,
    /// Pre-allocated output buffer for `process_block`. Avoids per-call alloc.
    output_buf: Vec<f32>,
    output_capacity: usize,
}

impl Mixer {
    /// Create a new `Mixer` with an empty effect chain.
    pub fn new(initial_block_size: usize) -> Self {
        Self {
            chain: EffectChain::new(initial_block_size),
            output_buf: vec![0.0_f32; initial_block_size],
            output_capacity: initial_block_size,
        }
    }

    /// Returns a mutable reference to the effect chain.
    ///
    /// Used by `engine.rs` to apply `EffectsCommand`s.
    pub fn chain_mut(&mut self) -> &mut EffectChain {
        &mut self.chain
    }

    /// Process a block of mic audio through the effect chain.
    ///
    /// Returns a slice into the internal output buffer. Caller copies this
    /// to the virtual sink's input buffer.
    ///
    /// No-op passthrough when chain is empty or bypassed. Real-time safe.
    pub fn process_block(&mut self, input: &[f32], sample_rate: u32) -> &[f32] {
        let n = input.len();
        if n > self.output_capacity {
            self.output_buf.resize(n, 0.0);
            self.output_capacity = n;
        }
        self.chain
            .process(input, &mut self.output_buf[..n], sample_rate);
        &self.output_buf[..n]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_new_is_empty_passthrough() {
        let mut mixer = Mixer::new(64);
        let input = vec![0.25_f32; 64];
        let output = mixer.process_block(&input, 48000);
        assert_eq!(output, input.as_slice());
    }

    #[test]
    fn mixer_chain_mut_accessible() {
        let mut mixer = Mixer::new(64);
        let chain = mixer.chain_mut();
        assert!(chain.is_empty());
    }

    #[test]
    fn mixer_process_block_returns_correct_length() {
        let mut mixer = Mixer::new(64);
        let input = vec![1.0_f32; 32];
        let output = mixer.process_block(&input, 48000);
        assert_eq!(output.len(), 32);
    }
}
```

- [ ] **Step 7.2: Run tests to verify they pass**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test mixer 2>&1
```

Expected: 3 tests PASS.

- [ ] **Step 7.3: Run clippy**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 7.4: Run full test suite**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test 2>&1 | tail -20
```

Expected: all existing tests pass, new tests pass.

- [ ] **Step 7.5: Commit**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
git add src/audio/mixer.rs src/audio/mod.rs
git commit -m "feat(audio/mixer): integration stub for Phase 4B effect chain"
```

---

## Task 8: Add `EffectsCommand`/`EffectsEvent` variants to `engine.rs`

**Files:**
- Modify: `src/audio/engine.rs`

- [ ] **Step 8.1: Write failing test for new command variant**

In `src/audio/engine.rs`, inside the existing `#[cfg(test)] mod tests` block, add:

```rust
    #[test]
    fn audio_command_set_effect_bypass_is_constructible() {
        let _ = AudioCommand::SetEffectBypass { index: 0, bypass: true };
    }

    #[test]
    fn audio_command_set_effect_wet_dry_is_constructible() {
        let _ = AudioCommand::SetEffectWetDry(0.5);
    }

    #[test]
    fn audio_event_effects_latency_changed_is_constructible() {
        let _ = AudioEvent::EffectsLatencyChanged(512);
    }
```

- [ ] **Step 8.2: Run test to verify it fails**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test audio_command_set_effect 2>&1 | head -20
```

Expected: FAIL — variants not defined.

- [ ] **Step 8.3: Add variants to `AudioCommand` and `AudioEvent` enums**

In `src/audio/engine.rs`, add to the `AudioCommand` enum (after `Shutdown`):

```rust
    /// Set bypass state for the effect at `index` in the mixer chain.
    SetEffectBypass { index: usize, bypass: bool },
    /// Set a parameter on the effect at `index`.
    SetEffectParam { index: usize, param: String, value: f32 },
    /// Set the chain-level wet/dry mix.
    SetEffectWetDry(f32),
    /// Set chain-level bypass.
    SetEffectChainBypass(bool),
```

Add to the `AudioEvent` enum (after `SourceFirstRun`):

```rust
    /// The effect chain's total latency changed (in samples).
    EffectsLatencyChanged(u32),
```

Add a `Mixer` field to `EngineCtx` and instantiate it in `run_engine`:

In `EngineCtx` struct, add:
```rust
    mixer: std::rc::Rc<std::cell::RefCell<crate::audio::mixer::Mixer>>,
```

In `run_engine`, after `let engine_volume: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));`, add:
```rust
    let mixer = Rc::new(RefCell::new(crate::audio::mixer::Mixer::new(4096)));
```

Update `EngineCtx` construction to include `mixer: mixer.clone()`.

Add match arms in the command listener (`_cmd_listener`):

```rust
        AudioCommand::SetEffectBypass { index, bypass } => {
            let _ = ctx.mixer.borrow_mut().chain_mut().set_bypass(index, bypass);
        }
        AudioCommand::SetEffectParam { index, param, value } => {
            let _ = ctx.mixer.borrow_mut().chain_mut().set_param(index, &param, value);
        }
        AudioCommand::SetEffectWetDry(wet_dry) => {
            ctx.mixer.borrow_mut().chain_mut().set_wet_dry(wet_dry);
        }
        AudioCommand::SetEffectChainBypass(bypass) => {
            ctx.mixer.borrow_mut().chain_mut().set_chain_bypass(bypass);
        }
```

- [ ] **Step 8.4: Run tests to verify they pass**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo test 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 8.5: Run clippy**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 8.6: Run full build**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
cargo build --release 2>&1 | tail -5
```

Expected: compiles successfully.

- [ ] **Step 8.7: Commit**

```bash
cd /home/adam/github/honkhonk/.worktrees/feat/issue-30
git add src/audio/engine.rs
git commit -m "feat(audio/engine): wire EffectsCommand/EffectsEvent variants into AudioCommand/AudioEvent"
```

---

## Spec Coverage Check

| Acceptance Criterion | Task |
|---|---|
| `src/audio/effects/mod.rs` — `AudioEffect` trait, `EffectChain` re-export | Task 3, 5, 6 |
| `src/audio/effects/chain.rs` — composable pipeline | Task 5 |
| `AudioEffect` trait: `process()`, `set_param()`, `bypass()`, `set_bypass()`, `latency_samples()` | Task 3 |
| `EffectChain` processes buffers through ordered list | Task 5 |
| Bypass: zero-copy passthrough when all effects bypassed | Task 5 |
| Wet/dry mix parameter on chain | Task 5 |
| `EffectsError` variants | Task 1 |
| Unit tests: chain with mock effects, bypass, wet/dry | Task 5 |
| Integration point in `mixer.rs` | Task 7 |
| `EffectsCommand` / `EffectsEvent` enums | Task 4, 8 |

All criteria covered.
