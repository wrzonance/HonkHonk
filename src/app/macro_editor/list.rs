use super::*;
use crate::state::{AppConfig, Macro, SortPref};
use crate::ui::list_controls::{
    filter::filter_items,
    sort::{SortKey, SortLabel},
};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroSortKey {
    Name,
    Created,
    Length,
}

impl MacroSortKey {
    pub const ALL: [Self; 3] = [Self::Name, Self::Created, Self::Length];
    pub fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Created => "created",
            Self::Length => "length",
        }
    }
}
impl SortLabel for MacroSortKey {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Created => "Created",
            Self::Length => "Length",
        }
    }
}

pub(super) struct MacroRow<'a> {
    pub value: &'a Macro,
    created: usize,
    length: u64,
}
impl SortKey<MacroRow<'_>> for MacroSortKey {
    fn compare(self, a: &MacroRow<'_>, b: &MacroRow<'_>) -> Ordering {
        match self {
            Self::Name => a
                .value
                .name
                .to_lowercase()
                .cmp(&b.value.name.to_lowercase()),
            Self::Created => a.created.cmp(&b.created),
            Self::Length => a.length.cmp(&b.length),
        }
    }
    fn tie_break(self, a: &MacroRow<'_>, b: &MacroRow<'_>) -> Ordering {
        a.created.cmp(&b.created)
    }
}

impl EditorState {
    pub fn restore_sort(&mut self, config: &AppConfig) {
        let Some(pref) = config.sort_prefs.get("macros") else {
            return;
        };
        if let Some(key) = MacroSortKey::ALL
            .into_iter()
            .find(|key| key.id() == pref.key())
        {
            let direction = if pref.direction() == "descending" {
                Direction::Descending
            } else {
                Direction::Ascending
            };
            self.sort = SortState::new(key, direction);
        }
    }
}

impl HonkHonk {
    pub(super) fn macro_rows(&self) -> Vec<MacroRow<'_>> {
        // MacroStore preserves insertion order in JSON, including legacy files.
        // This is the creation-order key; renames and edits never reorder it.
        let matches = filter_items(&self.macros.0, self.macro_editor.filter.query(), |m| {
            [&m.name]
        });
        let rows = self
            .macros
            .iter()
            .enumerate()
            .filter(|(_, m)| matches.contains(m))
            .map(|(created, value)| {
                let length = value
                    .steps
                    .iter()
                    .map(|step| {
                        let duration = self
                            .sounds
                            .iter()
                            .find(|s| s.path == step.sound)
                            .and_then(|s| s.duration_ms)
                            .unwrap_or(0);
                        step.start_offset_ms.saturating_add(duration)
                    })
                    .max()
                    .unwrap_or(0);
                MacroRow {
                    value,
                    created,
                    length,
                }
            });
        self.macro_editor.sort.sorted(rows)
    }

    pub(super) fn update_macro_list(&mut self, message: EditorMessage) {
        match message {
            EditorMessage::New => {
                let id = self.macros.add("New macro").id.clone();
                self.select_macro(id);
                self.persist_macros();
            }
            EditorMessage::Select(id) => self.select_macro(id),
            EditorMessage::Delete => self.delete_macro(),
            EditorMessage::BeginRename => self.macro_editor.text_entry_active = true,
            EditorMessage::EndRename => self.macro_editor.text_entry_active = false,
            EditorMessage::Rename(name) => {
                if let Some(id) = &self.macro_editor.active {
                    self.macros.rename(id, name);
                    self.persist_macros();
                }
            }
            EditorMessage::Filter(query) => self.macro_editor.filter.replace(query),
            EditorMessage::Sort(key) => {
                self.macro_editor.sort.select(key);
                self.save_macro_sort();
            }
            EditorMessage::ToggleDirection => {
                self.macro_editor.sort.toggle_direction();
                self.save_macro_sort();
            }
            EditorMessage::ToggleSort => {
                self.macro_editor.sort_open = !self.macro_editor.sort_open;
                self.sort_menu_anchor = self.macro_editor.sort_open.then_some(self.cursor_pos);
            }
            _ => {}
        }
    }

    fn save_macro_sort(&mut self) {
        let sort = self.macro_editor.sort;
        self.config.sort_prefs.insert(
            "macros".into(),
            SortPref::new(sort.key().id(), sort.direction().id()),
        );
        self.macro_editor.sort_open = false;
        self.sort_menu_anchor = None;
        self.persist_config();
    }

    fn select_macro(&mut self, id: String) {
        if self.macros.get(&id).is_none() {
            return;
        }
        self.cancel_macro();
        self.macro_editor.active = Some(id);
        self.macro_editor.menu = None;
        self.macro_editor.dragging = None;
        self.macro_editor.pointer = None;
        self.macro_editor.text_entry_active = false;
    }

    fn delete_macro(&mut self) {
        self.cancel_macro();
        if let Some(id) = self.macro_editor.active.take() {
            self.macros.remove(&id);
            self.persist_macros();
        }
        self.macro_editor.active = self.macros.iter().next().map(|m| m.id.clone());
        self.macro_editor.menu = None;
        self.macro_editor.dragging = None;
        self.macro_editor.pointer = None;
        self.macro_editor.text_entry_active = false;
    }
}
