//! Boundary tests for `slot_grid`'s render-order handling (#198 Task 9/10).
//! Pins two invariants that the design spike flagged as risk areas:
//!
//! - **Reorder-without-remap**: `render_order` controls *where* a slot's
//!   tile lands in the grid, but never *which* number the tile shows for
//!   itself — a tile always reports its own `slot_index`, never a
//!   position-derived one.
//! - **Plain `u8` copies**: the `&[u8]` slice crossing this boundary carries
//!   no borrow into the returned `Element` — the slice can be dropped the
//!   instant `slot_grid` returns.
//!
//! Also pins the not-yet-spiked empty-state branch: an empty `render_order`
//! (a query that matched nothing) renders the "no matches" message instead
//! of an empty grid.

use iced_test::simulator;

use super::*;
use crate::state::{AudioFormat, SlotMap};
use crate::test_lock::gui_lock;
use std::path::PathBuf;

fn sound(id: &str, name: &str, path: &str) -> SoundEntry {
    SoundEntry {
        id: id.into(),
        name: name.into(),
        path: PathBuf::from(path),
        format: AudioFormat::Wav,
        duration_ms: Some(100),
        modified_ms: None,
        category: "Test".into(),
    }
}

fn empty_triggers() -> [Option<String>; 20] {
    std::array::from_fn(|_| None)
}

fn ctx<'a>(
    slots: &'a SlotMap,
    slot_triggers: &'a [Option<String>; 20],
    sounds: &'a [SoundEntry],
    macros: &'a MacroStore,
) -> SlotManagerCtx<'a> {
    SlotManagerCtx {
        slots,
        slot_triggers,
        sounds,
        macros,
        selected_slot: None,
        configure_available: false,
    }
}

/// Two bound slots, deliberately not adjacent and not in ascending order of
/// content, so a position-derived label (row * 5 + col) would visibly
/// disagree with the slot's real index.
fn two_bound_slots() -> (SlotMap, Vec<SoundEntry>) {
    let mut slots = SlotMap::default();
    let alpha = sound("s-alpha", "Alpha", "/sounds/alpha.wav");
    let beta = sound("s-beta", "Beta", "/sounds/beta.wav");
    slots.set(3, alpha.path.clone());
    slots.set(7, beta.path.clone());
    (slots, vec![alpha, beta])
}

#[test]
fn grid_renders_only_the_slots_present_in_render_order() {
    let _gui = gui_lock();
    let (slots, sounds) = two_bound_slots();
    let macros = MacroStore::default();
    let triggers = empty_triggers();
    let ctx = ctx(&slots, &triggers, &sounds, &macros);

    // Only slot 7 (Beta) is in render order; slot 3 (Alpha) was filtered out.
    let element = slot_grid(&ctx, &[7], Theme::Dark);
    let mut ui = simulator(element);

    assert!(
        ui.find("Beta").is_ok(),
        "slot 7 is in render_order and must render"
    );
    assert!(
        ui.find("Alpha").is_err(),
        "slot 3 was excluded from render_order and must not render"
    );
}

/// Reorder-without-remap: swapping the two slots' positions in
/// `render_order` must not change either tile's own `#NN` label — it always
/// reflects the tile's real `slot_index`, never its position in the grid.
#[test]
fn grid_reorders_tiles_without_remapping_their_slot_number_label() {
    let _gui = gui_lock();
    let (slots, sounds) = two_bound_slots();
    let macros = MacroStore::default();
    let triggers = empty_triggers();
    let ctx = ctx(&slots, &triggers, &sounds, &macros);

    // Slot 7 rendered first, slot 3 second — reversed from index order.
    let element = slot_grid(&ctx, &[7, 3], Theme::Dark);
    let mut ui = simulator(element);

    assert!(
        ui.find("#08").is_ok(),
        "slot 7's tile must still report its own slot number (#08)"
    );
    assert!(
        ui.find("#04").is_ok(),
        "slot 3's tile must still report its own slot number (#04), \
         not a position-derived one"
    );
}

/// The `&[u8]` slice must not be borrowed into the returned `Element` — the
/// grid renders from `SlotManagerCtx`'s own `'a` lifetime, and the returned
/// element must remain usable after `render_order` itself has been dropped.
#[test]
fn grid_element_outlives_the_render_order_slice_it_was_built_from() {
    let _gui = gui_lock();
    let (slots, sounds) = two_bound_slots();
    let macros = MacroStore::default();
    let triggers = empty_triggers();
    let ctx = ctx(&slots, &triggers, &sounds, &macros);

    let element = {
        let render_order: Vec<u8> = vec![7, 3];
        slot_grid(&ctx, &render_order, Theme::Dark)
        // `render_order` is dropped here; `element` must still be sound.
    };

    let mut ui = simulator(element);
    assert!(ui.find("Beta").is_ok());
}

/// A filtered grid can end on a partial row. Tiles are `Length::Fill`, so a
/// short row without fillers would stretch its survivors across the whole
/// grid — one match would render as a single full-width card. Iced view
/// rendering is intentionally not unit-tested here; this pins the pure
/// filler-slot contract the grid rows are built from, as
/// `sound_grid::incomplete_rows_reserve_all_missing_tile_slots` does.
#[test]
fn incomplete_rows_reserve_all_missing_tile_slots() {
    assert_eq!(missing_tile_slots(0), 5);
    assert_eq!(missing_tile_slots(1), 4);
    assert_eq!(missing_tile_slots(4), 1);
    assert_eq!(missing_tile_slots(5), 0);
    assert_eq!(missing_tile_slots(6), 0);
}

/// A query that filtered out every slot yields an empty `render_order`,
/// which must render a "no matches" message rather than an empty grid.
#[test]
fn grid_shows_no_matches_message_for_empty_render_order() {
    let _gui = gui_lock();
    let slots = SlotMap::default();
    let sounds: Vec<SoundEntry> = Vec::new();
    let macros = MacroStore::default();
    let triggers = empty_triggers();
    let ctx = ctx(&slots, &triggers, &sounds, &macros);

    let element = slot_grid(&ctx, &[], Theme::Dark);
    let mut ui = simulator(element);

    assert!(ui.find("No slots match your search.").is_ok());
}
