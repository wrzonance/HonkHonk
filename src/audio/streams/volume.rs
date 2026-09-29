//! Node-wide Props control and conditional restoration on route teardown.
use super::{StreamEvent, StreamWatcher};
use pipewire::spa;
use serde::{Deserialize, Serialize};
mod control;
mod props;
pub(super) use control::NodeVolume;
use props::Props;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceLevel {
    pub volume: f32,
    pub muted: bool,
}

impl Default for SourceLevel {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
        }
    }
}

impl SourceLevel {
    pub fn normalized(mut self) -> Self {
        self.volume = if self.volume.is_finite() {
            self.volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VolumeError {
    #[error("source node is no longer available")]
    MissingNode,
    #[error("source volume/mute Props have not been observed yet")]
    Unobserved,
    #[error("could not serialize source volume Props: {0}")]
    Serialize(#[source] Box<dyn std::error::Error>),
    #[error("serialized volume Props are invalid")]
    InvalidPod,
}

impl StreamWatcher {
    pub(crate) fn restore_all_levels(&self) -> Vec<(u32, VolumeError)> {
        let nodes = self._tracked_nodes.borrow();
        restore_levels(
            nodes
                .iter()
                .map(|(id, tracked)| (*id, tracked.volume.as_ref())),
            |id, bytes| {
                let tracked = nodes.get(&id).ok_or(VolumeError::MissingNode)?;
                write_props(&tracked._node, bytes)
            },
        )
    }

    /// Caller must first verify the node still has an active, safe route.
    pub(crate) fn set_level(&self, id: u32, level: SourceLevel) -> Result<(), VolumeError> {
        let nodes = self._tracked_nodes.borrow();
        let tracked = nodes.get(&id).ok_or(VolumeError::MissingNode)?;
        let bytes = tracked.volume.borrow_mut().set_level(level)?;
        write_props(&tracked._node, &bytes)
    }

    pub(crate) fn restore_level(&self, id: u32) -> Result<(), VolumeError> {
        let nodes = self._tracked_nodes.borrow();
        let Some(tracked) = nodes.get(&id) else {
            return Ok(());
        };
        if let Some(bytes) = tracked.volume.borrow_mut().release()? {
            write_props(&tracked._node, &bytes)?;
        }
        Ok(())
    }
}

fn write_props(node: &pipewire::node::Node, bytes: &[u8]) -> Result<(), VolumeError> {
    let pod = spa::pod::Pod::from_bytes(bytes).ok_or(VolumeError::InvalidPod)?;
    node.set_param(spa::param::ParamType::Props, 0, pod);
    Ok(())
}

fn restore_levels<'a>(
    nodes: impl IntoIterator<Item = (u32, &'a std::cell::RefCell<NodeVolume>)>,
    mut write: impl FnMut(u32, &[u8]) -> Result<(), VolumeError>,
) -> Vec<(u32, VolumeError)> {
    nodes
        .into_iter()
        .filter_map(|(id, state)| {
            let result = state
                .borrow_mut()
                .release()
                .and_then(|bytes| bytes.map_or(Ok(()), |bytes| write(id, &bytes)));
            result.err().map(|error| (id, error))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn props_preserve_gain_when_muted_and_reject_malformed_observations() {
        let bytes = Props {
            volume: Some(0.4),
            muted: Some(true),
            channels: None,
        }
        .encode()
        .unwrap();
        assert_eq!(
            NodeVolume::default().observe(7, &bytes),
            Some(super::super::StreamEvent::SourceLevelChanged {
                id: 7,
                volume: Some(0.4),
                muted: Some(true),
            })
        );
        assert_eq!(NodeVolume::default().observe(7, &[0, 1]), None);
    }
    #[test]
    fn default_and_invalid_levels_are_bounded() {
        assert_eq!(SourceLevel::default().volume, 1.0);
        for (input, expected) in [
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (-1.0, 0.0),
            (2.0, 1.0),
        ] {
            let level = SourceLevel {
                volume: input,
                muted: true,
            }
            .normalized();
            assert_eq!(level.volume, expected);
            assert!(level.muted);
        }
    }
}

#[cfg(test)]
#[path = "volume/control_tests.rs"]
mod control_tests;
