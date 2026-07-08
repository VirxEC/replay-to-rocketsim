use boxcars::{ActorId, Attribute};

use crate::metadata::{BoostPickupKind, BoostPickupSource};

pub(super) fn demolish_event_ids(
    attribute: &Attribute,
) -> (Option<ActorId>, Option<ActorId>, Option<bool>) {
    match attribute {
        Attribute::Demolish(demolish) => (
            demolish
                .attacker_flag
                .then_some(demolish.attacker)
                .filter(|actor_id| valid_actor_id(*actor_id)),
            demolish
                .victim_flag
                .then_some(demolish.victim)
                .filter(|actor_id| valid_actor_id(*actor_id)),
            None,
        ),
        Attribute::DemolishExtended(demolish) => (
            demolish.attacker.active.then_some(demolish.attacker.actor),
            demolish.victim.active.then_some(demolish.victim.actor),
            Some(demolish.self_demolish),
        ),
        Attribute::DemolishFx(demolish) => (
            demolish
                .attacker_flag
                .then_some(demolish.attacker)
                .filter(|actor_id| valid_actor_id(*actor_id)),
            demolish
                .victim_flag
                .then_some(demolish.victim)
                .filter(|actor_id| valid_actor_id(*actor_id)),
            None,
        ),
        _ => (None, None, None),
    }
}

pub(super) fn demolish_victim_id(attribute: &Attribute) -> Option<ActorId> {
    demolish_event_ids(attribute).1
}

pub(super) fn valid_actor_id(actor_id: ActorId) -> bool {
    actor_id.0 >= 0
}

pub(super) fn pickup_event_kind_and_instigator(
    attribute: &Attribute,
) -> Option<(BoostPickupKind, Option<ActorId>, BoostPickupSource)> {
    match attribute {
        Attribute::Pickup(pickup) => Some((
            if pickup.picked_up {
                BoostPickupKind::PickedUp
            } else {
                BoostPickupKind::Released
            },
            pickup.instigator,
            BoostPickupSource::ReplicatedPickupData,
        )),
        Attribute::PickupNew(pickup) => Some((
            if pickup.picked_up != 0 {
                BoostPickupKind::PickedUp
            } else {
                BoostPickupKind::Released
            },
            pickup.instigator,
            BoostPickupSource::NewReplicatedPickupData,
        )),
        _ => None,
    }
}
