# Issue #8: Search Bar + Volume Controls + Now-Playing Bar

## Status: Approved

## Context

Phase 1 MVP has a working sound grid with click-to-play and category filtering. Users need to find sounds quickly (search), control volume, and see what's currently playing with progress feedback. Reference mockup in `docs/design-reference/honkhonk-direction-c.jsx` (lines 256-431) defines the visual layout.

## Architecture

### New Files

| File | Responsibility |
|------|---------------|
| `src/ui/search_bar.rs` | Pill-shaped `text_input` widget, emits `Message::SearchChanged(String)` |
| `src/ui/volume.rs` | Iced `slider` widget + percentage text label, emits `Message::VolumeChanged(f32)` |
| `src/ui/now_playing.rs` | Bottom bar composing: placeholder circle (sticker slot), sound name + "HONKING NOW" label, progress bar, volume slider |

### Modified Files

| File | Changes |
|------|---------|
| `src/app.rs` | New state fields (`search_query: String`, `progress: f32`). New Message variants (`SearchChanged`, `VolumeChanged`, `AudioEvent::Progress`). Search filtering in `view()`. Volume persistence on change. Compose now-playing bar below scrollable grid. |
| `src/audio/playback.rs` | Add `pub fn progress(&self) -> f32` method (cursor / total samples). |
| `src/audio/engine.rs` | Emit `AudioEvent::Progress(f32)` from process callback, throttled to every ~4800 samples (100ms at 48kHz). |
| `src/ui/mod.rs` | Add `pub mod search_bar; pub mod volume; pub mod now_playing;` |

## Data Flow

### Search
```
text_input onChange → Message::SearchChanged(query)
  → app.search_query = query
  → view() filters sounds: category filter AND case-insensitive substring match on name
```

### Volume
```
slider onChange → Message::VolumeChanged(f32)
  → app.config.volume = value
  → audio.send(AudioCommand::SetVolume(value))
  → config.save() persists to disk
```

### Progress
```
PipeWire process callback → cursor advances → every ~4800 samples:
  → evt_tx.send(AudioEvent::Progress(cursor as f32 / total as f32))
    → TrayPoll loop picks up event
      → app.progress = value
        → now-playing bar re-renders with updated progress width
```

Progress resets to 0.0 on `PlaybackFinished`.

## Component Details

### Search Bar (`src/ui/search_bar.rs`)

- Pill-shaped container (radius::PILL border)
- Confetti panel background with hairline border
- Placeholder text: "Find a sound…"
- Positioned in header row between title and "Stop all" button
- Filter applied in `view()` — no debounce needed (Iced re-renders are cheap)

### Volume Slider (`src/ui/volume.rs`)

- Iced `slider` widget, range 0.0..=1.0, step 0.01
- Accent-colored fill
- Percentage label (e.g., "85%") right of slider
- Returns `Element<Message>` composing icon + slider + label in a row

### Now-Playing Bar (`src/ui/now_playing.rs`)

- Fixed at bottom of window, panel background, hairline top border
- Left side: placeholder circle (44px, panel-deep background) → future sticker slot
- Center-left: sound name (bold) + "HONKING NOW · {category}" subtitle
- Center: progress bar (6px tall, max-width 320px, accent fill proportional to progress)
- Right side (margin-left auto): volume icon + volume slider (140px) + percentage
- Hidden when nothing is playing (returns empty container)

## Messages

New variants added to `Message` enum:
```rust
SearchChanged(String),
VolumeChanged(f32),
```

New variant added to `AudioEvent` enum:
```rust
Progress(f32),
```

## State Changes in `HonkHonk`

```rust
pub struct HonkHonk {
    // ... existing fields ...
    search_query: String,  // NEW
    progress: f32,         // NEW: 0.0..1.0
}
```

## Progress Throttling

In `engine.rs`, track samples since last progress event:
```rust
let samples_per_progress = (state.sample_rate() as usize * state.channels() as usize) / 10;
// Emit progress every ~100ms worth of samples
```

Use a counter in the process closure. Increment by frames written. When counter exceeds threshold, emit progress and reset counter.

## Testing

### Unit Tests
- `app.rs`: SearchChanged updates search_query, VolumeChanged updates config.volume, Progress updates progress field, PlaybackFinished resets progress
- `playback.rs`: progress() returns correct ratio at various cursor positions

### Integration
- Search filtering: sounds filtered by substring match combined with active category
- Volume persistence: VolumeChanged triggers config save

## Out of Scope

- Sticker/glyph thumbnails in now-playing (Phase 3, issue #13)
- Favorites star (Phase 3, issue #14)
- Per-sound volume (Phase 3, issue #14)
- Settings panel (Phase 2, issue #11)
- Waveform visualization (not in issue #8 acceptance criteria)

## Estimated LOC

~250-300 lines new code across 3 new files + modifications. Within 500 LOC PR limit.
