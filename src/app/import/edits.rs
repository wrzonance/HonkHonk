use super::*;
use std::sync::atomic::Ordering;

impl HonkHonk {
    pub(super) fn open_import(&mut self) {
        if !self.import.open {
            self.import.epoch = self.import.epoch.wrapping_add(1);
            self.import.open = true;
        }
    }

    pub(super) fn close_import(&mut self) {
        self.import.cancel.store(true, Ordering::Relaxed);
        self.stop_import_preview();
        self.import = ImportState {
            active_scan: self.import.active_scan,
            epoch: self.import.epoch.wrapping_add(1),
            preview: self.import.preview,
            ..Default::default()
        };
    }

    pub(super) fn edit_import(&mut self, edit: ImportMessage) {
        if self.import.scanning {
            return;
        }
        match edit {
            ImportMessage::Path(path) => self.import.path = path,
            ImportMessage::Filter(filter) => self.import.filter = filter,
            ImportMessage::BatchCategory(category) => self.import.batch_category = category,
            ImportMessage::Select(index, selected) => {
                self.edit_row(index, |r| r.selected = selected)
            }
            ImportMessage::Name(index, name) => self.edit_row(index, |r| r.name = name),
            ImportMessage::Category(index, category) => {
                self.edit_row(index, |r| r.category = category)
            }
            ImportMessage::Color(index, color) => self.edit_row(index, |r| r.color = color % 8),
            batch => self.batch_import(batch),
        }
    }

    fn edit_row(&mut self, index: usize, edit: impl FnOnce(&mut crate::state::import::ImportRow)) {
        if let Some(row) = self
            .import
            .report
            .rows
            .get_mut(index)
            .filter(|r| r.error.is_none())
        {
            edit(row);
        }
    }

    fn batch_import(&mut self, edit: ImportMessage) {
        for row in &mut self.import.report.rows {
            if row.error.is_some() {
                continue;
            }
            if let ImportMessage::SelectAll(selected) = edit {
                row.selected = selected;
            }
            if !row.selected {
                continue;
            }
            match &edit {
                ImportMessage::ApplyCategory => row.category = self.import.batch_category.clone(),
                ImportMessage::BatchColor(color) => row.color = color % 8,
                ImportMessage::Normalize(enabled) => row.normalize = *enabled,
                ImportMessage::Trim(enabled) => row.trim = *enabled,
                _ => {}
            }
        }
    }
}
