# ADR-007: Links-Only Routing for App Source Patching

## Status: Accepted

## Context

HonkHonk Phase 4A routes audio from other applications (browser, Spotify, games) into the virtual mic. Two architectural approaches exist:

### Option A: Links-Only (chosen)

Create PipeWire links from app output ports directly to the virtual sink's input ports. PipeWire mixes multiple inputs natively — no application-side mixer code.

```
[Browser FL] ──link──▶ [HonkHonk Mix FL]
[Spotify FL] ──link──▶ [HonkHonk Mix FL]   ← PipeWire sums these
[Mic FL]     ──link──▶ [HonkHonk Mix FL]
```

### Option B: Filter Node

Create a `pw_filter` node with explicit input/output ports. Process audio in a real-time callback, handling mixing, per-source volume, and DSP.

**Problem:** `pipewire-rs` v0.9 does not expose safe `pw_filter` bindings. Only `pipewire_sys` has raw C FFI. Using it means unsafe code, manual memory management, and maintaining our own bindings.

## Decision

Use **Links-Only routing** (Option A).

Rationale:
1. **PipeWire handles mixing natively** — linking multiple outputs to one input port sums them automatically. Zero application DSP code for the routing feature.
2. **Simpler implementation** — creating/destroying links via `core.create_object::<Link>("link-factory", ...)` is ~20 lines per link vs. hundreds of lines for a filter node with port management.
3. **No unsafe code** — `pipewire-rs` fully supports `create_object`, registry watching, and link management with safe APIs.
4. **Same pattern as proven tools** — pipewire-soundpad, Helvum, and pipewire-screenaudio all use this approach successfully.
5. **Latency** — link routing adds zero processing latency (PipeWire graph handles it).

### Per-Source Volume (Future)

Links-only doesn't provide per-source volume at the mix point. Two options when this becomes needed:
- Set volume on the source node itself (PipeWire supports per-node volume via `pw_node_set_param`)
- Upgrade to a filter node approach when `pipewire-rs` gains `pw_filter` bindings

This is acceptable because per-source volume is a Phase 4A sub-issue (PR 4 of 4), not a launch blocker.

### Transient Stream Lifecycle (Important Constraint)

PipeWire stream nodes are transient — their lifetime is controlled by the application, not PipeWire. Links-only routing inherits this constraint: **when a source stream is destroyed, all links to it die automatically.**

| App behavior | Stream lifecycle | Link impact |
|---|---|---|
| PulseAudio apps pausing (`pa_stream_cork`) | Node stays in graph (corked/paused) | Links survive |
| ALSA apps closing PCM handle (`snd_pcm_close`) | Node destroyed, new ID on reopen | Links die |
| App restart, tab close, PA `stream_disconnect` | Node destroyed | Links die |
| Firefox/Spotify orphan streams | Nodes accumulate (never cleaned) | Links survive |

**Mitigation:** The router must maintain **persistent route intent** keyed by stable app identity (`application.name`, `application.process.binary`, PID) rather than PipeWire `object.id`. On every `global` registry event for a matching `Stream/Output/Audio` node, auto-create links. On `global_remove`, clean up link references but preserve intent.

This is the same pattern used by venmic (Vencord) and pipewire-screenaudio. Helvum does NOT auto-reconnect (manual patchbay only — not a model for this use case).

See #17 for full stream lifecycle analysis and #27 for the persistent route intent implementation.

## Consequences

- App source routing requires only `pipewire-rs` (already a dependency) — no new crates
- All routing is safe Rust via `core.create_object` and `registry.destroy_global`
- `object.linger = false` ensures links auto-cleanup when HonkHonk exits
- Must connect with `"media.category" => "Manager"` property to enable `create_object` — this is a requirement for engine.rs regardless (needed for virtual device creation)
- Hot-plug handled via registry `global` / `global_remove` listeners — same pattern already planned for engine.rs
- **Stream transience requires auto-reconnect logic** — router must match new streams by app identity and re-link automatically (see "Transient Stream Lifecycle" section above)
- If `pipewire-rs` adds `pw_filter` support in future, we can migrate the mixer to a filter node for per-source volume/DSP without changing the graph.rs/router.rs module boundary
