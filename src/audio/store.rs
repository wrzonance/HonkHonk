//! In-memory caches for the playback hot path: a byte-capped LRU of decoded
//! PCM. Pure — no audio I/O. Eviction never affects playback (the engine holds
//! its own `Arc`). See spec #151.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Default LRU cap: ~one long stereo song plus many short clips.
pub const DEFAULT_PCM_CAP_BYTES: usize = 256 * 1024 * 1024;

/// Decoded PCM plus the metadata the engine and playhead need. `samples` is
/// `Arc`-wrapped so cloning (into the engine `Play` command and the cache) is
/// O(1) and a single canonical buffer is shared.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedPcm {
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Duration,
}

impl CachedPcm {
    fn bytes(&self) -> usize {
        self.samples.len() * std::mem::size_of::<f32>()
    }
}

struct PcmEntry {
    pcm: Arc<CachedPcm>,
    last_used: u64,
}

pub struct AudioStore {
    pcm: HashMap<String, PcmEntry>,
    bytes: usize,
    cap_bytes: usize,
    tick: u64,
}

impl AudioStore {
    pub fn new(cap_bytes: usize) -> Self {
        Self {
            pcm: HashMap::new(),
            bytes: 0,
            cap_bytes,
            tick: 0,
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.wrapping_add(1);
        self.tick
    }

    pub fn get_pcm(&mut self, id: &str) -> Option<Arc<CachedPcm>> {
        let tick = self.next_tick();
        let entry = self.pcm.get_mut(id)?;
        entry.last_used = tick;
        Some(Arc::clone(&entry.pcm))
    }

    pub fn insert_pcm(&mut self, id: String, pcm: Arc<CachedPcm>) -> Vec<String> {
        let tick = self.next_tick();
        let new_bytes = pcm.bytes();
        if let Some(old) = self.pcm.insert(
            id,
            PcmEntry {
                pcm,
                last_used: tick,
            },
        ) {
            self.bytes -= old.pcm.bytes();
        }
        self.bytes += new_bytes;
        self.evict_to_cap()
    }

    /// Evicts least-recently-used entries until at or below the cap. A single
    /// entry larger than the cap is kept (evicting it would free nothing useful
    /// and stop a legitimately-requested sound from playing).
    ///
    /// Each victim is found with an O(n) `min_by_key` scan; eviction is a rare
    /// cold-path event (only on insert past the cap) and `n` is bounded by the
    /// cap, so this is left simple. If a burst ever makes O(n·k) eviction show
    /// up, switch to a `BTreeMap<tick, id>` LRU sidecar.
    fn evict_to_cap(&mut self) -> Vec<String> {
        let mut evicted = Vec::new();
        while self.bytes > self.cap_bytes && self.pcm.len() > 1 {
            let Some(victim) = self
                .pcm
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            if let Some(entry) = self.pcm.remove(&victim) {
                self.bytes -= entry.pcm.bytes();
            }
            evicted.push(victim);
        }
        evicted
    }

    pub fn pcm_bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm(n: usize) -> Arc<CachedPcm> {
        Arc::new(CachedPcm {
            samples: Arc::new(vec![0.0_f32; n]),
            sample_rate: 48_000,
            channels: 2,
            duration: Duration::from_secs(1),
        })
    }

    #[test]
    fn insert_then_get_returns_pcm() {
        let mut store = AudioStore::new(1024);
        assert!(store.insert_pcm("a".into(), pcm(4)).is_empty());
        assert_eq!(store.get_pcm("a"), Some(pcm(4)));
        assert_eq!(store.get_pcm("missing"), None);
    }

    #[test]
    fn insert_past_cap_evicts_least_recently_used() {
        // cap = 32 bytes = 8 f32. Each pcm(4) = 16 bytes. Two fit; a third evicts.
        let mut store = AudioStore::new(32);
        store.insert_pcm("a".into(), pcm(4));
        store.insert_pcm("b".into(), pcm(4));
        // Touch "a" so "b" is now least-recently-used.
        let _ = store.get_pcm("a");
        let evicted = store.insert_pcm("c".into(), pcm(4));
        assert_eq!(evicted, vec!["b".to_string()]);
        assert!(store.get_pcm("b").is_none());
        assert!(store.get_pcm("a").is_some());
        assert!(store.get_pcm("c").is_some());
        assert!(store.pcm_bytes() <= 32);
    }

    #[test]
    fn single_entry_larger_than_cap_is_kept() {
        let mut store = AudioStore::new(8);
        let evicted = store.insert_pcm("big".into(), pcm(100)); // 400 bytes > cap
        assert!(
            evicted.is_empty(),
            "a lone oversized entry must not evict itself"
        );
        assert!(store.get_pcm("big").is_some());
    }

    #[test]
    fn reinserting_same_id_replaces_without_double_counting() {
        let mut store = AudioStore::new(1024);
        store.insert_pcm("a".into(), pcm(4));
        store.insert_pcm("a".into(), pcm(8));
        assert_eq!(store.pcm_bytes(), 8 * std::mem::size_of::<f32>());
        assert_eq!(store.get_pcm("a").map(|p| p.samples.len()), Some(8));
    }
}
