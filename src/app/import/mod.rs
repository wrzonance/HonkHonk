use super::{HonkHonk, Message};
use crate::state::import::{ImportError, ImportReport, ScanReport};
use iced::Task;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
mod actions;
mod edits;
mod preview;
mod scanning;
mod view;

#[derive(Debug, Clone, PartialEq)]
pub enum ImportMessage {
    Open,
    Cancel,
    Path(String),
    Scan,
    Pick,
    Picked(u64, Result<Option<PathBuf>, ImportError>),
    Drop(PathBuf),
    Scanned(u64, ScanReport),
    Select(usize, bool),
    SelectAll(bool),
    Name(usize, String),
    Category(usize, String),
    Color(usize, u8),
    BatchCategory(String),
    ApplyCategory,
    BatchColor(u8),
    Normalize(bool),
    Trim(bool),
    Filter(String),
    Preview(usize),
    Previewed(u64, u64, Result<crate::audio::CachedPcm, ImportError>),
    Confirm,
    Confirmed(u64, PathBuf, ImportReport),
}

#[derive(Default)]
pub(super) struct ImportState {
    pub open: bool,
    pub scanning: bool,
    pub active_scan: Option<u64>,
    pub pending_scan: bool,
    pub source_limit: bool,
    pub busy: bool,
    pub epoch: u64,
    pub preview: u64,
    pub path: String,
    pub sources: Vec<PathBuf>,
    pub batch_category: String,
    pub filter: String,
    pub report: ScanReport,
    pub status: String,
    pub cancel: Arc<AtomicBool>,
}

impl HonkHonk {
    pub(super) fn update_import(&mut self, message: ImportMessage) -> Task<Message> {
        if let ImportMessage::Confirmed(epoch, path, report) = message {
            return self.import_confirmed(epoch, path, report);
        }
        if let ImportMessage::Scanned(epoch, report) = message {
            return self.import_scanned(epoch, report);
        }
        if self.import.busy {
            return Task::none();
        }
        match message {
            ImportMessage::Open => {
                self.open_import();
                Task::none()
            }
            ImportMessage::Drop(path) => self.drop_import(path),
            ImportMessage::Cancel => {
                self.close_import();
                Task::none()
            }
            _ if !self.import.open => Task::none(),
            ImportMessage::Scan => {
                self.import.sources = vec![PathBuf::from(&self.import.path)];
                self.import.source_limit = false;
                self.import.report = ScanReport::default();
                self.scan_import()
            }
            ImportMessage::Pick => self.pick_import(),
            ImportMessage::Picked(epoch, result) => self.import_picked(epoch, result),
            ImportMessage::Confirm => self.confirm_import(),
            ImportMessage::Preview(index) => self.preview_import(index),
            ImportMessage::Previewed(epoch, generation, result) => {
                self.import_previewed(epoch, generation, result);
                Task::none()
            }
            edit => {
                self.edit_import(edit);
                Task::none()
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scan_tests;
