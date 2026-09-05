use iced::event::Status;
use iced::keyboard;

use super::{HonkHonk, Message};
use crate::ui::list_controls::filter::{Activation, ActivationContext, FilterState};
use crate::ui::search_bar;
use targets::{FilterTarget, active_filter_target};

mod cache;
#[cfg(test)]
mod grid_tests;
mod targets;
#[cfg(test)]
mod tests;

#[cfg(test)]
use super::{SettingsSection, ViewMode};

pub(super) fn type_to_filter_text(event: &iced::Event, status: Status) -> Option<String> {
    if status != Status::Ignored {
        return None;
    }

    let iced::Event::Keyboard(keyboard::Event::KeyPressed {
        modifiers,
        text: Some(text),
        ..
    }) = event
    else {
        return None;
    };

    if modifiers.control() || modifiers.alt() || modifiers.logo() {
        return None;
    }

    (!text.is_empty() && text.chars().all(|character| !character.is_control()))
        .then(|| text.to_string())
}

/// The search input that type-to-filter focuses when it seeds `target`.
///
/// A named seam rather than two inline `focus(..)` literals: Iced's `Task` is
/// opaque, so a test can only observe *that* an operation was scheduled, never
/// which widget it targets. Routing both branches through here makes the
/// id-per-target mapping directly assertable.
fn filter_input_id(target: FilterTarget) -> iced::widget::Id {
    match target {
        FilterTarget::Tiles => search_bar::input_id(),
        FilterTarget::Hotkeys => search_bar::hotkeys_input_id(),
        FilterTarget::Slots => search_bar::slots_input_id(),
        FilterTarget::Macros => search_bar::macros_input_id(),
    }
}

impl HonkHonk {
    pub(super) fn select_sound_category(&mut self, category: Option<String>) {
        if self.active_category == category {
            return;
        }
        self.active_category = category;
        self.refresh_filtered_sounds();
    }

    pub(super) fn replace_filter_query(&mut self, query: String) {
        let changed = self.filter.query() != query;
        self.filter.replace(query);
        if changed {
            self.refresh_filtered_sounds();
        }
    }

    fn filter_context(&self) -> ActivationContext {
        let activation = if active_filter_target(self).is_some() {
            Activation::TypeToFilter
        } else {
            Activation::ClickOnly
        };
        ActivationContext::new(activation, self.filter_is_blocked())
    }

    fn filter_is_blocked(&self) -> bool {
        self.context_menu.is_some()
            || self.editor_sound_id.is_some()
            || self.macro_editor_draft.is_some()
            || (self.view_mode == super::ViewMode::Macros
                && (self.macro_editor.text_entry_active
                    || self.macro_editor.menu.is_some()
                    || self.macro_editor.sort_open))
            || self.effects_panel.is_visible()
            || self.sort_menu_anchor.is_some()
    }

    /// Returns the `FilterState` owned by `target`. All three fields share a
    /// type, so this is the single seam that keeps tiles/hotkeys/slots
    /// dispatch from duplicating match arms across every mutator below.
    fn filter_state_mut(&mut self, target: FilterTarget) -> &mut FilterState {
        match target {
            FilterTarget::Tiles => &mut self.filter,
            FilterTarget::Hotkeys => &mut self.hotkey_filter,
            FilterTarget::Slots => &mut self.slot_filter,
            FilterTarget::Macros => &mut self.macro_editor.filter,
        }
    }

    pub(super) fn handle_type_to_filter(&mut self, text: &str) -> iced::Task<Message> {
        if !self.filter_context().allows_typing() {
            return iced::Task::none();
        }

        match active_filter_target(self) {
            Some(FilterTarget::Tiles) => self.insert_tiles_filter_text(text),
            Some(FilterTarget::Hotkeys) => {
                self.hotkey_filter.insert(text);
                iced::widget::operation::focus(filter_input_id(FilterTarget::Hotkeys))
            }
            Some(FilterTarget::Slots) => {
                self.slot_filter.insert(text);
                iced::widget::operation::focus(filter_input_id(FilterTarget::Slots))
            }
            None => iced::Task::none(),
            Some(FilterTarget::Macros) => {
                self.macro_editor.filter.insert(text);
                iced::widget::operation::focus(filter_input_id(FilterTarget::Macros))
            }
        }
    }

    /// Tiles-specific body of `handle_type_to_filter`: unlike the Shortcuts
    /// bindings list (`hotkey_rows()` is pure and rebuilds on every read),
    /// the tiles grid caches filtered indices and must be refreshed, then
    /// re-focuses the shared search bar.
    fn insert_tiles_filter_text(&mut self, text: &str) -> iced::Task<Message> {
        self.filter.insert(text);
        if !text.is_empty() {
            self.refresh_filtered_sounds();
        }
        iced::widget::operation::focus(filter_input_id(FilterTarget::Tiles))
    }

    pub(super) fn handle_escape(&mut self, event_was_captured: bool) -> iced::Task<Message> {
        if self.view_mode == super::ViewMode::Macros
            && (self.macro_editor.menu.take().is_some()
                || self.macro_editor.text_entry_active
                || self.macro_editor.sort_open
                || self.macro_editor.dragging.is_some())
        {
            self.macro_editor.text_entry_active = false;
            self.macro_editor.sort_open = false;
            self.macro_editor.dragging = None;
            self.macro_editor.pointer = None;
            self.sort_menu_anchor = None;
            return iced::Task::none();
        }
        if self.dismiss_sound_sort_menu() {
            return iced::Task::none();
        }
        if self.context_menu.is_some() {
            self.context_menu = None;
            self.context_menu_pos = None;
        } else if self.editor_sound_id.is_some() {
            self.editor_sound_id = None;
            self.editor_draft_name.clear();
            self.editor_draft_tags.clear();
            self.editor_draft_volume = 1.0;
        } else if self.macro_editor_draft.is_some() {
            // The draft belongs to the macro editor; its own close/discard flow
            // decides its fate, so global Escape must not alter filter state.
            return iced::Task::none();
        } else if self.effects_panel.is_visible() {
            self.close_effects_panel_from_escape(std::time::Instant::now());
        } else {
            self.escape_target_filter(event_was_captured);
        }
        iced::Task::none()
    }

    /// Routes Escape to whichever view currently owns filter input, if any.
    /// A captured Escape only consumes the staged-focus flag; an uncaptured
    /// one clears the query via `escape_active_filter`.
    fn escape_target_filter(&mut self, event_was_captured: bool) {
        let Some(target) = active_filter_target(self) else {
            return;
        };
        if event_was_captured {
            self.filter_state_mut(target).consume_focus();
        } else {
            self.escape_active_filter(target);
        }
    }

    /// Clears `target`'s query on an uncaptured Escape. Only the tiles grid
    /// caches filtered indices, so only it needs a refresh once its query
    /// goes from non-empty to empty; the Shortcuts bindings list rebuilds
    /// from current state on every read and needs no such nudge.
    fn escape_active_filter(&mut self, target: FilterTarget) {
        let filter = self.filter_state_mut(target);
        let query_was_present = !filter.query().is_empty();
        filter.escape();
        let query_cleared = query_was_present && filter.query().is_empty();
        if target == FilterTarget::Tiles && query_cleared {
            self.refresh_filtered_sounds();
        }
    }
}
