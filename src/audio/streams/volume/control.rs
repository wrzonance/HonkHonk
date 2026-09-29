use super::{Props, SourceLevel, StreamEvent, VolumeError};

#[derive(Default)]
pub(in crate::audio::streams) struct NodeVolume {
    observed: Props,
    lease: Option<Lease>,
}

struct Lease {
    original: Props,
    last_written: Props,
    pending: Vec<Props>,
    superseded: bool,
}

impl NodeVolume {
    pub fn observe(&mut self, id: u32, bytes: &[u8]) -> Option<StreamEvent> {
        let update = Props::decode(bytes)?;
        let previous = self.observed.clone();
        self.observed.merge(&update);
        if let Some(lease) = &mut self.lease {
            if let Some(index) = lease.pending.iter().position(|p| update.agrees_with(p)) {
                lease.pending.drain(..=index);
            } else if !update.agrees_with(&lease.last_written) && self.observed != previous {
                lease.superseded = true;
            }
        }
        Some(StreamEvent::SourceLevelChanged {
            id,
            volume: self.observed.effective_volume(),
            muted: self.observed.muted,
        })
    }

    pub fn set_level(&mut self, level: SourceLevel) -> Result<Vec<u8>, VolumeError> {
        let target = self.observed.for_level(level)?;
        let bytes = target.encode()?;
        if self.lease.as_ref().is_none_or(|lease| lease.superseded) {
            self.lease = Some(Lease {
                original: self.observed.clone(),
                last_written: target.clone(),
                pending: Vec::new(),
                superseded: false,
            });
        }
        if let Some(lease) = &mut self.lease {
            lease.last_written = target.clone();
            lease.pending.push(target);
        }
        Ok(bytes)
    }

    /// Own echoes may still be queued: restoration is sent after those writes.
    /// Any observed external override relinquishes ownership of the whole state.
    pub fn release(&mut self) -> Result<Option<Vec<u8>>, VolumeError> {
        let Some(lease) = self.lease.take() else {
            return Ok(None);
        };
        if lease.superseded {
            return Ok(None);
        }
        let bytes = lease.original.encode()?;
        self.observed = lease.original;
        Ok(Some(bytes))
    }
}
