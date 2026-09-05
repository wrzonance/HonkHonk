use super::*;
use crate::state::import;

impl HonkHonk {
    pub(super) fn pick_import(&mut self) -> Task<Message> {
        let epoch = self.import.epoch;
        Task::perform(
            async {
                super::super::library_actions::pick_directory()
                    .await
                    .map_err(ImportError::from)
            },
            move |result| Message::Import(ImportMessage::Picked(epoch, result)),
        )
    }

    pub(super) fn import_picked(
        &mut self,
        epoch: u64,
        result: Result<Option<PathBuf>, ImportError>,
    ) -> Task<Message> {
        if self.import.epoch != epoch {
            return Task::none();
        }
        match result {
            Ok(Some(path)) => {
                self.import.path = path.to_string_lossy().into_owned();
                self.import.sources = vec![path];
                self.import.source_limit = false;
                self.import.report = ScanReport::default();
                self.scan_import()
            }
            Ok(None) => Task::none(),
            Err(error) => {
                self.import.status = error.to_string();
                Task::none()
            }
        }
    }

    pub(super) fn confirm_import(&mut self) -> Task<Message> {
        if self.import.scanning
            || !self
                .import
                .report
                .rows
                .iter()
                .any(|r| r.selected && r.error.is_none())
        {
            return Task::none();
        }
        let destination = match import::destination() {
            Ok(path) => path,
            Err(error) => {
                self.import.status = error.to_string();
                return Task::none();
            }
        };
        self.stop_import_preview();
        self.import.busy = true;
        self.import.status = "Importing selected copies…".into();
        let epoch = self.import.epoch;
        let rows = self.import.report.rows.clone();
        let output = destination.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || import::confirm(&rows, &output))
                    .await
                    .unwrap_or_else(|error| ImportReport {
                        imported: vec![],
                        failures: vec![(
                            PathBuf::new(),
                            anyhow::Error::new(error)
                                .context("import worker failed")
                                .into(),
                        )],
                    })
            },
            move |report| {
                Message::Import(ImportMessage::Confirmed(epoch, destination.clone(), report))
            },
        )
    }

    pub(super) fn import_confirmed(
        &mut self,
        epoch: u64,
        destination: PathBuf,
        report: ImportReport,
    ) -> Task<Message> {
        if !self.import.open || !self.import.busy || self.import.epoch != epoch {
            return Task::none();
        }
        self.import.busy = false;
        let count = report.imported.len();
        let sources: std::collections::HashSet<_> =
            report.imported.iter().map(|r| r.source.clone()).collect();
        for imported in report.imported {
            let mut meta = self.sound_meta.get(&imported.sound.id);
            meta.display_name = Some(imported.name);
            meta.color = Some(imported.color);
            self.sound_meta.set(imported.sound.id.clone(), meta);
            self.sounds.push(imported.sound);
        }
        if count > 0 {
            if !self.config.sound_directories.contains(&destination) {
                self.config.sound_directories.push(destination);
                self.persist_config();
            }
            self.save_import_metadata();
            self.refresh_filtered_sounds();
            self.durations_loaded = false;
            self.duration_scan_pairs = Arc::new(
                self.sounds
                    .iter()
                    .map(|s| (s.id.clone(), s.path.clone()))
                    .collect(),
            );
        }
        self.import
            .report
            .rows
            .retain(|r| !r.selected || !sources.contains(&r.source));
        self.import.report.errors = report
            .failures
            .into_iter()
            .map(|(path, e)| anyhow::anyhow!("{}: {e}", path.display()).into())
            .collect();
        self.import.status = format!(
            "Imported {count} sounds. {} failed.",
            self.import.report.errors.len()
        );
        Task::none()
    }

    fn save_import_metadata(&mut self) {
        if self.persist
            && let Err(error) = self.sound_meta.save()
        {
            self.notices.push(
                super::super::Notice::error(
                    "Import metadata could not be saved",
                    error.to_string(),
                ),
                std::time::Instant::now(),
            );
        }
    }
}
