//! Shortcut-slot activation and macro-slot assignment (app layer, #169).
//! Resolves a slot's content ([`crate::state::SlotContent`]) at press time and
//! dispatches to the right play path, self-clearing any reference that no
//! longer resolves (a deleted sound file or a removed macro). Mirrors the
//! shipped [`super::macros`] controller: pure `impl HonkHonk` methods, no new
//! state-layer struct.

use std::path::PathBuf;

use iced::Task;

use super::{HonkHonk, Message};
use crate::app::slot_sort::SlotSortKey;
use crate::state::{AppConfig, SlotContent, SortPref};
use crate::ui::list_controls::filter::filter_items;
use crate::ui::list_controls::sort::{Direction, SortState};

#[cfg(test)]
mod query_tests;
mod rows;
#[cfg(test)]
mod sort_tests;
mod view;

/// Sort state for the slot manager grid (#198).
///
/// Declared `pub(crate)`, not `pub(super)`: Rust's re-export rule only lets
/// `pub use` narrow an item's own visibility, never widen it (E0365) — a
/// `pub(crate)` accessor that names this type in a return position (e.g.
/// `slot_sort_state()`, below) would be a private-interfaces violation under
/// `pub(super)`. Mirrors `hotkeys::HotkeySortState` exactly.
pub(crate) type SlotSortState = SortState<SlotSortKey>;

/// Sort-preference persistence key for the slot manager view, stored in
/// `AppConfig::sort_prefs` alongside the tiles view's `"tiles"` key and the
/// bindings list's `"shortcuts"` key.
const SLOTS_VIEW_KEY: &str = "slots";

fn default_slot_sort() -> SlotSortState {
    SlotSortState::new(SlotSortKey::default(), Direction::Ascending)
}

/// Resolves a persisted sort id back into a `SlotSortKey`. Mirrors
/// `hotkeys::slot_sort_key_from_id` — `SlotSortKey` keeps its own `from_id`
/// private to `slot_sort.rs`, so each consumer does its own `ALL`-scan.
fn slot_sort_key_from_id(id: &str) -> Option<SlotSortKey> {
    SlotSortKey::ALL.into_iter().find(|key| key.id() == id)
}

/// Reads the persisted sort preference, falling back to the default
/// (`SlotNumber` ascending) for a missing, unknown, or corrupt entry. Never
/// panics on untrusted config content.
pub(super) fn slot_sort_from_config(config: &AppConfig) -> SlotSortState {
    let Some(pref) = config.sort_prefs.get(SLOTS_VIEW_KEY) else {
        return default_slot_sort();
    };
    let Some(key) = slot_sort_key_from_id(pref.key()) else {
        return default_slot_sort();
    };
    let direction = match pref.direction() {
        "ascending" => Direction::Ascending,
        "descending" => Direction::Descending,
        _ => return default_slot_sort(),
    };
    SlotSortState::new(key, direction)
}

// Test-only spies for `HonkHonk::persist_slots`: populated unconditionally,
// ahead of the `self.persist` gate, so a test can prove a slot mutation
// actually reached the persist call. `HonkHonk::new_for_test()` hardcodes
// `persist: false` (see `mod.rs`) so `cargo test` never touches the real
// XDG config dir — which also makes the real disk write a guaranteed no-op
// and leaves the call itself unobservable without these spies (#169 review).
//
// Thread-local, not a process-wide static: `cargo test`'s worker threads run
// one `#[test]` fn to completion before picking up the next, so within a
// single test's execution only that test's own code can touch its thread's
// cell — a call made by an unrelated test running concurrently on another
// thread can never leak into this test's before/after delta. A process-wide
// counter could not make that attribution guarantee.
//
// Compiled only under `cfg(test)`; zero footprint on release builds.
#[cfg(test)]
thread_local! {
    /// Count of `persist_slots` calls observed on this thread.
    static PERSIST_SLOTS_CALLS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Clone of `self.slots` taken *inside* `persist_slots`, at the instant
    /// it runs — lets a test prove ordering (the mutation was already
    /// applied before persistence ran), not just that persistence ran at
    /// some point.
    static PERSIST_SLOTS_SNAPSHOT: std::cell::RefCell<Option<crate::state::SlotMap>> =
        const { std::cell::RefCell::new(None) };
}

/// Current value of this thread's [`PERSIST_SLOTS_CALLS`] cell. Assertions
/// must compare a before/after delta (`after > before`), never an absolute
/// value — see the thread-local's doc comment for why the delta is safe to
/// trust even under parallel test execution.
#[cfg(test)]
pub(crate) fn persist_slots_call_count() -> u32 {
    PERSIST_SLOTS_CALLS.with(std::cell::Cell::get)
}

/// The slot map as it looked *inside* the most recent [`HonkHonk::persist_slots`]
/// call on this thread, or `None` if it has not been called yet this test.
/// Lets a test assert that a specific mutation was already visible at
/// persist-time, pinning ordering rather than mere occurrence.
#[cfg(test)]
pub(crate) fn persist_slots_last_snapshot() -> Option<crate::state::SlotMap> {
    PERSIST_SLOTS_SNAPSHOT.with(|snapshot| snapshot.borrow().clone())
}

impl HonkHonk {
    /// A shortcut (or the slot manager) fired slot `idx`. Resolves the slot's
    /// content and dispatches: a missing slot is a no-op, a sound plays via
    /// `request_play`, a macro plays via `play_macro`. Never panics — an
    /// out-of-range `idx` is already a safe no-op in [`SlotMap::content`].
    pub(crate) fn activate_slot(&mut self, idx: u8) -> Task<Message> {
        match self.slots.content(idx).cloned() {
            None => Task::none(),
            Some(SlotContent::Sound(path)) => self.activate_sound_slot(idx, path),
            Some(SlotContent::Macro(macro_id)) => self.activate_macro_slot(idx, macro_id),
        }
    }

    /// Plays the library sound at `path` if it still exists; otherwise the
    /// slot outlived its target (file deleted/moved) and is cleared.
    fn activate_sound_slot(&mut self, idx: u8, path: PathBuf) -> Task<Message> {
        if let Some(sound) = self.sounds.iter().find(|s| s.path == path).cloned() {
            return self.request_play(&sound, true);
        }
        tracing::warn!(
            slot = idx + 1,
            ?path,
            "slot points to missing file; clearing stale slot"
        );
        self.clear_stale_slot(idx);
        Task::none()
    }

    /// Fires `macro_id` unconditionally once it is known to exist — including
    /// an existing-but-zero-step macro, a valid authoring state that must
    /// never self-clear. An unknown/deleted id is the only stale case here,
    /// and clears the slot without ever reaching `play_macro`.
    fn activate_macro_slot(&mut self, idx: u8, macro_id: String) -> Task<Message> {
        if self.macros.get(&macro_id).is_none() {
            tracing::warn!(
                slot = idx + 1,
                macro_id = %macro_id,
                "slot points to missing macro; clearing stale slot"
            );
            self.clear_stale_slot(idx);
            return Task::none();
        }
        self.play_macro(&macro_id)
    }

    /// Shared self-clear: drops the slot's content and persists the change
    /// under the same switch as every other slot mutation.
    fn clear_stale_slot(&mut self, idx: u8) {
        self.slots.clear(idx);
        self.persist_slots();
    }

    /// Binds slot `idx` to macro `macro_id`. Mutates and persists only if the
    /// id passes `SlotMap::set_macro`'s validation; a rejected id leaves the
    /// slot's existing content untouched.
    pub(crate) fn assign_macro_slot(&mut self, idx: u8, macro_id: String) -> Task<Message> {
        match self.slots.set_macro(idx, macro_id) {
            Ok(()) => self.persist_slots(),
            Err(e) => {
                tracing::warn!(slot = idx + 1, error = %e, "macro slot assignment rejected");
            }
        }
        Task::none()
    }

    /// Persists the slot map under the same persistence switch as the config.
    /// Colocated here with its two slot-mutation call sites above
    /// (`clear_stale_slot`, `assign_macro_slot`); also called from
    /// `mod.rs`'s `AssignSlot`/`ClearSlot` message arms.
    pub(super) fn persist_slots(&self) {
        #[cfg(test)]
        {
            PERSIST_SLOTS_CALLS.with(|calls| calls.set(calls.get() + 1));
            PERSIST_SLOTS_SNAPSHOT
                .with(|snapshot| *snapshot.borrow_mut() = Some(self.slots.clone()));
        }
        if self.persist
            && let Err(e) = self.slots.save()
        {
            tracing::warn!(error = %e, "slots save error");
        }
    }
}

/// The slot manager grid's pure query surface (#198): filtered, sorted rows
/// and the individual pieces of state they're built from. Mirrors
/// `hotkeys::hotkey_rows`/`hotkey_filter_query`/`hotkey_sort_state` exactly.
impl HonkHonk {
    /// Every fixed slot ([`rows::build_slot_rows`]), narrowed by
    /// `slot_filter`'s query, then reordered by `slot_sort` — filtering
    /// always runs before sorting, so the active sort key never influences
    /// which rows survive the query. Pure: rebuilt from current state on
    /// every call, no cache (at most 20 rows, so there's no cost to
    /// justify one).
    fn slot_rows(&self) -> Vec<rows::SlotRow> {
        let built = rows::build_slot_rows(self);
        let matched: Vec<rows::SlotRow> =
            filter_items(&built, self.slot_filter.query(), rows::slot_haystacks)
                .into_iter()
                .cloned()
                .collect();
        self.slot_sort.sorted(matched)
    }

    /// The render-only projection of [`Self::slot_rows`]: just the slot
    /// indices, in the order the grid should lay tiles out. `ui::slot_manager`
    /// resolves each index's content independently (`resolve_slot`), so this
    /// never needs to carry more than the ordering itself.
    pub(crate) fn slot_render_order(&self) -> Vec<u8> {
        self.slot_rows().iter().map(|row| row.slot_index).collect()
    }

    /// Mirrors `hotkey_filter_query()`: `ui/slot_manager/mod.rs` lives in a
    /// sibling module tree to `crate::app`, so it needs an accessor rather
    /// than a private-field reach-across.
    pub(crate) fn slot_filter_query(&self) -> &str {
        self.slot_filter.query()
    }

    /// Mirrors the above for the active sort state.
    pub(crate) fn slot_sort_state(&self) -> SlotSortState {
        self.slot_sort
    }
}

/// Message-driven mutators for the slot manager's filter/sort chip (#198).
/// Mirrors `hotkeys`'s equivalent block; wired into `update()` by a
/// follow-up task in this issue's chain.
impl HonkHonk {
    /// Opens or closes the shared sort-menu overlay for the slot manager's
    /// sort chip. Reuses `sort_menu_anchor`, the same field every other
    /// list-controls sort menu uses — only one sort menu is ever open at a
    /// time.
    pub(super) fn toggle_slot_sort_menu(&mut self) {
        self.sort_menu_anchor = if self.sort_menu_anchor.is_some() {
            None
        } else {
            Some(self.cursor_pos)
        };
    }

    pub(super) fn toggle_slot_sort_direction(&mut self) {
        self.slot_sort.toggle_direction();
        self.persist_slot_sort();
    }

    /// An unknown id (e.g. a persisted value from a newer build reading an
    /// older config) closes the menu without changing the active sort,
    /// matching `select_hotkey_sort`.
    pub(super) fn select_slot_sort(&mut self, key_id: &str) {
        let Some(key) = slot_sort_key_from_id(key_id) else {
            self.sort_menu_anchor = None;
            return;
        };
        self.slot_sort.select(key);
        self.sort_menu_anchor = None;
        self.persist_slot_sort();
    }

    pub(super) fn dismiss_slot_sort_menu(&mut self) -> bool {
        self.sort_menu_anchor.take().is_some()
    }

    fn persist_slot_sort(&mut self) {
        self.config.sort_prefs.insert(
            SLOTS_VIEW_KEY.into(),
            SortPref::new(self.slot_sort.key().id(), self.slot_sort.direction().id()),
        );
        self.persist_config();
    }

    /// Transient, like the tiles view's filter query: never written to
    /// `config.sort_prefs`. Mirrors `replace_hotkey_filter_query`.
    pub(super) fn replace_slot_filter_query(&mut self, query: String) {
        self.slot_filter.replace(query);
    }
}

#[cfg(test)]
mod tests;
