//! Invariant tests for the slot manager's row model and sort key (#198).
//!
//! Scoped to `app::slots::rows` in isolation, ahead of `HonkHonk::slot_rows()`
//! / `slot_render_order()` (a follow-up task in this issue's chain) -- these
//! pin the row-building and `SortKey<SlotRow>` contracts directly against
//! `rows::build_slot_rows` and `SortState<SlotSortKey>::sorted`, the same
//! primitives that method will be built from.

use std::cmp::Ordering;
use std::path::PathBuf;

use super::HonkHonk;
use super::rows::{self, SlotRow};
use crate::app::slot_sort::SlotSortKey;
use crate::state::{AudioFormat, SoundEntry};
use crate::ui::list_controls::sort::{Direction, SortKey, SortState};

fn sound(id: &str, name: &str, path: &str, category: &str) -> SoundEntry {
    SoundEntry {
        id: id.into(),
        name: name.into(),
        path: PathBuf::from(path),
        format: AudioFormat::Wav,
        duration_ms: Some(1_000),
        modified_ms: Some(500),
        category: category.into(),
    }
}

/// A populated app: slot 0 gets a resolved sound, slot 1 a dangling sound
/// reference (file removed from the library), slot 2 a dangling macro
/// reference (macro deleted); every other slot is left empty.
fn app_with_mixed_slots() -> HonkHonk {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![sound(
        "goose",
        "goose_honk",
        "/sounds/Animals/goose.wav",
        "Animals",
    )];
    app.slots.set(0, PathBuf::from("/sounds/Animals/goose.wav"));
    app.slots
        .set(1, PathBuf::from("/sounds/Animals/deleted.wav"));
    app.slots
        .set_macro(2, "gone-macro")
        .expect("valid macro id");
    app
}

/// The single most important test in this PR (per the issue): sorting must
/// only ever reorder `build_slot_rows`' output -- it can never add, drop, or
/// mutate a row. Every `SlotSortKey`, in both directions, is checked against
/// the same fixture.
#[test]
fn sort_reorders_render_only() {
    let app = app_with_mixed_slots();
    let unsorted = rows::build_slot_rows(&app);

    for key in SlotSortKey::ALL {
        for direction in [Direction::Ascending, Direction::Descending] {
            let sort = SortState::new(key, direction);
            let sorted = sort.sorted(unsorted.clone());

            assert_eq!(
                sorted.len(),
                unsorted.len(),
                "key {key:?} direction {direction:?} changed row count"
            );

            let mut sorted_indices: Vec<u8> = sorted.iter().map(|row| row.slot_index).collect();
            let mut unsorted_indices: Vec<u8> = unsorted.iter().map(|row| row.slot_index).collect();
            sorted_indices.sort_unstable();
            unsorted_indices.sort_unstable();
            assert_eq!(
                sorted_indices, unsorted_indices,
                "key {key:?} direction {direction:?} changed row membership"
            );

            for row in &sorted {
                let original = unsorted
                    .iter()
                    .find(|candidate| candidate.slot_index == row.slot_index)
                    .expect("every sorted row exists in the unsorted set");
                assert_eq!(row, original, "sorting must not mutate row content");
            }
        }
    }
}

/// `build_slot_rows` is total: it always returns exactly one row per fixed
/// slot, in slot order, whatever `HonkHonk`'s content -- unlike the bindings
/// list (`hotkeys::rows::build_hotkey_rows`), row membership never depends on
/// a bound trigger.
#[test]
fn build_slot_rows_is_total_and_slot_ordered() {
    for app in [HonkHonk::new_for_test(), app_with_mixed_slots()] {
        let rows = rows::build_slot_rows(&app);
        let indices: Vec<u8> = rows.iter().map(|row| row.slot_index).collect();
        assert_eq!(indices, (0..20).collect::<Vec<u8>>());
    }
}

/// A dangling sound reference, a dangling macro reference, and a genuinely
/// empty slot all resolve to the same fully-blank haystack -- there is no
/// placeholder text (unlike the bindings list's "Missing sound"/"Deleted
/// macro"/"Unassigned"), because the slot manager tile itself already
/// renders the empty-slot affordance for every one of these cases.
#[test]
fn dangling_and_empty_slots_share_a_fully_blank_haystack() {
    let app = app_with_mixed_slots();
    let built = rows::build_slot_rows(&app);

    for idx in [1u8, 2, 3] {
        let row = built
            .iter()
            .find(|row| row.slot_index == idx)
            .unwrap_or_else(|| panic!("slot {idx} missing from build_slot_rows output"));
        assert_eq!(
            rows::slot_haystacks(row).collect::<Vec<_>>(),
            ["", "", ""],
            "slot {idx} should have a fully-blank haystack"
        );
        assert_eq!(row.duration_ms, None);
        assert_eq!(row.modified_ms, None);
        assert_eq!(row.added_ms, None);
    }
}

/// `Name` and `Tag` must treat a blank value as "unknown" -- mirroring how
/// `Length`/`Modified`/`Added` already treat their `Option` fields -- because
/// a blank `display_name`/`tag` *is* the dangling/empty-slot signal
/// (`rows.rs`'s own doc comment). Without this, a dangling/empty row sorts by
/// plain string comparison instead of being routed last regardless of
/// `Direction`.
#[test]
fn name_and_tag_treat_a_blank_value_as_unknown() {
    let app = app_with_mixed_slots();
    let built = rows::build_slot_rows(&app);

    let named = built
        .iter()
        .find(|row| row.slot_index == 0)
        .expect("slot 0 is populated by the fixture");
    assert!(!named.display_name.is_empty());
    assert!(!named.tag.is_empty());
    assert!(!SlotSortKey::Name.value_unknown(named));
    assert!(!SlotSortKey::Tag.value_unknown(named));

    let blank = built
        .iter()
        .find(|row| row.slot_index == 3)
        .expect("slot 3 is left empty by the fixture");
    assert!(blank.display_name.is_empty());
    assert!(blank.tag.is_empty());
    assert!(SlotSortKey::Name.value_unknown(blank));
    assert!(SlotSortKey::Tag.value_unknown(blank));
}

/// `SlotSortKey::SlotNumber` never treats a row as "unknown" (every slot has
/// a slot index by construction). Its own `tie_break` -- reachable when
/// resolving ties for *any* key, e.g. two rows sharing a blank `Tag` -- always
/// orders ascending by slot index, regardless of the active `Direction`.
#[test]
fn slot_number_is_never_unknown_and_ties_break_ascending() {
    let app = app_with_mixed_slots();
    let built = rows::build_slot_rows(&app);

    for row in &built {
        assert!(!SlotSortKey::SlotNumber.value_unknown(row));
    }

    let low = &built[0];
    let high = &built[5];
    assert_eq!(SlotSortKey::SlotNumber.tie_break(low, high), Ordering::Less);
    assert_eq!(
        SlotSortKey::SlotNumber.tie_break(high, low),
        Ordering::Greater
    );

    // Slots 3-19 all share a blank `Tag`, so `Tag::compare()` ties for every
    // pair among them; the tie must resolve to ascending slot order in
    // *both* directions -- `tie_break()` is never reversed by `Direction`,
    // unlike the primary comparison.
    let tied: Vec<SlotRow> = built
        .iter()
        .filter(|row| row.tag.is_empty())
        .cloned()
        .collect();
    for direction in [Direction::Ascending, Direction::Descending] {
        let sort = SortState::new(SlotSortKey::Tag, direction);
        let sorted = sort.sorted(tied.clone());
        let indices: Vec<u8> = sorted.iter().map(|row| row.slot_index).collect();
        let mut expected = indices.clone();
        expected.sort_unstable();
        assert_eq!(
            indices, expected,
            "direction {direction:?}: ties among equal Tag values must resolve ascending by slot_index"
        );
    }
}
