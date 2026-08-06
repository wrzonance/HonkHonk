//! Resolves which view currently owns type-to-filter keyboard input and
//! Escape-clearing, so `filtering.rs` can route to the right `FilterState`
//! without re-deriving view-mode/section checks at every call site (#199).

use super::HonkHonk;
use crate::app::{SettingsSection, ViewMode};

/// The view that owns type-to-filter input for the current app state.
///
/// Routing is total and mutually exclusive: [`active_filter_target`] maps
/// every `(view_mode, settings section, staged-search state)` combination to
/// exactly one of `Some(Tiles)`, `Some(Hotkeys)`, `Some(Slots)`, or `None` —
/// never more than one target is active at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilterTarget {
    /// The main sound grid's search bar.
    Tiles,
    /// The Settings → Shortcuts bindings list's own, independent search bar.
    Hotkeys,
    /// The slot manager's own, independent search bar (#198).
    Slots,
}

/// Resolves the active filter target, if any, for the current app state.
///
/// `Settings` only routes to `Hotkeys` while the Shortcuts section is
/// selected *and* the staged settings search (#213, click-only, searches
/// settings themselves — a distinct list-controls surface) is not itself
/// active. The two search surfaces are independent and must never claim the
/// same keystroke.
pub(super) fn active_filter_target(state: &HonkHonk) -> Option<FilterTarget> {
    match state.view_mode {
        ViewMode::Main => Some(FilterTarget::Tiles),
        ViewMode::Settings
            if state.settings_ui.section() == SettingsSection::Hotkeys
                && !state.settings_ui.is_searching() =>
        {
            Some(FilterTarget::Hotkeys)
        }
        ViewMode::Settings => None,
        ViewMode::SlotManager => Some(FilterTarget::Slots),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECTIONS: [SettingsSection; 5] = [
        SettingsSection::Audio,
        SettingsSection::Library,
        SettingsSection::Hotkeys,
        SettingsSection::Appearance,
        SettingsSection::About,
    ];

    const VIEW_MODES: [ViewMode; 3] = [ViewMode::Main, ViewMode::SlotManager, ViewMode::Settings];

    /// Compile-time tripwire for the two hand-maintained arrays above.
    ///
    /// Neither enum exposes an authoritative all-variants list, so a runtime
    /// test iterating `SECTIONS`/`VIEW_MODES` cannot notice a variant missing
    /// from them. These exhaustive matches can: adding a `ViewMode` or
    /// `SettingsSection` stops this module compiling, which lands the author
    /// directly on the arrays and `EXPECTED_ROUTING` that must grow with it.
    /// The index each arm yields is what the assertion below pins.
    const fn section_index(section: SettingsSection) -> usize {
        match section {
            SettingsSection::Audio => 0,
            SettingsSection::Library => 1,
            SettingsSection::Hotkeys => 2,
            SettingsSection::Appearance => 3,
            SettingsSection::About => 4,
        }
    }

    const fn view_mode_index(view_mode: ViewMode) -> usize {
        match view_mode {
            ViewMode::Main => 0,
            ViewMode::SlotManager => 1,
            ViewMode::Settings => 2,
        }
    }

    /// Ties the arrays to the matches above — each must list every variant
    /// once, in index order — and the table to the arrays, so a state added
    /// to one but not the others fails the build rather than a later assert.
    const _: () = {
        let mut i = 0;
        while i < SECTIONS.len() {
            assert!(section_index(SECTIONS[i]) == i);
            i += 1;
        }
        let mut i = 0;
        while i < VIEW_MODES.len() {
            assert!(view_mode_index(VIEW_MODES[i]) == i);
            i += 1;
        }
        assert!(EXPECTED_ROUTING.len() == VIEW_MODES.len() * SECTIONS.len() * 2);
    };

    /// Every `(view_mode, section, staged-search)` state with its intended
    /// target written out literally, rather than re-deriving it from
    /// [`active_filter_target`]'s own match — a mirrored oracle can only fail
    /// when the two copies diverge, never when the rule itself is wrong.
    #[rustfmt::skip]
    const EXPECTED_ROUTING: [(ViewMode, SettingsSection, bool, Option<FilterTarget>); 30] = [
        // The main grid owns typing regardless of any settings state behind it.
        (ViewMode::Main, SettingsSection::Audio,      false, Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::Audio,      true,  Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::Library,    false, Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::Library,    true,  Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::Hotkeys,    false, Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::Hotkeys,    true,  Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::Appearance, false, Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::Appearance, true,  Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::About,      false, Some(FilterTarget::Tiles)),
        (ViewMode::Main, SettingsSection::About,      true,  Some(FilterTarget::Tiles)),
        // The slot manager always owns its own search bar, regardless of
        // whatever settings state sits behind it.
        (ViewMode::SlotManager, SettingsSection::Audio,      false, Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::Audio,      true,  Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::Library,    false, Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::Library,    true,  Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::Hotkeys,    false, Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::Hotkeys,    true,  Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::Appearance, false, Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::Appearance, true,  Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::About,      false, Some(FilterTarget::Slots)),
        (ViewMode::SlotManager, SettingsSection::About,      true,  Some(FilterTarget::Slots)),
        // Settings routes to the bindings list only on Shortcuts, and only
        // while the staged settings search is not itself claiming keystrokes.
        (ViewMode::Settings, SettingsSection::Audio,      false, None),
        (ViewMode::Settings, SettingsSection::Audio,      true,  None),
        (ViewMode::Settings, SettingsSection::Library,    false, None),
        (ViewMode::Settings, SettingsSection::Library,    true,  None),
        (ViewMode::Settings, SettingsSection::Hotkeys,    false, Some(FilterTarget::Hotkeys)),
        (ViewMode::Settings, SettingsSection::Hotkeys,    true,  None),
        (ViewMode::Settings, SettingsSection::Appearance, false, None),
        (ViewMode::Settings, SettingsSection::Appearance, true,  None),
        (ViewMode::Settings, SettingsSection::About,      false, None),
        (ViewMode::Settings, SettingsSection::About,      true,  None),
    ];

    /// The table is only a totality proof if it enumerates every state, so
    /// this pins that it covers the full `VIEW_MODES × SECTIONS × searching`
    /// product exactly once — no gaps, no contradicting duplicate rows.
    ///
    /// Keeping those arrays complete is the compile-time guard's job, not
    /// this test's: both are hand-maintained, so a variant missing from an
    /// array would be missing from this loop too.
    #[test]
    fn expectation_table_covers_every_state_exactly_once() {
        for view_mode in VIEW_MODES {
            for section in SECTIONS {
                for searching in [false, true] {
                    let matches = EXPECTED_ROUTING
                        .iter()
                        .filter(|(m, s, q, _)| *m == view_mode && *s == section && *q == searching)
                        .count();
                    assert_eq!(
                        matches, 1,
                        "view_mode={view_mode:?} section={section:?} searching={searching}"
                    );
                }
            }
        }
    }

    #[test]
    fn routing_is_total_and_mutually_exclusive_across_every_state() {
        for (view_mode, section, searching, expected) in EXPECTED_ROUTING {
            let mut app = HonkHonk::new_for_test();
            app.view_mode = view_mode;
            app.settings_ui.select_section(section);
            if searching {
                let _ = app.settings_ui.replace_query("query".into());
            }

            assert_eq!(
                active_filter_target(&app),
                expected,
                "view_mode={view_mode:?} section={section:?} searching={searching}"
            );
        }
    }

    #[test]
    fn main_view_always_targets_tiles_regardless_of_settings_state() {
        let mut app = HonkHonk::new_for_test();
        app.view_mode = ViewMode::Main;
        app.settings_ui.select_section(SettingsSection::Hotkeys);

        assert_eq!(active_filter_target(&app), Some(FilterTarget::Tiles));
    }

    #[test]
    fn staged_settings_search_takes_priority_over_hotkeys_filter() {
        let mut app = HonkHonk::new_for_test();
        app.view_mode = ViewMode::Settings;
        app.settings_ui.select_section(SettingsSection::Hotkeys);
        let _ = app.settings_ui.replace_query("theme".into());

        assert_eq!(active_filter_target(&app), None);
    }

    #[test]
    fn slot_manager_always_targets_its_own_filter() {
        let mut app = HonkHonk::new_for_test();
        app.view_mode = ViewMode::SlotManager;

        assert_eq!(active_filter_target(&app), Some(FilterTarget::Slots));
    }
}
