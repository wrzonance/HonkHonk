//! Pure row-building for the Settings → Shortcuts bindings list (#199).
//!
//! A row exists for every slot with a bound trigger (`HonkHonk::slot_triggers`),
//! regardless of whether that slot has any content assigned in `SlotMap` — a
//! trigger can be bound to an empty slot, or point at a sound/macro that no
//! longer exists. `build_hotkey_rows` resolves each of those cases into an
//! owned, always-displayable [`HotkeyRow`]; `HonkHonk::hotkey_rows()` (in the
//! parent module) filters and sorts the result.

use std::cmp::Ordering;
use std::path::Path;

use super::HonkHonk;
use crate::app::slot_sort::SlotSortKey;
use crate::state::SlotContent;
use crate::ui::list_controls::sort::SortKey;

const MISSING_SOUND_LABEL: &str = "Missing sound";
const DELETED_MACRO_LABEL: &str = "Deleted macro";
const UNASSIGNED_LABEL: &str = "Unassigned";

/// One row in the Settings → Shortcuts bindings list: a bound slot plus its
/// resolved content, if any. Fully owned (no lifetime param) so it can
/// outlive the borrows used to build it — `hotkey_rows()` returns a fresh
/// `Vec` on every call rather than caching indices into `HonkHonk`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HotkeyRow {
    pub(crate) slot_index: u8,
    pub(crate) trigger: String,
    /// Never empty: falls back to a placeholder ("Missing sound", "Deleted
    /// macro", "Unassigned") when there's nothing real to show.
    pub(crate) display_name: String,
    pub(crate) filename: String,
    pub(crate) tag: String,
    pub(crate) tags: Vec<String>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) modified_ms: Option<u64>,
    pub(crate) added_ms: Option<u64>,
}

/// Builds one row per bound trigger, in slot order. A slot without a bound
/// trigger contributes no row, even if it has content assigned.
pub(super) fn build_hotkey_rows(state: &HonkHonk) -> Vec<HotkeyRow> {
    state
        .slot_triggers
        .iter()
        .enumerate()
        .filter_map(|(index, trigger)| {
            let trigger = trigger.as_deref()?;
            Some(hotkey_row(state, index as u8, trigger))
        })
        .collect()
}

/// Resolves the content assigned to `slot_index`, if any, into a row. The
/// three match arms are the only three cases `SlotMap::content` can return —
/// each is defensive and non-panicking on top of that.
fn hotkey_row(state: &HonkHonk, slot_index: u8, trigger: &str) -> HotkeyRow {
    match state.slots.content(slot_index) {
        Some(SlotContent::Sound(path)) => sound_row(state, slot_index, trigger, path),
        Some(SlotContent::Macro(id)) => macro_row(state, slot_index, trigger, id),
        None => unassigned_row(slot_index, trigger),
    }
}

/// A slot bound to a sound path. The path may no longer resolve to a library
/// entry (file deleted/moved outside the app) — that's a dangling row, not
/// an error: it still renders with its file name and a "Missing sound" label.
fn sound_row(state: &HonkHonk, slot_index: u8, trigger: &str, path: &Path) -> HotkeyRow {
    let filename = file_name(path);
    let Some(sound) = state.sounds.iter().find(|entry| entry.path == path) else {
        return HotkeyRow {
            slot_index,
            trigger: trigger.to_owned(),
            display_name: MISSING_SOUND_LABEL.to_owned(),
            filename,
            tag: String::new(),
            tags: Vec::new(),
            duration_ms: None,
            modified_ms: None,
            added_ms: None,
        };
    };
    let custom_name = state
        .sound_meta
        .get_ref(&sound.id)
        .and_then(|meta| meta.display_name.as_deref());
    HotkeyRow {
        slot_index,
        trigger: trigger.to_owned(),
        display_name: resolved_display_name(custom_name, &sound.name),
        filename,
        tag: sound.category.clone(),
        tags: state.sound_meta.get(&sound.id).tags,
        duration_ms: sound.duration_ms,
        modified_ms: sound.modified_ms,
        added_ms: state.sound_meta.added_ms(&sound.id),
    }
}

/// A slot bound to a macro id. The macro may have since been deleted —
/// that's a dangling row too, rendered with a "Deleted macro" label.
fn macro_row(state: &HonkHonk, slot_index: u8, trigger: &str, id: &str) -> HotkeyRow {
    let macro_name = state.macros.get(id).map(|entry| entry.name.as_str());
    HotkeyRow {
        slot_index,
        trigger: trigger.to_owned(),
        display_name: resolved_display_name(macro_name, DELETED_MACRO_LABEL),
        filename: String::new(),
        tag: String::new(),
        tags: Vec::new(),
        duration_ms: None,
        modified_ms: None,
        added_ms: None,
    }
}

/// A slot with a bound trigger but no content assigned at all.
fn unassigned_row(slot_index: u8, trigger: &str) -> HotkeyRow {
    HotkeyRow {
        slot_index,
        trigger: trigger.to_owned(),
        display_name: UNASSIGNED_LABEL.to_owned(),
        filename: String::new(),
        tag: String::new(),
        tags: Vec::new(),
        duration_ms: None,
        modified_ms: None,
        added_ms: None,
    }
}

/// `raw`, if present and non-empty, else `fallback` — the shared
/// non-empty-display-name guarantee used by every resolution branch.
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

/// The fields a type-to-filter query matches against: customized name, file
/// name, folder, and each assigned tag independently. The trigger itself is
/// deliberately not searchable.
pub(super) fn hotkey_haystacks(row: &HotkeyRow) -> impl Iterator<Item = &str> {
    [row.display_name.as_str(), &row.filename, &row.tag]
        .into_iter()
        .chain(row.tags.iter().map(String::as_str))
}

impl SortKey<HotkeyRow> for SlotSortKey {
    fn compare(self, left: &HotkeyRow, right: &HotkeyRow) -> Ordering {
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

    /// Every row has a unique slot_index, so this tie-break is total: two
    /// distinct rows sharing a primary value (e.g. the same `Tag`) are
    /// never `Ordering::Equal` all the way through. Always ascending,
    /// regardless of `Direction` — ties must land in the same relative
    /// order whichever way the primary key is sorted.
    fn tie_break(self, left: &HotkeyRow, right: &HotkeyRow) -> Ordering {
        left.slot_index.cmp(&right.slot_index)
    }

    fn value_unknown(self, row: &HotkeyRow) -> bool {
        match self {
            Self::Length => row.duration_ms.is_none(),
            Self::Modified => row.modified_ms.is_none(),
            Self::Added => row.added_ms.is_none(),
            Self::SlotNumber | Self::Name | Self::Tag => false,
        }
    }
}
