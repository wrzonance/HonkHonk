//! Shared, cancellation-safe preparation for background and on-demand work.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use tokio::sync::{Mutex, watch};

use super::{CachedPcm, ENVELOPE_BUCKETS, Envelope};

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAudio {
    pub pcm: Arc<CachedPcm>,
    pub envelope: Arc<Envelope>,
    identity: SourceIdentity,
}

impl PreparedAudio {
    pub fn from_pcm(pcm: CachedPcm) -> Arc<Self> {
        let envelope = Arc::new(Envelope::from_samples(
            pcm.samples.as_ref(),
            pcm.channels,
            ENVELOPE_BUCKETS,
        ));
        Arc::new(Self {
            pcm: Arc::new(pcm),
            envelope,
            identity: SourceIdentity::empty(),
        })
    }

    pub fn source_path(&self) -> Option<&Path> {
        (!self.identity.is_empty()).then_some(self.identity.path.as_path())
    }

    pub fn has_source_identity(&self) -> bool {
        !self.identity.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceIdentity {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

impl SourceIdentity {
    fn capture(path: &Path) -> Self {
        let metadata = std::fs::metadata(path).ok();
        Self {
            path: path.to_path_buf(),
            size: metadata.as_ref().map_or(0, std::fs::Metadata::len),
            modified: metadata.and_then(|metadata| metadata.modified().ok()),
        }
    }

    fn empty() -> Self {
        Self {
            path: PathBuf::new(),
            size: 0,
            modified: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.path.as_os_str().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PreparationError {
    #[error("audio source changed while preparing")]
    SourceChanged,
    #[error("audio preparation failed: {0}")]
    Decode(Arc<str>),
    #[error("audio preparation worker failed: {0}")]
    Worker(Arc<str>),
}

type Loader = dyn Fn(PathBuf, SourceIdentity) -> Result<PreparedAudio, PreparationError>
    + Send
    + Sync
    + 'static;

struct Work {
    sender: watch::Sender<Option<Result<Arc<PreparedAudio>, PreparationError>>>,
    receiver: watch::Receiver<Option<Result<Arc<PreparedAudio>, PreparationError>>>,
}

pub struct PreparationCoordinator {
    entries: Arc<Mutex<HashMap<SourceIdentity, Arc<Work>>>>,
    loader: Arc<Loader>,
}

impl Default for PreparationCoordinator {
    fn default() -> Self {
        Self::new_with_loader(|path, identity| prepare_blocking(&path, identity))
    }
}

impl PreparationCoordinator {
    pub fn new_with_loader<F>(loader: F) -> Self
    where
        F: Fn(PathBuf, SourceIdentity) -> Result<PreparedAudio, PreparationError>
            + Send
            + Sync
            + 'static,
    {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            loader: Arc::new(loader),
        }
    }

    pub async fn prepare(&self, path: &Path) -> Result<Arc<PreparedAudio>, PreparationError> {
        let path = path.to_path_buf();
        let identity = tokio::task::spawn_blocking({
            let path = path.clone();
            move || SourceIdentity::capture(&path)
        })
        .await
        .map_err(|error| PreparationError::Worker(Arc::from(error.to_string())))?;
        let (work, producer) = self.claim(identity.clone()).await;
        if producer {
            self.spawn_producer(identity, path, Arc::clone(&work));
        }
        Self::await_completion(work).await
    }

    async fn claim(&self, identity: SourceIdentity) -> (Arc<Work>, bool) {
        let mut entries = self.entries.lock().await;
        if let Some(work) = entries.get(&identity) {
            return (Arc::clone(work), false);
        }
        let (sender, receiver) = watch::channel(None);
        let work = Arc::new(Work { sender, receiver });
        entries.insert(identity, Arc::clone(&work));
        (work, true)
    }

    fn spawn_producer(&self, identity: SourceIdentity, path: PathBuf, work: Arc<Work>) {
        let entries = Arc::clone(&self.entries);
        let loader = Arc::clone(&self.loader);
        tokio::spawn(async move {
            let loader_identity = identity.clone();
            let result = tokio::task::spawn_blocking(move || loader(path, loader_identity))
                .await
                .map_err(|error| PreparationError::Worker(Arc::from(error.to_string())))
                .and_then(|result| result.map(Arc::new));
            let _ = work.sender.send(Some(result));
            entries.lock().await.remove(&identity);
        });
    }

    async fn await_completion(work: Arc<Work>) -> Result<Arc<PreparedAudio>, PreparationError> {
        let mut completion = work.receiver.clone();
        loop {
            if let Some(result) = completion.borrow().clone() {
                return result;
            }
            completion
                .changed()
                .await
                .map_err(|_| PreparationError::Worker(Arc::from("preparation producer stopped")))?;
        }
    }
}

fn prepare_blocking(
    path: &Path,
    identity: SourceIdentity,
) -> Result<PreparedAudio, PreparationError> {
    let pcm = super::processing::decode_cached(path)
        .map_err(|error| PreparationError::Decode(Arc::from(error.to_string())))?;
    if SourceIdentity::capture(path) != identity {
        return Err(PreparationError::SourceChanged);
    }
    let pcm = Arc::new(pcm);
    let envelope = Arc::new(Envelope::from_samples(
        pcm.samples.as_ref(),
        pcm.channels,
        ENVELOPE_BUCKETS,
    ));
    Ok(PreparedAudio {
        pcm,
        envelope,
        identity,
    })
}

#[cfg(test)]
pub(crate) fn wrap_for_test(pcm: CachedPcm) -> Arc<PreparedAudio> {
    PreparedAudio::from_pcm(pcm)
}

#[cfg(test)]
pub(crate) fn wrap_for_test_at(pcm: CachedPcm, path: &Path) -> Arc<PreparedAudio> {
    let envelope = Arc::new(Envelope::from_samples(
        pcm.samples.as_ref(),
        pcm.channels,
        ENVELOPE_BUCKETS,
    ));
    Arc::new(PreparedAudio {
        pcm: Arc::new(pcm),
        envelope,
        identity: SourceIdentity::capture(path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{Duration, timeout};

    fn prepared(identity: SourceIdentity) -> PreparedAudio {
        let pcm = Arc::new(CachedPcm {
            analysis: Default::default(),
            samples: Arc::new(vec![0.0]),
            sample_rate: 8_000,
            channels: 1,
            duration: Duration::from_millis(1),
        });
        PreparedAudio {
            envelope: Arc::new(Envelope::from_samples(&[0.0], 1, ENVELOPE_BUCKETS)),
            pcm,
            identity,
        }
    }

    /// Blocks until `callers` `prepare()` calls have joined the in-flight
    /// entry for `identity`. The `Work` entry holds one receiver itself; every
    /// caller awaiting completion clones one more, so the joined count is
    /// `receiver_count() - 1`. Releasing the loader before this point lets the
    /// producer finish and drop the entry first, which turns the late caller
    /// into a second producer (#264).
    async fn wait_for_joined_callers(
        coordinator: &PreparationCoordinator,
        identity: &SourceIdentity,
        callers: usize,
    ) {
        timeout(Duration::from_secs(2), async {
            loop {
                let joined = coordinator
                    .entries
                    .lock()
                    .await
                    .get(identity)
                    .map_or(0, |work| work.sender.receiver_count().saturating_sub(1));
                if joined == callers {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("callers should join the in-flight preparation");
    }

    #[tokio::test]
    async fn public_prepare_shares_one_loader_and_replays_completion() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_loader = Arc::clone(&calls);
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Barrier::new(2));
        let release_for_loader = Arc::clone(&release);
        let coordinator = Arc::new(PreparationCoordinator::new_with_loader(
            move |_path, identity| {
                calls_for_loader.fetch_add(1, Ordering::SeqCst);
                started_sender
                    .send(())
                    .expect("test loader is still active");
                release_for_loader.wait();
                Ok(prepared(identity))
            },
        ));
        let path = PathBuf::from("controlled.wav");
        let first = {
            let coordinator = Arc::clone(&coordinator);
            let path = path.clone();
            tokio::spawn(async move { coordinator.prepare(&path).await })
        };
        tokio::task::spawn_blocking(move || {
            started_receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("producer should reach controlled barrier");
        })
        .await
        .expect("producer start task should join");
        let second = {
            let coordinator = Arc::clone(&coordinator);
            let path = path.clone();
            tokio::spawn(async move { coordinator.prepare(&path).await })
        };
        wait_for_joined_callers(&coordinator, &SourceIdentity::capture(&path), 2).await;
        let release_task = tokio::task::spawn_blocking(move || release.wait());
        let results = timeout(Duration::from_secs(2), async {
            let results = tokio::join!(first, second, release_task);
            (results.0, results.1)
        })
        .await
        .expect("shared preparation must not hang");
        assert!(results.0.unwrap().is_ok());
        assert!(results.1.unwrap().is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancelled_caller_does_not_strand_or_duplicate_producer() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_loader = Arc::clone(&calls);
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let release = Arc::new(std::sync::Barrier::new(2));
        let release_for_loader = Arc::clone(&release);
        let coordinator = Arc::new(PreparationCoordinator::new_with_loader(
            move |_path, identity| {
                calls_for_loader.fetch_add(1, Ordering::SeqCst);
                started_sender
                    .send(())
                    .expect("test loader is still active");
                release_for_loader.wait();
                Ok(prepared(identity))
            },
        ));
        let path = PathBuf::from("cancelled.wav");
        let first = {
            let coordinator = Arc::clone(&coordinator);
            let path = path.clone();
            tokio::spawn(async move { coordinator.prepare(&path).await })
        };
        tokio::task::spawn_blocking(move || {
            started_receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("producer should reach controlled barrier");
        })
        .await
        .expect("producer start task should join");
        first.abort();
        // Let the runtime drop the aborted caller (and its completion
        // receiver) before counting joined callers, so only the live second
        // caller can satisfy the wait below.
        assert!(first.await.is_err_and(|error| error.is_cancelled()));
        let second = {
            let coordinator = Arc::clone(&coordinator);
            let path = path.clone();
            tokio::spawn(async move { coordinator.prepare(&path).await })
        };
        wait_for_joined_callers(&coordinator, &SourceIdentity::capture(&path), 1).await;
        let release_task = tokio::task::spawn_blocking(move || release.wait());
        let result = timeout(Duration::from_secs(2), async {
            let (result, _) = tokio::join!(second, release_task);
            result
        })
        .await
        .expect("cancelled producer must still complete")
        .expect("subscriber task should join")
        .expect("loader result");
        assert_eq!(result.pcm.samples.len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
