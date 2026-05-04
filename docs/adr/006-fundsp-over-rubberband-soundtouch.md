# ADR-006: fundsp + pitch_shift for Voice Effects DSP

## Status: Accepted

## Context

HonkHonk Phase 4B adds real-time voice effects (pitch shifting, robot voice, reverb, formant manipulation). We need a DSP library stack that:

1. Processes audio buffers in real-time with <10ms added latency
2. Is composable — chain multiple effects (pitch → filter → reverb)
3. Has a license compatible with MIT
4. Works with our existing architecture: capture mic via `pw_stream`, process in Rust, write to virtual sink via `pw_stream`

`pipewire-rs` does not expose `pw_filter` bindings, so all DSP must run in application code on captured audio buffers.

### Options Evaluated

| Library | License | Rust-native | Real-time safe | Composable | Notes |
|---------|---------|-------------|----------------|------------|-------|
| **fundsp 0.23** | MIT | Yes | Yes (stack-alloc, SIMD) | Yes (operator chains) | 20+ filter types, FFT resynth, reverb. No built-in pitch shifter |
| **pitch_shift 2.1** | MIT | Yes | Yes (fixed buffers) | No (standalone) | Phase vocoder, 128-sample blocks. Drop-in pitch shifting |
| **RubberBand** | GPL-2.0+ | No (C++ FFI) | Yes | No | Excellent quality but GPL incompatible with MIT project |
| **SoundTouch** | LGPL-2.1 | No (C++ FFI) | Yes | No | Workable license (dynamic link) but unnecessary C++ dependency |
| **tdpsola** | AGPL-3.0 | Yes | No (planned) | No | AGPL incompatible. Algorithm (TD-PSOLA) worth implementing from scratch |
| **surgefx-vocoder** | GPL-3.0 | Yes | Unknown | No | 15 internal Surge deps. GPL. Dead project |
| **Signalsmith Stretch** | MIT | No (C++ via cxx) | Yes | No | High-quality pitch shifting. Upgrade path for wide-range shifts |
| **Faust DSP** | Generated code: free | Compile-time | Yes | Yes (Faust lang) | Powerful but adds build toolchain complexity (Faust compiler) |

## Decision

Use **fundsp** as the core DSP framework and **pitch_shift** for quick-start pitch effects.

- **fundsp** provides the composable effect chain: filters, ring modulation, reverb, FFT resynth (for custom phase vocoder / formant manipulation). Operator-based composition (`>>` chain, `|` parallel, `&` mix) maps naturally to our `EffectChain` trait.
- **pitch_shift** provides a drop-in phase vocoder for Tier 1 pitch presets (deep/chipmunk/anonymous). Ships faster than building a pitch shifter from fundsp primitives.
- **Signalsmith Stretch** (MIT, C++ via ssstretch) is the upgrade path if pitch_shift quality proves insufficient for wide pitch ranges.

Both are MIT-licensed, pure Rust (or MIT C++ with Rust bindings), and real-time safe.

## Consequences

- All voice effects are pure Rust — no C/C++ runtime dependencies for Phase 4B
- fundsp's `resynth` opcode enables Tier 3 formant-aware effects without external libraries
- pitch_shift adds ~21ms latency at 48kHz (1024-sample window) — acceptable for voice chat
- If formant preservation quality is insufficient, we can implement TD-PSOLA from scratch (algorithm is public domain, only the `tdpsola` crate is AGPL)
- No dependency on PipeWire's filter chain module or LADSPA/LV2 plugins — simpler deployment
