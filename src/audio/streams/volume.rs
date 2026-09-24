//! Node-wide Props control; no gain processing or extra audio links.
use super::{StreamEvent, StreamWatcher};
use pipewire::spa::{
    self,
    pod::{Object, Property, Value},
};
use serde::{Deserialize, Serialize};

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
    #[error("could not serialize source volume Props: {0}")]
    Serialize(#[source] Box<dyn std::error::Error>),
    #[error("serialized volume Props are invalid")]
    InvalidPod,
}

impl StreamWatcher {
    /// Caller must first verify the node still has an active, safe route.
    pub(crate) fn set_level(&self, id: u32, level: SourceLevel) -> Result<(), VolumeError> {
        let nodes = self._tracked_nodes.borrow();
        let tracked = nodes.get(&id).ok_or(VolumeError::MissingNode)?;
        let bytes = encode(level)?;
        let pod = spa::pod::Pod::from_bytes(&bytes).ok_or(VolumeError::InvalidPod)?;
        tracked
            ._node
            .set_param(spa::param::ParamType::Props, 0, pod);
        Ok(())
    }
}

fn encode(level: SourceLevel) -> Result<Vec<u8>, VolumeError> {
    let level = level.normalized();
    let value = Value::Object(Object {
        type_: spa::utils::SpaTypes::ObjectParamProps.as_raw(),
        id: spa::param::ParamType::Props.as_raw(),
        properties: vec![
            Property::new(spa::sys::SPA_PROP_volume, Value::Float(level.volume)),
            Property::new(spa::sys::SPA_PROP_mute, Value::Bool(level.muted)),
        ],
    });
    spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &value)
        .map(|result| result.0.into_inner())
        .map_err(|e| VolumeError::Serialize(Box::new(e)))
}

pub(super) fn observe(id: u32, bytes: &[u8]) -> Option<StreamEvent> {
    let (_, Value::Object(object)) =
        spa::pod::deserialize::PodDeserializer::deserialize_from::<Value>(bytes).ok()?
    else {
        return None;
    };
    if object.type_ != spa::utils::SpaTypes::ObjectParamProps.as_raw() {
        return None;
    }
    let mut volume = None;
    let mut muted = None;
    for property in object.properties {
        match (property.key, property.value) {
            (spa::sys::SPA_PROP_volume, Value::Float(v)) if v.is_finite() => volume = Some(v),
            (spa::sys::SPA_PROP_mute, Value::Bool(v)) => muted = Some(v),
            _ => {}
        }
    }
    (volume.is_some() || muted.is_some()).then_some(StreamEvent::SourceLevelChanged {
        id,
        volume,
        muted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn props_preserve_gain_when_muted_and_reject_malformed_observations() {
        let bytes = encode(SourceLevel {
            volume: 0.4,
            muted: true,
        })
        .unwrap();
        assert_eq!(
            observe(7, &bytes),
            Some(super::super::StreamEvent::SourceLevelChanged {
                id: 7,
                volume: Some(0.4),
                muted: Some(true),
            })
        );
        assert_eq!(observe(7, &[0, 1]), None);
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
