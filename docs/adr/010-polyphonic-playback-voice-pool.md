# ADR-010: Polyphonic Playback Voice Pool

## Status: Accepted

## Context

HonkHonk's playback path was monophonic: every new tile press displaced the
current `ActivePlayback`, and the engine emitted `PlaybackFinished` for the
old generation before starting the new one. That matched the early soundboard
MVP, but it blocks macros and the restored overlap mode because several
internal HonkHonk sounds must be able to play at the same time.

The existing PipeWire router and `Mixer` are for external mic/app passthrough
into the virtual mic. They do not solve internal soundboard voice summing.
Creating one PipeWire stream per sound would also bring back node churn and
make per-voice completion accounting harder.

## Decision

Use a bounded in-process voice pool for decoded soundboard playback.

- Each play command carries a caller-assigned `voice_id`, a generation, gain,
  mode, and an `EffectSettings` snapshot.
- `PlayMode::Concurrent` adds a voice to the pool. `PlayMode::Interrupt`
  first finishes every active voice, preserving the old monophonic behavior.
- The pool is capped at `MAX_VOICES` and steals the oldest voice when full,
  emitting one `PlaybackFinished` for the stolen voice.
- Sink and monitor playback use shared PipeWire streams. Their callbacks ask
  the pool to mix all active voices into the output block, clamp the sum, and
  drain naturally finished voices from the timer path.
- Each voice owns separate sink and monitor `PlaybackState`s plus separate
  `EffectChain`s, so per-voice effects do not leak across concurrent voices.
- The app owns `overlap_mode` in `AppConfig`. Tile presses map it to
  `PlayMode`; future macro playback always uses concurrent mode.

## Consequences

- Macros can schedule overlapping sounds without creating per-sound PipeWire
  nodes.
- `PlaybackFinished` now includes `voice_id` so completion is tracked per
  active voice while the app can continue using the generation guard for
  stale UI updates.
- A mixed-format play while voices are active falls back to interrupt behavior:
  active voices finish, then the shared streams rebuild for the new format.
  Future resampling/normalization work can restore true mixed-format overlap
  without replacing PipeWire streams under live voices.
- The external source mixer remains separate and continues to represent the
  ADR-007 mic/app routing path, not internal soundboard voice summing.
