//! Sort-menu overlay for the slot manager's sort chip (#198). Mirrors
//! `hotkeys::view_hotkey_sort_overlay` exactly, gating on `ViewMode` instead
//! of `SettingsSection`: the slot manager is a `ViewMode`, not a settings
//! section, so a stale `sort_menu_anchor` left over from the tiles view or
//! Settings → Shortcuts must not leak an overlay onto this view either.

use iced::Element;

use crate::app::slot_sort::SlotSortKey;
use crate::app::{HonkHonk, Message, ViewMode};
use crate::ui::list_controls::sort;
use crate::ui::theme::Theme;

impl HonkHonk {
    /// Sort-menu overlay for the slot manager's sort chip. Reuses the same
    /// `sort_menu_anchor` field as every other list-controls sort menu, so
    /// it is only rendered while both the anchor is set *and* the slot
    /// manager is the view currently on screen — otherwise a menu opened on
    /// the slot manager would keep rendering after switching views (or the
    /// anchor is stale from another view's sort menu).
    pub(crate) fn view_slot_sort_overlay(&self, theme: Theme) -> Option<Element<'_, Message>> {
        if self.view_mode != ViewMode::SlotManager {
            return None;
        }
        let anchor = self.sort_menu_anchor?;
        Some(sort::view_sort_menu_overlay(
            sort::SortMenu {
                state: self.slot_sort_state(),
                options: &SlotSortKey::ALL,
                theme,
                anchor,
                window_size: self.window_size,
            },
            |key| Message::SelectSlotSort(key.id()),
            Message::DismissSlotSortMenu,
        ))
    }
}

#[cfg(test)]
mod tests {
    use iced::Point;

    use super::*;

    fn open_slot_sort_menu(app: &mut HonkHonk) {
        app.view_mode = ViewMode::SlotManager;
        app.toggle_slot_sort_menu();
        assert!(app.sort_menu_anchor.is_some(), "setup: menu did not open");
    }

    #[test]
    fn overlay_renders_when_anchor_set_and_slot_manager_active() {
        let mut app = HonkHonk::new_for_test();
        open_slot_sort_menu(&mut app);

        assert!(app.view_slot_sort_overlay(Theme::Light).is_some());
    }

    #[test]
    fn overlay_hidden_without_an_anchor() {
        let app = HonkHonk::new_for_test();
        assert!(app.view_mode != ViewMode::SlotManager || app.sort_menu_anchor.is_none());

        assert!(app.view_slot_sort_overlay(Theme::Light).is_none());
    }

    /// Slot manager is active but no sort menu was ever opened: the anchor
    /// guard alone must suppress the overlay. Without this test, dropping
    /// the `sort_menu_anchor?` early return (e.g. an accidental merge with
    /// the hotkeys overlay variant) would go undetected — every other test
    /// in this file pairs `ViewMode::SlotManager` only with an anchor that
    /// is `Some`.
    #[test]
    fn overlay_hidden_in_slot_manager_with_no_anchor_set() {
        let mut app = HonkHonk::new_for_test();
        app.view_mode = ViewMode::SlotManager;
        assert_eq!(app.sort_menu_anchor, None, "setup: anchor should be unset");

        assert!(app.view_slot_sort_overlay(Theme::Light).is_none());
    }

    #[test]
    fn overlay_hidden_when_anchor_set_but_a_different_view_is_active() {
        let mut app = HonkHonk::new_for_test();
        open_slot_sort_menu(&mut app);
        app.view_mode = ViewMode::Main;

        assert!(app.view_slot_sort_overlay(Theme::Light).is_none());
    }

    /// A stale anchor left over from another view's sort menu must not leak
    /// an overlay onto the slot manager.
    #[test]
    fn overlay_hidden_for_stale_anchor_on_a_non_slot_manager_view() {
        let mut app = HonkHonk::new_for_test();
        app.view_mode = ViewMode::Main;
        app.sort_menu_anchor = Some(Point::ORIGIN);

        assert!(app.view_slot_sort_overlay(Theme::Light).is_none());
    }
}
