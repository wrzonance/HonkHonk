use super::*;
use crate::state::import;
use std::sync::atomic::Ordering;

const SOURCE_LIMIT: usize = 1000;
const LIMIT_MESSAGE: &str = "Source limit reached (1000 paths); additional drops were ignored. ";

impl HonkHonk {
    pub(super) fn drop_import(&mut self, path: PathBuf) -> Task<Message> {
        self.open_import();
        if self.import.sources.contains(&path) {
            return Task::none();
        }
        if self.import.sources.len() >= SOURCE_LIMIT {
            self.import.source_limit = true;
            self.import.status = LIMIT_MESSAGE.into();
            return Task::none();
        }
        self.import.sources.push(path);
        self.scan_import()
    }

    pub(super) fn scan_import(&mut self) -> Task<Message> {
        self.import.cancel.store(true, Ordering::Relaxed);
        self.stop_import_preview();
        self.import.epoch = self.import.epoch.wrapping_add(1);
        self.import.scanning = true;
        self.import.pending_scan = true;
        self.import.status = "Scanning and analyzing sounds…".into();
        if self.import.active_scan.is_some() {
            Task::none()
        } else {
            self.start_import_scan()
        }
    }

    fn start_import_scan(&mut self) -> Task<Message> {
        self.import.pending_scan = false;
        self.import.cancel = Arc::new(AtomicBool::new(false));
        let epoch = self.import.epoch;
        self.import.active_scan = Some(epoch);
        let cancel = self.import.cancel.clone();
        let sources = self.import.sources.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || import::scan(&sources, &cancel))
                    .await
                    .unwrap_or_else(|error| ScanReport {
                        rows: vec![],
                        errors: vec![
                            anyhow::Error::new(error)
                                .context("import scan worker failed")
                                .into(),
                        ],
                    })
            },
            move |report| Message::Import(ImportMessage::Scanned(epoch, report)),
        )
    }

    pub(super) fn import_scanned(&mut self, epoch: u64, report: ScanReport) -> Task<Message> {
        if self.import.active_scan != Some(epoch) {
            return Task::none();
        }
        self.import.active_scan = None;
        if self.import.pending_scan && self.import.open {
            return self.start_import_scan();
        }
        if self.import.open && self.import.epoch == epoch && self.import.scanning {
            self.apply_import_scan(report);
        }
        Task::none()
    }

    fn apply_import_scan(&mut self, mut report: ScanReport) {
        for row in &mut report.rows {
            if let Some(previous) = self
                .import
                .report
                .rows
                .iter()
                .find(|r| r.source == row.source)
                && row.error.is_none()
                && previous.error.is_none()
            {
                let analysis = row.analysis.clone();
                *row = previous.clone();
                row.analysis = analysis;
            }
        }
        self.import.scanning = false;
        let limit = if self.import.source_limit {
            LIMIT_MESSAGE
        } else {
            ""
        };
        self.import.status = format!(
            "{limit}{} sounds found. Review selected sounds before importing.",
            report.rows.len()
        );
        self.import.report = report;
    }
}
