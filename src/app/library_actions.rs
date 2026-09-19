//! Library actions and their state transitions.

use super::*;

pub(super) async fn pick_directory() -> anyhow::Result<Option<std::path::PathBuf>> {
    use anyhow::Context;
    use ashpd::desktop::file_chooser::SelectedFiles;

    let request = SelectedFiles::open_file()
        .title("Select Sound Folder")
        .directory(true)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))
        .context("file chooser portal send failed")?;

    let files = match request.response() {
        Ok(f) => f,
        Err(ashpd::Error::Response(_)) => return Ok(None), // user cancelled
        Err(e) => return Err(anyhow::anyhow!(e).context("file chooser response failed")),
    };

    let uri = match files.uris().first() {
        Some(u) => u.clone(),
        None => return Ok(None),
    };

    let url = url::Url::parse(uri.as_str()).with_context(|| format!("parsing file URI: {uri}"))?;

    url.to_file_path()
        .map(Some)
        .map_err(|_| anyhow::anyhow!("URI is not a file:// path: {uri}"))
}

impl HonkHonk {
    pub(super) fn rescan_library(&mut self) -> Task<Message> {
        let scan = match crate::state::Library::scan(&self.config.sound_directories) {
            Ok(scan) => scan,
            Err(e) => {
                tracing::warn!(dirs = ?self.config.sound_directories, error = %e, "library rescan failed");
                return Task::none();
            }
        };
        self.apply_library_scan(scan);
        Task::none()
    }

    pub(super) fn add_sound_directory(&mut self, path: std::path::PathBuf) -> Task<Message> {
        if !self.config.sound_directories.contains(&path) {
            self.config.sound_directories.push(path);
            self.persist_config();
            self.update(Message::RescanLibrary)
        } else {
            Task::none()
        }
    }

    pub(super) fn remove_sound_directory(&mut self, path: std::path::PathBuf) -> Task<Message> {
        self.config.sound_directories.retain(|p| p != &path);
        self.persist_config();
        self.update(Message::RescanLibrary)
    }
}
