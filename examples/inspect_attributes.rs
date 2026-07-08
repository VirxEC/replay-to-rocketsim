use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::{env, fs};

const RECOGNIZED_ATTRIBUTES: &[&str] = &[
    "TAGame.RBActor_TA:ReplicatedRBState",
    "TAGame.Vehicle_TA:ReplicatedThrottle",
    "TAGame.Vehicle_TA:ReplicatedSteer",
    "TAGame.Vehicle_TA:bReplicatedHandbrake",
    "TAGame.CarComponent_TA:Vehicle",
    "TAGame.CarComponent_TA:ReplicatedActive",
    "TAGame.CarComponent_Boost_TA:ReplicatedBoostAmount",
    "TAGame.CarComponent_Boost_TA:ReplicatedBoost",
    "Engine.Pawn:PlayerReplicationInfo",
    "Engine.PlayerReplicationInfo:Team",
    "TAGame.PRI_TA:ClientLoadout",
    "TAGame.PRI_TA:ClientLoadouts",
    "TAGame.PRI_TA:ClientLoadoutOnline",
    "TAGame.PRI_TA:ClientLoadoutsOnline",
    "TAGame.Car_TA:ReplicatedDemolish",
    "TAGame.Car_TA:ReplicatedDemolishExtended",
    "TAGame.Car_TA:ReplicatedDemolish_CustomFX",
    "TAGame.Car_TA:ReplicatedDemolishGoalExplosion",
    "TAGame.Vehicle_TA:InputRestriction",
    "TAGame.Vehicle_TA:bDriving",
    "TAGame.Car_TA:DodgesRefreshedCounter",
    "TAGame.Car_TA:bUnlimitedJumps",
    "TAGame.Car_TA:bUnlimitedTimeForDodge",
    "TAGame.CarComponent_Dodge_TA:DodgeTorque",
    "TAGame.CarComponent_Dodge_TA:DodgeImpulse",
    "TAGame.CarComponent_DoubleJump_TA:DoubleJumpImpulse",
    "TAGame.CarComponent_FlipCar_TA:bFlipRight",
    "TAGame.CarComponent_FlipCar_TA:FlipCarTime",
    "TAGame.VehiclePickup_TA:ReplicatedPickupData",
    "TAGame.VehiclePickup_TA:NewReplicatedPickupData",
    "TAGame.Ball_TA:HitTeamNum",
    "TAGame.Ball_TA:ReplicatedBallScale",
    "TAGame.Ball_TA:ReplicatedBallGravityScale",
    "TAGame.Ball_TA:ReplicatedBallMaxLinearSpeedScale",
    "TAGame.Ball_TA:ReplicatedWorldBounceScale",
    "TAGame.Ball_TA:ReplicatedAddedCarBounceScale",
    "TAGame.Ball_TA:BallHitSpinScale",
    "TAGame.Ball_TA:AirResistance",
    "TAGame.Ball_TA:bWarnBallReset",
    "TAGame.GameEvent_Soccar_TA:SecondsRemaining",
    "TAGame.GameEvent_TA:ReplicatedGameStateTimeRemaining",
    "TAGame.GameEvent_Soccar_TA:bOverTime",
    "TAGame.GameEvent_Soccar_TA:bBallHasBeenHit",
    "TAGame.GameEvent_TA:GameMode",
    "TAGame.GameEvent_TA:ReplicatedStateName",
    "TAGame.GameEvent_TA:ReplicatedStateIndex",
    "TAGame.GameEvent_Soccar_TA:ReplicatedScoredOnTeam",
    "TAGame.GameEvent_Soccar_TA:bMatchEnded",
    "ProjectX.GRI_X:ReplicatedGamePlaylist",
    "ProjectX.GRI_X:ReplicatedGameMutatorIndex",
    "Engine.TeamInfo:Score",
    "Engine.PlayerReplicationInfo:UniqueId",
    "Engine.PlayerReplicationInfo:PlayerName",
    "TAGame.PRI_TA:MatchScore",
    "TAGame.PRI_TA:MatchGoals",
    "TAGame.PRI_TA:MatchAssists",
    "TAGame.PRI_TA:MatchSaves",
    "TAGame.PRI_TA:MatchShots",
    "Engine.PlayerReplicationInfo:Ping",
    "Engine.PlayerReplicationInfo:PlayerID",
    "Engine.PlayerReplicationInfo:Score",
    "Engine.PlayerReplicationInfo:bBot",
    "Engine.PlayerReplicationInfo:bIsSpectator",
    "TAGame.PRI_TA:MatchDemolishes",
    "TAGame.PRI_TA:CarDemolitions",
    "TAGame.PRI_TA:SelfDemolitions",
    "TAGame.CarComponent_Boost_TA:BoostModifier",
    "TAGame.CarComponent_Boost_TA:BoostRestriction",
    "TAGame.CarComponent_Boost_TA:RechargeDelay",
    "TAGame.CarComponent_Boost_TA:RechargeRate",
    "TAGame.CarComponent_Boost_TA:UnlimitedBoostRefCount",
    "TAGame.CarComponent_Boost_TA:bNoBoost",
    "TAGame.CarComponent_Boost_TA:bUnlimitedBoost",
    "TAGame.CarComponent_Boost_TA:bRechargeGroundOnly",
];

const INTERESTING_KEYWORDS: &[&str] = &[
    "Pitch",
    "Yaw",
    "Roll",
    "Torque",
    "Input",
    "Throttle",
    "Steer",
    "Handbrake",
    "Jump",
    "Dodge",
    "DoubleJump",
    "Flip",
    "Boost",
    "Demolish",
    "Camera",
];

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let replay_path = args
        .next()
        .ok_or("usage: cargo run --example inspect_attributes -- <path-to-replay> [top-n]")?;
    let top_n = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(80);

    let replay_bytes = fs::read(replay_path)?;
    let replay = boxcars::ParserBuilder::new(&replay_bytes)
        .must_parse_network_data()
        .parse()?;
    let network_frames = replay
        .network_frames
        .as_ref()
        .ok_or("replay has no network frames")?;

    let recognized = RECOGNIZED_ATTRIBUTES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut attributes = BTreeMap::<String, AttributeStats>::new();
    let mut actor_archetypes = BTreeMap::<String, u64>::new();
    let mut unknown_object_ids = BTreeSet::new();

    for frame in &network_frames.frames {
        for new_actor in &frame.new_actors {
            if let Some(object_name) = object_name(&replay.objects, new_actor.object_id) {
                *actor_archetypes.entry(object_name.to_owned()).or_default() += 1;
            } else {
                unknown_object_ids.insert(usize::from(new_actor.object_id));
            }
        }

        for updated in &frame.updated_actors {
            let Some(attribute_name) = object_name(&replay.objects, updated.object_id) else {
                unknown_object_ids.insert(usize::from(updated.object_id));
                continue;
            };
            let stats = attributes.entry(attribute_name.to_owned()).or_default();
            stats.count += 1;
            stats.recognized = recognized.contains(attribute_name);
        }
    }

    println!(
        "frames={} unique_attributes={} unique_actor_archetypes={}",
        network_frames.frames.len(),
        attributes.len(),
        actor_archetypes.len()
    );
    if !unknown_object_ids.is_empty() {
        println!("unknown object ids referenced: {unknown_object_ids:?}");
    }

    println!("\nTop {top_n} observed attributes by update count:");
    for (attribute_name, stats) in top_attributes(&attributes, top_n) {
        println!(
            "{:>8}  {:<10}  {}",
            stats.count,
            if stats.recognized {
                "parsed"
            } else {
                "unparsed"
            },
            attribute_name
        );
    }

    println!("\nUnparsed attributes matching input/control keywords:");
    let mut printed_interesting = false;
    for (attribute_name, stats) in top_attributes(&attributes, attributes.len()) {
        if !stats.recognized && looks_input_related(attribute_name) {
            printed_interesting = true;
            println!("{:>8}  {}", stats.count, attribute_name);
        }
    }
    if !printed_interesting {
        println!("none observed");
    }

    println!("\nObserved attributes containing 'Pitch':");
    let mut printed_pitch = false;
    for (attribute_name, stats) in top_attributes(&attributes, attributes.len()) {
        if attribute_name.contains("Pitch") {
            printed_pitch = true;
            println!(
                "{:>8}  {:<10}  {}",
                stats.count,
                if stats.recognized {
                    "parsed"
                } else {
                    "unparsed"
                },
                attribute_name
            );
        }
    }
    if !printed_pitch {
        println!("none observed");
    }

    println!("\nTop {top_n} actor archetypes by spawn count:");
    for (object_name, count) in top_counts(&actor_archetypes, top_n) {
        println!("{count:>8}  {object_name}");
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
struct AttributeStats {
    count: u64,
    recognized: bool,
}

fn object_name(objects: &[String], object_id: boxcars::ObjectId) -> Option<&str> {
    objects.get(usize::from(object_id)).map(String::as_str)
}

fn looks_input_related(attribute_name: &str) -> bool {
    INTERESTING_KEYWORDS
        .iter()
        .any(|keyword| attribute_name.contains(keyword))
}

fn top_attributes(
    attributes: &BTreeMap<String, AttributeStats>,
    limit: usize,
) -> Vec<(&str, AttributeStats)> {
    let mut entries = attributes
        .iter()
        .map(|(name, stats)| (name.as_str(), *stats))
        .collect::<Vec<_>>();
    entries.sort_by(|(name_a, stats_a), (name_b, stats_b)| {
        stats_b
            .count
            .cmp(&stats_a.count)
            .then_with(|| name_a.cmp(name_b))
    });
    entries.truncate(limit);
    entries
}

fn top_counts(counts: &BTreeMap<String, u64>, limit: usize) -> Vec<(&str, u64)> {
    let mut entries = counts
        .iter()
        .map(|(name, count)| (name.as_str(), *count))
        .collect::<Vec<_>>();
    entries.sort_by(|(name_a, count_a), (name_b, count_b)| {
        count_b.cmp(count_a).then_with(|| name_a.cmp(name_b))
    });
    entries.truncate(limit);
    entries
}
