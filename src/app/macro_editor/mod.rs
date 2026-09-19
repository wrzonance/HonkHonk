//! Macro editor controller. The store is the saved buffer; selection is by ID.
mod edits;
#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod lifecycle_tests;
mod list;
#[cfg(test)]
mod tests;
mod view;

use super::{HonkHonk, Message, ViewMode};
use crate::ui::list_controls::filter::FilterState;
use crate::ui::list_controls::sort::{Direction, SortState};
use crate::ui::macros::timeline::TimelineState;
pub use edits::Edit;
use iced::{Point, Task};
pub use list::MacroSortKey;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum EditorMessage {
    New,
    Select(String),
    Delete,
    BeginRename,
    Rename(String),
    EndRename,
    Filter(String),
    Sort(MacroSortKey),
    ToggleDirection,
    ToggleSort,
    Edit(Edit),
    PaletteDrag(PathBuf),
    MoveStart(usize, f32),
    Pointer(Point),
    Release(Option<Point>),
    Menu(usize),
    CloseMenu,
    Effects(Box<Message>),
    Snap(bool),
    Play,
    Stop,
}

#[derive(Debug, Clone)]
pub(crate) enum Drag {
    Sound(PathBuf),
    Step { index: usize, grab: f32 },
}

pub(crate) struct EditorState {
    pub active: Option<String>,
    pub filter: FilterState,
    pub sort: SortState<MacroSortKey>,
    pub sort_open: bool,
    pub text_entry_active: bool,
    pub menu: Option<usize>,
    pub effects: crate::ui::effects_panel::EffectsUiState,
    pub dragging: Option<Drag>,
    pub pointer: Option<Point>,
    pub snap: bool,
    pub timeline: TimelineState,
    pub preview_start: Option<Instant>,
    pub playhead_ms: u64,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            active: None,
            filter: FilterState::default(),
            sort: SortState::new(MacroSortKey::Name, Direction::Ascending),
            sort_open: false,
            text_entry_active: false,
            menu: None,
            effects: Default::default(),
            dragging: None,
            pointer: None,
            snap: true,
            timeline: Default::default(),
            preview_start: None,
            playhead_ms: 0,
        }
    }
}

impl HonkHonk {
    pub(super) fn show_macros(&mut self) -> Task<Message> {
        self.dismiss_sound_sort_menu();
        self.view_mode = ViewMode::Macros;
        self.macro_editor.text_entry_active = false;
        self.macro_editor.dragging = None;
        self.macro_editor.pointer = None;
        self.macro_editor.menu = None;
        self.macro_editor.sort_open = false;
        self.adopt_macro_draft();
        self.macro_editor.restore_sort(&self.config);
        if self.macro_editor.active.is_none() {
            self.macro_editor.active = self.macros.iter().next().map(|m| m.id.clone());
        }
        self.sync_macro_timeline();
        Task::none()
    }

    pub(super) fn adopt_macro_draft(&mut self) {
        let Some(draft) = self.macro_editor_draft.take() else {
            return;
        };
        let id = self.macros.add(draft.name).id.clone();
        self.macros.replace_steps(&id, draft.steps);
        self.macro_editor.active = Some(id);
        self.persist_macros();
        self.sync_macro_timeline();
    }

    pub(super) fn sync_macro_timeline(&mut self) {
        let selected = self
            .macro_editor
            .active
            .as_deref()
            .and_then(|id| self.macros.get(id));
        self.macro_editor
            .timeline
            .sync(selected, &self.sounds, self.config.theme);
    }

    pub(super) fn tick_macro_preview(&mut self, now: Instant) {
        if self.macro_playback.is_none() {
            self.macro_editor.preview_start = None;
        }
        self.macro_editor.playhead_ms = self
            .macro_editor
            .preview_start
            .map(|start| now.saturating_duration_since(start).as_millis() as u64)
            .unwrap_or(0);
    }

    pub(super) fn update_macro_editor(&mut self, message: EditorMessage) -> Task<Message> {
        if self.view_mode != ViewMode::Macros {
            return Task::none();
        }
        match message {
            EditorMessage::Edit(edit) => self.edit_macro(edit),
            EditorMessage::PaletteDrag(path) => {
                self.macro_editor.pointer = None;
                self.macro_editor.dragging = Some(Drag::Sound(path));
            }
            EditorMessage::MoveStart(index, grab) => {
                self.macro_editor.pointer = None;
                self.macro_editor.dragging = Some(Drag::Step { index, grab });
            }
            EditorMessage::Pointer(point) => self.macro_editor.pointer = Some(point),
            EditorMessage::Release(point) => self.release_macro_drag(point),
            EditorMessage::Menu(index) => self.open_step_menu(index),
            EditorMessage::CloseMenu => self.macro_editor.menu = None,
            EditorMessage::Effects(message) => self.edit_step_effects(*message),
            EditorMessage::Snap(enabled) => self.macro_editor.snap = enabled,
            EditorMessage::Play => return self.preview_macro(),
            EditorMessage::Stop => {
                self.cancel_macro();
                self.tick_macro_preview(Instant::now());
            }
            other => self.update_macro_list(other),
        }
        self.sync_macro_timeline();
        Task::none()
    }

    fn preview_macro(&mut self) -> Task<Message> {
        let Some(id) = self.macro_editor.active.clone() else {
            return Task::none();
        };
        let task = self.play_macro(&id);
        self.macro_editor.preview_start = self.macro_playback.as_ref().map(|_| Instant::now());
        self.macro_editor.playhead_ms = 0;
        task
    }

    fn persist_macros(&self) {
        if self.persist
            && let Err(error) = self.macros.save()
        {
            tracing::warn!(%error, "saving macro edits failed");
        }
    }
}
