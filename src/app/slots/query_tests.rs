//! Invariant tests for the slot manager's `HonkHonk`-level query surface
//! (#198): `slot_rows()`/`slot_render_order()` compose filtering and
//! sorting, `slot_filter_query()`/`slot_sort_state()` mirror the underlying
//! state, and the sort-menu mutators persist under `sort_prefs["slots"]`.
//! Complements `sort_tests.rs`, which pins `rows::build_slot_rows` and
//! `SortKey<SlotRow>` in isolation ahead of this query surface.

use std::path::PathBuf;

use super::{HonkHonk, SLOTS_VIEW_KEY, SlotSortState, default_slot_sort};
use crate::app::slot_sort::SlotSortKey;
use crate::state::{AppConfig, AudioFormat, SortPref, SoundEntry};
use crate::ui::list_controls::sort::Direction;

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

/// Two resolved slots ("Alpha"/"Zoo" tag at index 0, "Beta"/"Ark" tag at
/// index 5) plus an empty slot at index 1 — enough to exercise filtering,
/// sorting, and their composition against real content.
fn app_with_two_sounds() -> HonkHonk {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        sound("a", "Alpha", "/sounds/alpha.wav", "Zoo"),
        sound("b", "Beta", "/sounds/beta.wav", "Ark"),
    ];
    app.slots.set(0, PathBuf::from("/sounds/alpha.wav"));
    app.slots.set(5, PathBuf::from("/sounds/beta.wav"));
    app
}

/// The single most important composition invariant: filtering narrows the
/// row set *before* sorting reorders it. A query matching only one of the
/// two populated slots must leave exactly one row in `slot_rows()`,
/// regardless of which sort key/direction is active.
#[test]
fn filter_narrows_before_sort_reorders() {
    let mut app = app_with_two_sounds();
    app.slot_filter.replace("alpha".into());

    for key in SlotSortKey::ALL {
        for direction in [Direction::Ascending, Direction::Descending] {
            app.slot_sort = SlotSortState::new(key, direction);
            let rows = app.slot_rows();
            assert_eq!(
                rows.len(),
                1,
                "key {key:?} direction {direction:?}: filter must narrow before sort runs"
            );
            assert_eq!(rows[0].slot_index, 0);
        }
    }
}

/// `slot_render_order()` mirrors `slot_rows()`'s slot-index order exactly —
/// it's the render-only projection consumed by the grid layout.
#[test]
fn slot_render_order_mirrors_filtered_and_sorted_rows() {
    let mut app = app_with_two_sounds();
    app.slot_sort = SlotSortState::new(SlotSortKey::Name, Direction::Descending);

    let rows = app.slot_rows();
    let order = app.slot_render_order();

    let expected: Vec<u8> = rows.iter().map(|row| row.slot_index).collect();
    assert_eq!(order, expected);
    // "Beta" > "Alpha" descending -> slot 5 first.
    assert_eq!(order.first(), Some(&5));
}

/// Filter matching is a case-insensitive substring search over exactly
/// three fields: display name, file name, tag — nothing else (e.g. the slot
/// index itself must not be searchable text).
#[test]
fn filter_matches_display_name_filename_and_tag_case_insensitively() {
    let mut app = app_with_two_sounds();

    for query in ["ALPHA", "alpha.wav", "zoo"] {
        app.slot_filter.replace(query.into());
        let rows = app.slot_rows();
        assert_eq!(
            rows.len(),
            1,
            "query {query:?} should case-insensitively match slot 0's row"
        );
        assert_eq!(rows[0].slot_index, 0);
    }

    app.slot_filter.replace("0".into());
    assert!(
        app.slot_rows().is_empty(),
        "the slot index itself must not be searchable text"
    );
}

/// `MacroStore` accepts a blank or whitespace-only macro name, which every
/// rendering surface shows as "Untitled macro" (`slot_manager::display_name`).
/// The row must carry that same label, or the grid would be searchable and
/// sortable by a value the user cannot see: querying the visible label would
/// match nothing, and a whitespace-only name would sort ahead of every real
/// one instead of being treated as unnamed.
#[test]
fn unnamed_macro_slots_filter_and_sort_by_their_visible_label() {
    for raw_name in ["", "   \t "] {
        let mut app = HonkHonk::new_for_test();
        let id = app.macros.add(raw_name).id.clone();
        app.slots.set_macro(4, &id).expect("valid macro id");

        app.slot_filter.replace("untitled".into());
        let rows = app.slot_rows();
        assert_eq!(
            rows.len(),
            1,
            "macro named {raw_name:?} renders as \"Untitled macro\" and must be findable by it"
        );
        assert_eq!(rows[0].slot_index, 4);
        assert_eq!(rows[0].display_name, "Untitled macro");
    }
}

/// An empty query matches every slot — all 20 rows, not just the populated
/// ones (`build_slot_rows` is total).
#[test]
fn empty_query_returns_every_slot() {
    let app = app_with_two_sounds();
    assert_eq!(app.slot_rows().len(), 20);
}

/// The composed-API cardinality invariant, checked through the actual method
/// the invariant names (`slot_rows()`/`slot_render_order()`) rather than only
/// through the disconnected `rows::build_slot_rows` + `SortState::sorted`
/// primitives (`sort_tests.rs`'s `sort_reorders_render_only` covers those in
/// isolation): under an empty query, every one of the 20 fixed slot indices
/// appears in `slot_render_order()` exactly once -- no duplicate, no drop --
/// for every sort key and direction.
#[test]
fn slot_render_order_has_no_duplicate_or_dropped_slot_under_an_empty_query() {
    let mut expected: Vec<u8> = (0..20).collect();

    for key in SlotSortKey::ALL {
        for direction in [Direction::Ascending, Direction::Descending] {
            let mut app = app_with_two_sounds();
            app.slot_sort = SlotSortState::new(key, direction);

            let mut order = app.slot_render_order();
            assert_eq!(
                order.len(),
                20,
                "key {key:?} direction {direction:?}: expected exactly 20 rows"
            );
            order.sort_unstable();
            expected.sort_unstable();
            assert_eq!(
                order, expected,
                "key {key:?} direction {direction:?}: every slot index must appear exactly once"
            );
        }
    }
}

/// Dangling/empty rows must sort last regardless of `Direction` (the pinned
/// `value_unknown` invariant on `SlotSortKey::Name`/`Tag`), never flip
/// position with the two named rows depending on ascending/descending.
#[test]
fn blank_rows_sort_last_by_name_regardless_of_direction() {
    let mut app = app_with_two_sounds();

    for direction in [Direction::Ascending, Direction::Descending] {
        app.slot_sort = SlotSortState::new(SlotSortKey::Name, direction);
        let order = app.slot_render_order();

        let mut named_positions: Vec<u8> = order[..2].to_vec();
        named_positions.sort_unstable();
        assert_eq!(
            named_positions,
            vec![0, 5],
            "direction {direction:?}: the two named slots must land first"
        );
        assert!(
            order[2..].iter().all(|idx| *idx != 0 && *idx != 5),
            "direction {direction:?}: blank slots must occupy every later position"
        );
    }
}

#[test]
fn accessors_mirror_the_underlying_filter_and_sort_state() {
    let mut app = HonkHonk::new_for_test();
    app.slot_filter.replace("honk".into());
    app.slot_sort = SlotSortState::new(SlotSortKey::Added, Direction::Descending);

    assert_eq!(app.slot_filter_query(), "honk");
    assert_eq!(
        app.slot_sort_state(),
        SlotSortState::new(SlotSortKey::Added, Direction::Descending)
    );
}

/// `select_slot_sort` + `toggle_slot_sort_direction` both mutate
/// `slot_sort` and persist to `config.sort_prefs["slots"]` — the round-trip
/// invariant.
#[test]
fn selecting_a_sort_key_and_toggling_direction_persists_under_slots_key() {
    let mut app = HonkHonk::new_for_test();

    app.select_slot_sort("tag");
    app.toggle_slot_sort_direction();

    assert_eq!(
        app.slot_sort,
        SlotSortState::new(SlotSortKey::Tag, Direction::Descending)
    );
    assert_eq!(
        app.config.sort_prefs.get(SLOTS_VIEW_KEY),
        Some(&SortPref::new("tag", "descending"))
    );
}

/// An unknown persisted sort id closes the menu (via `select_slot_sort`'s
/// early return) without changing the active sort or writing a preference.
#[test]
fn selecting_an_unknown_sort_id_leaves_the_active_sort_unchanged() {
    let mut app = HonkHonk::new_for_test();
    app.toggle_slot_sort_menu();

    app.select_slot_sort("future-key");

    assert!(app.sort_menu_anchor.is_none());
    assert_eq!(app.slot_sort, default_slot_sort());
    assert!(app.config.sort_prefs.is_empty());
}

/// Opening the sort menu captures the current cursor position as the
/// anchor; dismissing clears it without touching the active sort or
/// persisting anything.
#[test]
fn toggling_the_sort_menu_opens_and_closes_without_changing_sort() {
    let mut app = HonkHonk::new_for_test();
    app.cursor_pos = iced::Point::new(12.0, 34.0);
    let original = app.slot_sort;

    app.toggle_slot_sort_menu();
    assert_eq!(app.sort_menu_anchor, Some(iced::Point::new(12.0, 34.0)));

    let dismissed = app.dismiss_slot_sort_menu();
    assert!(dismissed);
    assert!(app.sort_menu_anchor.is_none());
    assert_eq!(app.slot_sort, original);
    assert!(app.config.sort_prefs.is_empty());
}

/// `replace_slot_filter_query` is transient like every other list-controls
/// filter: it lands in `slot_filter` but is never written to `sort_prefs`.
#[test]
fn replace_filter_query_updates_state_without_persisting() {
    let mut app = HonkHonk::new_for_test();

    app.replace_slot_filter_query("goose".into());

    assert_eq!(app.slot_filter_query(), "goose");
    assert!(app.config.sort_prefs.is_empty());
}

#[test]
fn slots_view_key_matches_the_issue_acceptance_text() {
    assert_eq!(SLOTS_VIEW_KEY, "slots");
}

/// A missing, unknown-key, or corrupt-direction preference all fall back to
/// the complete default (`SlotNumber`/`Ascending`) rather than a partial
/// reconstruction.
#[test]
fn slot_sort_from_config_round_trips_a_valid_preference_and_falls_back_otherwise() {
    let mut valid = AppConfig::default();
    valid
        .sort_prefs
        .insert(SLOTS_VIEW_KEY.into(), SortPref::new("tag", "descending"));
    assert_eq!(
        super::slot_sort_from_config(&valid),
        SlotSortState::new(SlotSortKey::Tag, Direction::Descending)
    );

    for pref in [
        SortPref::new("future", "descending"),
        SortPref::new("tag", "sideways"),
    ] {
        let mut config = AppConfig::default();
        config.sort_prefs.insert(SLOTS_VIEW_KEY.into(), pref);
        assert_eq!(super::slot_sort_from_config(&config), default_slot_sort());
    }

    assert_eq!(
        super::slot_sort_from_config(&AppConfig::default()),
        default_slot_sort()
    );
}
