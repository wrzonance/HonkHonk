# Slot Manager — Design Spec

**Date:** 2026-05-07  
**Issues:** #11 (partial), #12 (partial)  
**Phase:** 2 visual targets  
**PR scope:** View + unbind only. Sound assignment deferred to right-click context menu PR.

---

## Scope

Build the slot manager full-window view. Users can see all 20 slots, their bound sounds and hotkeys, select a slot to inspect it, and unbind a slot. Navigation uses a `ViewMode` enum — the slot manager replaces the main grid as a full-window swap.

**Out of scope this PR:**
- Assigning sounds from within the slot manager ("Pick a sound…")
- Drag-and-drop binding
- Hotkey rebinding UI
- Conflict detection (portal doesn't surface this at runtime)
- Settings panel (#11 full scope)
- Right-click context menu enrichment (#12 full scope)

---

## State Changes (`src/app.rs`)

```rust
enum ViewMode {
    Main,
    SlotManager,
}

// Added to HonkHonk struct:
view_mode: ViewMode,        // default: ViewMode::Main
selected_slot: Option<u8>,  // default: None
```

New `Message` variants:

```rust
Message::ShowSlots          // view_mode = SlotManager, selected_slot = None
Message::ShowMain           // view_mode = Main
Message::SelectSlot(u8)     // selected_slot = Some(i)
// ClearSlot(u8) already exists — no change needed
```

---

## Header Layout

Navigation buttons anchor **left**, near the title. Global action buttons anchor **right**. Search bridges them.

```
[HonkHonk]  [Slots]  ·····  [Search]  [Stop All]
```

- "Slots" button: panel-styled, renders only when `view_mode == Main`
- `on_press(Message::ShowSlots)`
- Stop All remains isolated on the right with natural position contrast — no extra separator needed

---

## `view()` Branching (`src/app.rs`)

```rust
pub fn view(&self) -> Element<'_, Message> {
    match self.view_mode {
        ViewMode::Main => self.view_main(),
        ViewMode::SlotManager => {
            let t = theme::Theme::Dark;
            slot_manager::view_slot_manager(
                &self.slots,
                &self.sounds,
                self.selected_slot,
                t,
            )
        }
    }
}
```

Existing `view()` body extracted into private `view_main()` — mechanical, no logic change.

---

## New Module: `src/ui/slot_manager.rs`

Public API matches `sound_grid.rs` pattern:

```rust
pub fn view_slot_manager<'a>(
    slots: &'a SlotMap,
    sounds: &'a [SoundEntry],
    selected_slot: Option<u8>,
    theme: Theme,
) -> Element<'a, Message>
```

### Layout

Three regions: header bar, 4×5 slot grid (flex 1), right sidebar (320px fixed).

### Header Bar

```
[← Back to sounds]  Slots  ·  X bound · Y with hotkey
```

- "← Back to sounds": panel button, `on_press(Message::ShowMain)`
- "Slots": italic, weight 800, size 22
- Stats: `{bound_count} bound · {hotkey_count} with hotkey` (inkDim, size 12)
- `bound_count` = slots with a path that resolves to a sound in the library
- `hotkey_count` = not computable without portal data — omit for now, show only `{bound_count} bound`

### Slot Grid (4 rows × 5 columns)

Each tile is 138px tall, `borderRadius: 18`, cursor pointer.

**Bound slot tile:**
- Background: tone-tinted (`hsl(hue sat% 93%)` light / `hsl(hue sat-min% 13%)` dark)
- Border: `1px solid hairline` normally; `2.5px solid ink` + accent glow when selected
- Top-left: slot number badge (`#01`–`#20`, monospace, inkFaint)
- Center: sticker placeholder circle (tone color, 48px) — full canvas sticker deferred to Phase 3
- Bottom: sound name (truncated, weight 800, size 11.5)
- Bottom: hotkey badge (monospace, `rgba(0,0,0,.07)` bg) or "no hotkey" dashed badge (inkFaint)

**Empty slot tile:**
- Background: `theme.panel`
- Border: `2px dashed hairline2`
- Center: "+" icon (22px, opacity 0.6) + "EMPTY" label (inkFaint, weight 700, size 10.5, letterspacing)

**Interaction:**
- Click any tile → `Message::SelectSlot(i)`

### Right Sidebar (320px)

Bordered left, `theme.panel` background.

**No slot selected:**
```
[placeholder text: "Select a slot to inspect it"]
```

**Selected — empty slot:**
```
SLOT #13

[goose placeholder, dashed border box]
"Slot is empty"
"Assign a sound via right-click on any sound tile"
```

**Selected — bound slot:**
```
SLOT #04

[sticker circle 56px]  Vine Boom
                       MEMES · 0:02

GLOBAL HOTKEY
[  F1  ]   (read-only monospace display)

PORTAL STATUS
● Registered via xdg-desktop-portal

[Unbind]
```

- Hotkey display: read-only monospace box, `theme.accent` border
- If no hotkey assigned: monospace box shows "—"
- Portal status: green dot + "Registered via xdg-desktop-portal" (always shown when slot manager is reachable — shortcuts are registered on app start)
- "Unbind" button: danger style (red border, red text), `on_press(Message::ClearSlot(i))`. After clear, `selected_slot` stays on the same index so user sees the now-empty panel.

---

## Data Flow

```
SlotMap::get(i)           → Option<PathBuf>   (bound path or empty)
library.sounds            → Vec<SoundEntry>   (to resolve name/category/duration/tone)
slot_for_path             → resolves sound metadata for display
```

Tone for sticker placeholder: look up `SoundEntry::tone` (already on `SoundEntry` from Phase 1). If path no longer in library, show slot as empty (same stale-slot guard logic already in `ShortcutActivated` handler).

---

## Tests (`src/app.rs` `#[cfg(test)]`)

```rust
show_slots_sets_view_mode()          // ShowSlots → view_mode == SlotManager, selected_slot == None
show_main_resets_view_mode()         // ShowMain → view_mode == Main
select_slot_sets_selected()          // SelectSlot(3) → selected_slot == Some(3)
clear_slot_keeps_selection_showing_empty_panel()  // ClearSlot(3) when selected_slot == Some(3) → selected_slot stays Some(3), slot now empty
```

No view-rendering tests (Iced framework responsibility).

---

## File Changes

| File | Change |
|------|--------|
| `src/app.rs` | Add `ViewMode` enum, two fields, four message arms, `view_main()` extraction, `view()` branch, header Slots button, tests |
| `src/ui/slot_manager.rs` | New module — full slot manager view |
| `src/ui/mod.rs` | `pub mod slot_manager;` |

Estimated LOC delta: ~350–420 lines.
