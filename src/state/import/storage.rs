use super::{ImportError, ImportReport, ImportRow, Imported, transform};
use anyhow::Context;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

pub fn destination() -> Result<PathBuf, ImportError> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.data_dir().join("honkhonk/imported"))
        .ok_or_else(|| anyhow::anyhow!("XDG data directory unavailable").into())
}

pub fn confirm(rows: &[ImportRow], destination: &Path) -> ImportReport {
    let mut report = ImportReport::default();
    for row in rows.iter().filter(|r| r.selected && r.error.is_none()) {
        match import_one(row, destination) {
            Ok(imported) => report.imported.push(imported),
            Err(error) => report.failures.push((row.source.clone(), error)),
        }
    }
    report
}

fn category_path(destination: &Path, category: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !category.trim().is_empty()
            && !matches!(category, "." | "..")
            && !category.contains(['/', '\\'])
            && !category.chars().any(char::is_control),
        "Category must be a single folder name"
    );
    fs::create_dir_all(destination).context("creating import destination")?;
    let directory = destination.join(category);
    match fs::create_dir(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            anyhow::ensure!(
                fs::symlink_metadata(&directory)
                    .context("checking category folder")?
                    .file_type()
                    .is_dir(),
                "Category destination is not a regular directory"
            );
        }
        Err(error) => return Err(error).context("creating category folder"),
    }
    Ok(directory)
}

fn create_unique(directory: &Path, extension: &str) -> anyhow::Result<(PathBuf, File)> {
    for index in 1..=100_000 {
        let path = directory.join(format!("sound-{index}.{extension}"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("creating imported copy"),
        }
    }
    anyhow::bail!("Import filename limit reached")
}

fn import_one(row: &ImportRow, destination: &Path) -> Result<Imported, ImportError> {
    if row.name.trim().is_empty() {
        return Err(anyhow::anyhow!("Sound name cannot be empty").into());
    }
    let decoded = transform::decode(&row.source)?;
    let directory = category_path(destination, &row.category)?;
    let transformed = row.normalize || row.trim;
    let extension = if transformed {
        "wav"
    } else {
        row.source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("wav")
    };
    let (path, mut file) = create_unique(&directory, extension)?;
    let result = write_copy(row, decoded, &mut file)
        .and_then(|()| file.sync_all().context("syncing imported copy"));
    if let Err(error) = result {
        drop(file);
        let cleanup = fs::remove_file(&path);
        return Err(error
            .context(format!(
                "importing {}; partial-copy cleanup: {cleanup:?}",
                row.source.display()
            ))
            .into());
    }
    let sound = crate::state::library::entry_from_path(&path)
        .ok_or_else(|| anyhow::anyhow!("Unsupported imported path {}", path.display()))?;
    Ok(Imported {
        source: row.source.clone(),
        sound,
        name: row.name.trim().into(),
        color: row.color,
    })
}

fn write_copy(
    row: &ImportRow,
    decoded: crate::audio::DecodedAudio,
    file: &mut File,
) -> anyhow::Result<()> {
    use std::io::{BufWriter, Read, Write};
    if row.normalize || row.trim {
        let prepared = transform::prepare(decoded, row.normalize, row.trim);
        let mut writer = BufWriter::new(file);
        transform::write_wav(&prepared, &mut writer).context("writing processed WAV copy")?;
        writer.flush().context("flushing processed WAV copy")
    } else {
        let mut source = File::open(&row.source)
            .context("opening import source")?
            .take(64 * 1024 * 1024 + 1);
        let copied = std::io::copy(&mut source, file).context("copying import source")?;
        anyhow::ensure!(
            copied <= 64 * 1024 * 1024,
            "Source grew beyond the import size limit"
        );
        Ok(())
    }
}
