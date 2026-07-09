use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash};

use boxcars::Attribute;
use rocketsim::CarControls;

use crate::attributes::{
    CAR_BOOST_AMOUNT_ATTR, CAR_COMPONENT_ACTIVE_ATTR, CAR_HANDBRAKE_ATTR,
    CAR_REPLICATED_BOOST_ATTR, CAR_STEER_ATTR, CAR_THROTTLE_ATTR,
};

pub(crate) fn car_controls<K, S>(
    attributes: &std::collections::HashMap<K, Attribute, S>,
    boost_component_attributes: Option<&std::collections::HashMap<K, Attribute, S>>,
    jump_component_attributes: Option<&std::collections::HashMap<K, Attribute, S>>,
) -> CarControls
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    CarControls {
        throttle: byte_axis(attributes.get(CAR_THROTTLE_ATTR)),
        steer: byte_axis(attributes.get(CAR_STEER_ATTR)),
        jump: jump_component_attributes.is_some_and(component_active),
        boost: boost_component_attributes.is_some_and(component_active),
        handbrake: byte_bool(attributes.get(CAR_HANDBRAKE_ATTR)),
        ..CarControls::default()
    }
}

/// Apply a small deadzone — zero out throttle/steer values below 0.01.
#[must_use]
pub(crate) fn apply_deadzone(mut controls: CarControls) -> CarControls {
    if controls.throttle.abs() < 0.01 {
        controls.throttle = 0.0;
    }
    if controls.steer.abs() < 0.01 {
        controls.steer = 0.0;
    }
    controls
}

pub(crate) fn replicated_boost_amount<K, S>(
    attributes: &std::collections::HashMap<K, Attribute, S>,
) -> Option<f32>
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    if let Some(Attribute::ReplicatedBoost(boost)) = attributes.get(CAR_REPLICATED_BOOST_ATTR) {
        return Some(f32::from(boost.boost_amount) / 2.55);
    }

    match attributes.get(CAR_BOOST_AMOUNT_ATTR) {
        Some(Attribute::Byte(value) | Attribute::FlaggedByte(_, value)) => {
            Some(f32::from(*value) / 2.55)
        }
        _ => None,
    }
}

fn byte_axis(attribute: Option<&Attribute>) -> f32 {
    match attribute {
        Some(Attribute::Byte(value) | Attribute::FlaggedByte(_, value)) => {
            (f32::from(*value) - 128.0) / 127.0
        }
        _ => 0.0,
    }
    .clamp(-1.0, 1.0)
}

fn component_active<K, S>(attributes: &std::collections::HashMap<K, Attribute, S>) -> bool
where
    K: Borrow<str> + Eq + Hash,
    S: BuildHasher,
{
    match attributes.get(CAR_COMPONENT_ACTIVE_ATTR) {
        Some(Attribute::Byte(value)) => value & 1 != 0,
        Some(Attribute::FlaggedByte(flag, value)) => *flag || value & 1 != 0,
        Some(Attribute::Boolean(value)) => *value,
        _ => false,
    }
}

fn byte_bool(attribute: Option<&Attribute>) -> bool {
    match attribute {
        Some(Attribute::Boolean(value)) => *value,
        Some(Attribute::Byte(value)) => *value != 0,
        Some(Attribute::FlaggedByte(flag, value)) => *flag || *value != 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use rustc_hash::FxHashMap;

    use super::*;

    #[test]
    fn converts_replicated_control_bytes_to_normalized_controls() {
        let mut attributes = FxHashMap::default();
        attributes.insert(CAR_THROTTLE_ATTR.to_owned(), Attribute::Byte(255));
        attributes.insert(CAR_STEER_ATTR.to_owned(), Attribute::Byte(0));
        attributes.insert(CAR_HANDBRAKE_ATTR.to_owned(), Attribute::Boolean(true));

        let controls = car_controls(&attributes, None, None);

        let throttle = controls.throttle;
        let steer = controls.steer;
        let pitch = controls.pitch;
        let yaw = controls.yaw;
        let handbrake = controls.handbrake;

        assert!((throttle - 1.0).abs() < f32::EPSILON);
        assert!((steer - -1.0).abs() < f32::EPSILON);
        assert!(pitch.abs() < f32::EPSILON);
        assert!(yaw.abs() < f32::EPSILON);
        assert!(handbrake);
    }

    #[test]
    fn converts_replicated_boost_component_state() {
        let mut component_attributes = FxHashMap::default();
        component_attributes.insert(CAR_COMPONENT_ACTIVE_ATTR.to_owned(), Attribute::Byte(1));
        component_attributes.insert(CAR_BOOST_AMOUNT_ATTR.to_owned(), Attribute::Byte(128));

        let controls = car_controls(&FxHashMap::default(), Some(&component_attributes), None);

        assert!(controls.boost);
        assert!(
            (replicated_boost_amount(&component_attributes).unwrap() - 50.196_08).abs() < 0.001
        );
    }

    #[test]
    fn converts_replicated_jump_component_state() {
        let mut component_attributes = FxHashMap::default();
        component_attributes.insert(CAR_COMPONENT_ACTIVE_ATTR.to_owned(), Attribute::Byte(1));

        let controls = car_controls(&FxHashMap::default(), None, Some(&component_attributes));

        assert!(controls.jump);
    }

    #[test]
    fn deadzone_zeros_near_zero_throttle_and_steer() {
        let controls = CarControls {
            throttle: 0.005,
            steer: -0.008,
            ..CarControls::default()
        };

        let controls = apply_deadzone(controls);

        let throttle = controls.throttle;
        let steer = controls.steer;
        assert!(throttle.abs() < f32::EPSILON);
        assert!(steer.abs() < f32::EPSILON);
    }

    #[test]
    fn deadzone_preserves_values_above_threshold() {
        let controls = CarControls {
            throttle: 0.015,
            steer: -0.5,
            ..CarControls::default()
        };

        let controls = apply_deadzone(controls);

        let throttle = controls.throttle;
        let steer = controls.steer;
        assert!((throttle - 0.015).abs() < f32::EPSILON);
        assert!((steer - -0.5).abs() < f32::EPSILON);
    }
}
