//! Pure row-building for the slot manager grid (#198).
//!
//! Unlike the Settings → Shortcuts bindings list
//! (`super::super::hotkeys::rows`), which only emits a row for a slot with a
//! bound trigger, the slot manager shows every fixed slot regardless of
//! binding — an unbound, unassigned slot still renders (as an empty tile),
//! so the grid's cardinality never shifts under filtering/sorting.
//! `build_slot_rows` is total: it always returns exactly one row per slot,
//! in slot order.

use std::cmp::Ordering;
use std::path::Path;

use super::HonkHonk;
use crate::app::slot_sort::SlotSortKey;
use crate::state::SlotContent;
use crate::ui::list_controls::sort::SortKey;

/// The fixed slot-map size. `state::slots::SLOT_COUNT` is private to that
/// module; this mirrors the `[Option<String>; 20]` literal already used for
/// `HonkHonk::slot_triggers`.
const SLOT_COUNT: u8 = 20;

/// One row in the slot manager grid: a slot index plus its resolved content,
/// if any. Fully owned (no lifetime param) so it can outlive the borrows
/// used to build it — `HonkHonk::slot_rows()` returns a fresh `Vec` on every
/// call rather than caching indices into `HonkHonk`.
///
/// Unlike `hotkeys::rows::HotkeyRow`, there is no placeholder/has-content
/// field: a dangling or empty slot resolves to blank strings and `None`
/// timestamps, which `filter_items` and `SortKey::value_unknown` already
/// route correctly without a dedicated flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SlotRow {
    pub(super) slot_index: u8,
    pub(super) display_name: String,
    pub(super) filename: String,
    pub(super) tag: String,
    pub(super) duration_ms: Option<u64>,
    pub(super) modified_ms: Option<u64>,
    pub(super) added_ms: Option<u64>,
}

/// Builds one row per slot, in slot order — always `SLOT_COUNT` rows,
/// regardless of whether a slot has content assigned.
pub(super) fn build_slot_rows(state: &HonkHonk) -> Vec<SlotRow> {
    (0..SLOT_COUNT).map(|idx| slot_row(state, idx)).collect()
}

/// Resolves the content assigned to `idx`, if any, into a row. The three
/// match arms are the only three cases `SlotMap::content` can return.
fn slot_row(state: &HonkHonk, idx: u8) -> SlotRow {
    match state.slots.content(idx) {
        Some(SlotContent::Sound(path)) => sound_slot_row(state, idx, path),
        Some(SlotContent::Macro(id)) => macro_slot_row(state, idx, id),
        None => empty_slot_row(idx),
    }
}

/// A slot bound to a sound path. The path may no longer resolve to a
/// library entry (file deleted/moved outside the app) — that's a dangling
/// row, collapsing to the fully blank [`empty_slot_row`] shape rather than a
/// partial one, so every "nothing to show here" case shares one haystack.
fn sound_slot_row(state: &HonkHonk, idx: u8, path: &Path) -> SlotRow {
    let Some(sound) = state.sounds.iter().find(|entry| entry.path == path) else {
        return empty_slot_row(idx);
    };
    let custom_name = state
        .sound_meta
        .get_ref(&sound.id)
        .and_then(|meta| meta.display_name.as_deref());
    SlotRow {
        slot_index: idx,
        display_name: resolved_display_name(custom_name, &sound.name),
        filename: file_name(path),
        tag: sound.category.clone(),
        duration_ms: sound.duration_ms,
        modified_ms: sound.modified_ms,
        added_ms: state.sound_meta.added_ms(&sound.id),
    }
}

/// A slot bound to a macro id. A deleted macro is a dangling row too — it
/// collapses to the fully blank `empty_slot_row` shape, same as a dangling
/// sound reference.
///
/// The name comes from `slot_manager::display_name`, the same helper the tile
/// and sidebar render — `MacroStore` accepts a blank or whitespace-only name,
/// which those surfaces show as "Untitled macro". Deriving the row's name any
/// other way would make the grid searchable and sortable by a value the user
/// cannot see: querying the visible label would match nothing, and a
/// whitespace-only name would sort ahead of every real one.
fn macro_slot_row(state: &HonkHonk, idx: u8, id: &str) -> SlotRow {
    let Some(entry) = state.macros.get(id) else {
        return empty_slot_row(idx);
    };
    SlotRow {
        slot_index: idx,
        display_name: crate::ui::slot_manager::display_name(entry).to_owned(),
        ..empty_slot_row(idx)
    }
}

/// A slot with no content assigned, or one whose content no longer resolves.
/// Every field but `slot_index` is blank/`None` — the shared "nothing here"
/// value `SortKey::value_unknown` and `filter_items` both already handle.
fn empty_slot_row(idx: u8) -> SlotRow {
    SlotRow {
        slot_index: idx,
        display_name: String::new(),
        filename: String::new(),
        tag: String::new(),
        duration_ms: None,
        modified_ms: None,
        added_ms: None,
    }
}

/// `raw`, if present and non-empty, else `fallback`. Guards against an
/// empty custom name/macro name being mistaken for "nothing here".
fn resolved_display_name(raw: Option<&str>, fallback: &str) -> String {
    match raw {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => fallback.to_owned(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The fields a type-to-filter query matches against: name, file name, tag.
pub(super) fn slot_haystacks(row: &SlotRow) -> [&str; 3] {
    [&row.display_name, &row.filename, &row.tag]
}

impl SortKey<SlotRow> for SlotSortKey {
    fn compare(self, left: &SlotRow, right: &SlotRow) -> Ordering {
        match self {
            // Slot index *is* the primary key here, so it must fully
            // reverse under `Direction::Descending` like any other primary
            // key — it does not go through `tie_break()` (which never
            // reverses), and it never ties (slot indices are unique), so
            // `tie_break()` is never reached for this variant anyway.
            Self::SlotNumber => left.slot_index.cmp(&right.slot_index),
            Self::Name => left
                .display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase()),
            Self::Length => left.duration_ms.cmp(&right.duration_ms),
            Self::Tag => left.tag.to_lowercase().cmp(&right.tag.to_lowercase()),
            Self::Modified => left.modified_ms.cmp(&right.modified_ms),
            Self::Added => left.added_ms.cmp(&right.added_ms),
        }
    }

    /// Every row has a unique `slot_index`, so this tie-break is total: two
    /// distinct rows sharing a primary value (e.g. the same `Tag`) are
    /// never `Ordering::Equal` all the way through. Always ascending,
    /// regardless of `Direction` — ties must land in the same relative
    /// order whichever way the primary key is sorted.
    fn tie_break(self, left: &SlotRow, right: &SlotRow) -> Ordering {
        left.slot_index.cmp(&right.slot_index)
    }

    /// A blank `display_name`/`tag` *is* the "nothing here" signal for a
    /// dangling or empty slot (see the module doc and `empty_slot_row`), so
    /// `Name`/`Tag` must treat it as unknown exactly like the `Option`-typed
    /// keys below treat `None` — otherwise a dangling/empty row sorts by
    /// plain (empty-)string comparison and its position flips with
    /// `Direction` instead of landing last every time.
    fn value_unknown(self, row: &SlotRow) -> bool {
        match self {
            Self::Length => row.duration_ms.is_none(),
            Self::Modified => row.modified_ms.is_none(),
            Self::Added => row.added_ms.is_none(),
            Self::Name => row.display_name.is_empty(),
            Self::Tag => row.tag.is_empty(),
            Self::SlotNumber => false,
        }
    }
}
