use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash};

use boxcars::Attribute;

use super::ActorState;
use crate::attributes::CAR_COMPONENT_ACTIVE_ATTR;
use crate::metadata::ReplayGameMode;

pub(super) fn component_active_attr(actor: &ActorState) -> Option<bool> {
    bool_attr(&actor.attributes, CAR_COMPONENT_ACTIVE_ATTR)
}

pub(super) fn game_mode_attr<K, S>(
    attributes: &std::collections::HashMap<K, Attribute, S>,
    name: &str,
) -> Option<ReplayGameMode>
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    match attributes.get(name) {
        Some(Attribute::GameMode(major, minor)) => Some(match (*major, *minor) {
            (0, 0) => ReplayGameMode::Soccar,
            (1, 0) => ReplayGameMode::Hoops,
            (2, 0) => ReplayGameMode::Heatseeker,
            (3, 0) => ReplayGameMode::Snowday,
            (4, 0) => ReplayGameMode::Dropshot,
            (major, minor) => ReplayGameMode::Other(major, minor),
        }),
        _ => None,
    }
}

pub(super) fn int_attr<K, S>(
    attributes: &std::collections::HashMap<K, Attribute, S>,
    name: &str,
) -> Option<i32>
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    match attributes.get(name) {
        Some(Attribute::Int(value)) => Some(*value),
        Some(Attribute::Byte(value)) => Some(i32::from(*value)),
        _ => None,
    }
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn float_attr<K, S>(
    attributes: &std::collections::HashMap<K, Attribute, S>,
    name: &str,
) -> Option<f32>
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    match attributes.get(name) {
        Some(Attribute::Float(value)) => Some(*value),
        Some(Attribute::Int(value)) => Some(*value as f32),
        Some(Attribute::Byte(value) | Attribute::FlaggedByte(_, value)) => Some(f32::from(*value)),
        _ => None,
    }
}

pub(super) fn byte_attr<K, S>(
    attributes: &std::collections::HashMap<K, Attribute, S>,
    name: &str,
) -> Option<u8>
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    attributes.get(name).and_then(byte_attribute)
}

pub(super) fn byte_attribute(attribute: &Attribute) -> Option<u8> {
    match attribute {
        Attribute::Byte(value) => Some(*value),
        Attribute::Int(value) => u8::try_from(*value).ok(),
        _ => None,
    }
}

pub(super) fn bool_attr<K, S>(
    attributes: &std::collections::HashMap<K, Attribute, S>,
    name: &str,
) -> Option<bool>
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    match attributes.get(name) {
        Some(Attribute::Boolean(value)) => Some(*value),
        Some(Attribute::Byte(value)) => Some(*value != 0),
        Some(Attribute::FlaggedByte(flag, value)) => Some(*flag || *value != 0),
        _ => None,
    }
}

pub(super) fn vec3_attr<K, S>(
    attributes: &std::collections::HashMap<K, Attribute, S>,
    name: &str,
) -> Option<rocketsim::Vec3A>
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    match attributes.get(name) {
        Some(Attribute::Location(value)) => Some(rocketsim::Vec3A::new(value.x, value.y, value.z)),
        _ => None,
    }
}
