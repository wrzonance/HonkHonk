//! Shared preparation coordination for background and on-demand work.

use std::{collections::HashMap, path::Path, sync::Arc};

use tokio::sync::{Mutex, Notify};

use super::CachedPcm;
use super::processing::ProcessingError;
use crate::audio::{ENVELOPE_BUCKETS, Envelope};

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAudio {
    pub pcm: Arc<CachedPcm>,
    pub envelope: Arc<Envelope>,
    source_key: String,
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
            source_key: String::new(),
        })
    }

    pub fn matches_path(&self, path: &Path) -> bool {
        self.source_key == source_key(path)
    }

    pub fn has_source_identity(&self) -> bool {
        !self.source_key.is_empty()
    }
}

#[derive(Default)]
pub struct PreparationCoordinator {
    entries: Mutex<HashMap<String, Arc<Work>>>,
}

const MAX_COORDINATED_RESULTS: usize = 32;

struct Work {
    result: Mutex<Option<Result<Arc<PreparedAudio>, String>>>,
    notify: Notify,
}

enum Claim {
    Start(Arc<Work>),
    Wait(Arc<Work>),
    Ready(Result<Arc<PreparedAudio>, String>),
}

impl PreparationCoordinator {
    async fn claim(&self, key: String) -> Claim {
        let mut entries = self.entries.lock().await;
        if let Some(work) = entries.get(&key).cloned() {
            drop(entries);
            let result = work.result.lock().await.clone();
            return match result {
                Some(result) => Claim::Ready(result),
                None => Claim::Wait(work),
            };
        }
        let work = Arc::new(Work {
            result: Mutex::new(None),
            notify: Notify::new(),
        });
        entries.insert(key, Arc::clone(&work));
        Claim::Start(work)
    }

    async fn publish(
        &self,
        key: &str,
        work: &Arc<Work>,
        result: Result<Arc<PreparedAudio>, String>,
    ) {
        *work.result.lock().await = Some(result);
        work.notify.notify_waiters();
        let mut entries = self.entries.lock().await;
        if entries.len() > MAX_COORDINATED_RESULTS {
            entries.remove(key);
        }
    }

    pub async fn prepare(&self, path: &Path) -> Result<Arc<PreparedAudio>, String> {
        let key = source_key(path);
        loop {
            match self.claim(key.clone()).await {
                Claim::Ready(result) => return result,
                Claim::Wait(work) => {
                    work.notify.notified().await;
                    if let Some(result) = work.result.lock().await.clone() {
                        return result;
                    }
                }
                Claim::Start(work) => {
                    let path = path.to_owned();
                    let result = tokio::task::spawn_blocking(move || prepare_blocking(&path))
                        .await
                        .map_err(|error| error.to_string())
                        .and_then(|result| result);
                    self.publish(&key, &work, result.clone().map(Arc::new))
                        .await;
                    return result.map(Arc::new);
                }
            }
        }
    }
}

fn source_key(path: &Path) -> String {
    let metadata = std::fs::metadata(path)
        .ok()
        .map(|metadata| (metadata.len(), metadata.modified().ok()));
    format!("{}:{metadata:?}", path.display())
}

fn prepare_blocking(path: &Path) -> Result<PreparedAudio, String> {
    let pcm = Arc::new(super::processing::decode_cached(path).map_err(|error| error.to_string())?);
    let envelope = Arc::new(Envelope::from_samples(
        pcm.samples.as_ref(),
        pcm.channels,
        ENVELOPE_BUCKETS,
    ));
    Ok(PreparedAudio {
        pcm,
        envelope,
        source_key: source_key(path),
    })
}

/// Prepares one source file into the canonical cached PCM representation.
///
/// File I/O and decoding happen here; callers must run this function on a
/// blocking worker. Decoder repairs are represented in the returned analysis
/// metadata and cached PCM. Source files are never rewritten.
pub fn prepare(path: &Path) -> Result<CachedPcm, ProcessingError> {
    super::processing::decode_cached(path)
}

#[cfg(test)]
pub(crate) fn wrap_for_test(pcm: CachedPcm) -> Arc<PreparedAudio> {
    let envelope = Arc::new(Envelope::from_samples(
        pcm.samples.as_ref(),
        pcm.channels,
        ENVELOPE_BUCKETS,
    ));
    Arc::new(PreparedAudio {
        pcm: Arc::new(pcm),
        envelope,
        source_key: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn one_claim_waits_and_published_result_is_shared() {
        let coordinator = PreparationCoordinator::default();
        let key = "controlled".to_owned();
        let first = coordinator.claim(key.clone()).await;
        let second = coordinator.claim(key.clone()).await;
        assert!(matches!(first, Claim::Start(_)));
        assert!(matches!(second, Claim::Wait(_)));
        let (Claim::Start(work), Claim::Wait(wait)) = (first, second) else {
            unreachable!();
        };
        let pcm = Arc::new(CachedPcm {
            analysis: Default::default(),
            samples: Arc::new(vec![0.0]),
            sample_rate: 8_000,
            channels: 1,
            duration: std::time::Duration::from_millis(1),
        });
        let prepared = Arc::new(PreparedAudio {
            envelope: Arc::new(Envelope::from_samples(&[0.0], 1, ENVELOPE_BUCKETS)),
            pcm,
            source_key: "controlled".into(),
        });
        coordinator
            .publish(&key, &work, Ok(Arc::clone(&prepared)))
            .await;
        assert!(wait.result.lock().await.is_some());
        assert!(matches!(coordinator.claim(key).await, Claim::Ready(_)));
    }
}
