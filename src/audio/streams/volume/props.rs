use super::{SourceLevel, VolumeError};
use pipewire::spa::{
    self,
    pod::{Object, Property, Value, ValueArray},
};

/// Raw server values, retained losslessly for restoration (including balance).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Props {
    pub volume: Option<f32>,
    pub channels: Option<Vec<f32>>,
    pub muted: Option<bool>,
}

impl Props {
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let (_, Value::Object(object)) =
            spa::pod::deserialize::PodDeserializer::deserialize_from::<Value>(bytes).ok()?
        else {
            return None;
        };
        if object.type_ != spa::utils::SpaTypes::ObjectParamProps.as_raw() {
            return None;
        }
        let mut props = Self::default();
        for property in object.properties {
            match (property.key, property.value) {
                (spa::sys::SPA_PROP_volume, Value::Float(v)) if valid_gain(v) => {
                    props.volume = Some(v)
                }
                (spa::sys::SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(v)))
                    if !v.is_empty()
                        && v.len() <= spa::param::audio::MAX_CHANNELS
                        && v.iter().copied().all(valid_gain) =>
                {
                    props.channels = Some(v)
                }
                (spa::sys::SPA_PROP_mute, Value::Bool(v)) => props.muted = Some(v),
                (
                    spa::sys::SPA_PROP_volume
                    | spa::sys::SPA_PROP_channelVolumes
                    | spa::sys::SPA_PROP_mute,
                    _,
                ) => return None,
                _ => {}
            }
        }
        (props != Self::default()).then_some(props)
    }

    pub fn encode(&self) -> Result<Vec<u8>, VolumeError> {
        let mut properties = Vec::new();
        if let Some(v) = self.volume {
            properties.push(Property::new(spa::sys::SPA_PROP_volume, Value::Float(v)));
        }
        if let Some(v) = &self.channels {
            properties.push(Property::new(
                spa::sys::SPA_PROP_channelVolumes,
                Value::ValueArray(ValueArray::Float(v.clone())),
            ));
        }
        if let Some(v) = self.muted {
            properties.push(Property::new(spa::sys::SPA_PROP_mute, Value::Bool(v)));
        }
        let value = Value::Object(Object {
            type_: spa::utils::SpaTypes::ObjectParamProps.as_raw(),
            id: spa::param::ParamType::Props.as_raw(),
            properties,
        });
        spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &value)
            .map(|result| result.0.into_inner())
            .map_err(|e| VolumeError::Serialize(Box::new(e)))
    }

    pub fn merge(&mut self, update: &Self) {
        if update.volume.is_some() {
            self.volume = update.volume;
        }
        if update.channels.is_some() {
            self.channels.clone_from(&update.channels);
        }
        if update.muted.is_some() {
            self.muted = update.muted;
        }
    }

    pub fn agrees_with(&self, other: &Self) -> bool {
        self.volume.is_none_or(|v| Some(v) == other.volume)
            && self
                .channels
                .as_ref()
                .is_none_or(|v| Some(v) == other.channels.as_ref())
            && self.muted.is_none_or(|v| Some(v) == other.muted)
    }

    pub fn effective_volume(&self) -> Option<f32> {
        let peak = self
            .channels
            .as_ref()
            .map(|v| v.iter().copied().fold(0.0, f32::max));
        let volume = match (self.volume, peak) {
            (Some(scalar), Some(peak)) => scalar * peak,
            (scalar, peak) => scalar.or(peak)?,
        };
        volume.is_finite().then_some(volume)
    }

    pub fn for_level(&self, level: SourceLevel) -> Result<Self, VolumeError> {
        if self.muted.is_none() || (self.volume.is_none() && self.channels.is_none()) {
            return Err(VolumeError::Unobserved);
        }
        let level = level.normalized();
        let mut target = self.clone();
        target.muted = Some(level.muted);
        if let Some(channels) = &mut target.channels {
            let peak = channels.iter().copied().fold(0.0, f32::max);
            for channel in channels {
                *channel = if peak > 0.0 {
                    (*channel / peak) * level.volume
                } else {
                    level.volume
                };
            }
            target.volume = self.volume.map(|_| 1.0);
        } else {
            target.volume = Some(level.volume);
        }
        Ok(target)
    }
}

fn valid_gain(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}
