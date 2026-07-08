use std::collections::{BTreeMap, BTreeSet};
use std::{env, fs};

use boxcars::Attribute;
use replay_to_rocketsim::rocketsim::init_from_default;
use replay_to_rocketsim::{CarActionKind, CarLifecycleKind, Converter, FrameReplayEvent};

const CAR_COMPONENT_VEHICLE_ATTR: &str = "TAGame.CarComponent_TA:Vehicle";
const CAR_BOOST_AMOUNT_ATTR: &str = "TAGame.CarComponent_Boost_TA:ReplicatedBoostAmount";
const CAR_REPLICATED_BOOST_ATTR: &str = "TAGame.CarComponent_Boost_TA:ReplicatedBoost";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let replay_paths = env::args().skip(1).collect::<Vec<_>>();
    if replay_paths.is_empty() {
        return Err("usage: cargo run --example audit_car_state -- <replay> [<replay> ...]".into());
    }

    init_from_default(true)?;

    let mut total = Audit::default();
    for replay_path in replay_paths {
        let replay_bytes = fs::read(&replay_path)?;
        let raw = raw_attribute_audit(&replay_bytes)?;
        let output = Converter::new().convert_bytes(&replay_bytes)?;
        let audit = audit_output(&output, raw);
        print_audit(&replay_path, &audit);
        total.merge(&audit);
    }

    println!("\n=== aggregate ===");
    print_audit("all sampled replays", &total);
    Ok(())
}

#[derive(Debug, Default, Clone)]
struct Audit {
    replays: usize,
    frames: usize,
    car_frame_rows: usize,
    car_state_rows: usize,
    raw_pitch_attrs: BTreeMap<String, u64>,
    raw_yaw_attrs: BTreeMap<String, u64>,
    raw_roll_attrs: BTreeMap<String, u64>,
    raw_contact_attrs: BTreeMap<String, u64>,
    raw_supersonic_attrs: BTreeMap<String, u64>,
    raw_bump_attrs: BTreeMap<String, u64>,
    raw_respawn_attrs: BTreeMap<String, u64>,
    raw_unknown_body_product_ids: BTreeSet<u32>,
    car_metadata_without_pri: usize,
    car_metadata_without_body_product_id: usize,
    player_metadata_without_team: usize,
    nonzero_pitch_state: usize,
    nonzero_yaw_state: usize,
    nonzero_roll_state: usize,
    jump_control_true: usize,
    boost_control_true: usize,
    handbrake_control_true: usize,
    on_ground_false: usize,
    wheels_contact_any: usize,
    has_jumped: usize,
    has_double_jumped: usize,
    has_flipped: usize,
    is_jumping: usize,
    is_flipping: usize,
    flip_time_nonzero: usize,
    flip_torque_nonzero: usize,
    air_time_nonzero: usize,
    supersonic_true: usize,
    handbrake_val_nonzero: usize,
    bump_cooldown_nonzero: usize,
    world_contact_normal_some: usize,
    demoed_state_rows: usize,
    boost_config_metadata_rows: usize,
    boost_amount_state_non_spawn: usize,
    raw_replicated_boost_updates: usize,
    raw_replicated_boost_amount_updates: usize,
    raw_unlinked_boost_amount_updates: usize,
    boost_state_matches_latest_raw: usize,
    boost_state_mismatches_latest_raw: usize,
    boost_state_without_latest_raw: usize,
    boost_default_without_latest_raw: usize,
    max_boost_state_raw_abs_diff: f32,
    rb_repeated_rows: usize,
    rb_sleeping_true_rows: usize,
    rb_missing_vel_age: usize,
    jump_component_active_rows: usize,
    dodge_component_active_rows: usize,
    double_jump_component_active_rows: usize,
    flip_car_component_active_rows: usize,
    dodge_torque_rows: usize,
    dodge_impulse_rows: usize,
    double_jump_impulse_rows: usize,
    flip_car_time_rows: usize,
    car_action_events: BTreeMap<String, usize>,
    demo_events: usize,
    demo_events_by_kind: BTreeMap<String, usize>,
    demo_events_without_victim: usize,
    demo_events_self_demo_true: usize,
    lifecycle_events: usize,
    lifecycle_events_by_kind: BTreeMap<String, usize>,
    demo_started_events: usize,
    respawn_ended_events: usize,
    demo_started_without_same_frame_demo: usize,
    respawn_ended_without_prior_demo: usize,
    repeated_demo_started_without_respawn: usize,
    demoed_rows_after_respawn_ended_same_frame: usize,
}

impl Audit {
    fn merge(&mut self, other: &Self) {
        self.replays += other.replays;
        self.frames += other.frames;
        self.car_frame_rows += other.car_frame_rows;
        self.car_state_rows += other.car_state_rows;
        merge_counts(&mut self.raw_pitch_attrs, &other.raw_pitch_attrs);
        merge_counts(&mut self.raw_yaw_attrs, &other.raw_yaw_attrs);
        merge_counts(&mut self.raw_roll_attrs, &other.raw_roll_attrs);
        merge_counts(&mut self.raw_contact_attrs, &other.raw_contact_attrs);
        merge_counts(&mut self.raw_supersonic_attrs, &other.raw_supersonic_attrs);
        merge_counts(&mut self.raw_bump_attrs, &other.raw_bump_attrs);
        merge_counts(&mut self.raw_respawn_attrs, &other.raw_respawn_attrs);
        self.raw_unknown_body_product_ids
            .extend(other.raw_unknown_body_product_ids.iter().copied());
        self.car_metadata_without_pri += other.car_metadata_without_pri;
        self.car_metadata_without_body_product_id += other.car_metadata_without_body_product_id;
        self.player_metadata_without_team += other.player_metadata_without_team;
        self.nonzero_pitch_state += other.nonzero_pitch_state;
        self.nonzero_yaw_state += other.nonzero_yaw_state;
        self.nonzero_roll_state += other.nonzero_roll_state;
        self.jump_control_true += other.jump_control_true;
        self.boost_control_true += other.boost_control_true;
        self.handbrake_control_true += other.handbrake_control_true;
        self.on_ground_false += other.on_ground_false;
        self.wheels_contact_any += other.wheels_contact_any;
        self.has_jumped += other.has_jumped;
        self.has_double_jumped += other.has_double_jumped;
        self.has_flipped += other.has_flipped;
        self.is_jumping += other.is_jumping;
        self.is_flipping += other.is_flipping;
        self.flip_time_nonzero += other.flip_time_nonzero;
        self.flip_torque_nonzero += other.flip_torque_nonzero;
        self.air_time_nonzero += other.air_time_nonzero;
        self.supersonic_true += other.supersonic_true;
        self.handbrake_val_nonzero += other.handbrake_val_nonzero;
        self.bump_cooldown_nonzero += other.bump_cooldown_nonzero;
        self.world_contact_normal_some += other.world_contact_normal_some;
        self.demoed_state_rows += other.demoed_state_rows;
        self.boost_config_metadata_rows += other.boost_config_metadata_rows;
        self.boost_amount_state_non_spawn += other.boost_amount_state_non_spawn;
        self.raw_replicated_boost_updates += other.raw_replicated_boost_updates;
        self.raw_replicated_boost_amount_updates += other.raw_replicated_boost_amount_updates;
        self.raw_unlinked_boost_amount_updates += other.raw_unlinked_boost_amount_updates;
        self.boost_state_matches_latest_raw += other.boost_state_matches_latest_raw;
        self.boost_state_mismatches_latest_raw += other.boost_state_mismatches_latest_raw;
        self.boost_state_without_latest_raw += other.boost_state_without_latest_raw;
        self.boost_default_without_latest_raw += other.boost_default_without_latest_raw;
        self.max_boost_state_raw_abs_diff = self
            .max_boost_state_raw_abs_diff
            .max(other.max_boost_state_raw_abs_diff);
        self.rb_repeated_rows += other.rb_repeated_rows;
        self.rb_sleeping_true_rows += other.rb_sleeping_true_rows;
        self.rb_missing_vel_age += other.rb_missing_vel_age;
        self.jump_component_active_rows += other.jump_component_active_rows;
        self.dodge_component_active_rows += other.dodge_component_active_rows;
        self.double_jump_component_active_rows += other.double_jump_component_active_rows;
        self.flip_car_component_active_rows += other.flip_car_component_active_rows;
        self.dodge_torque_rows += other.dodge_torque_rows;
        self.dodge_impulse_rows += other.dodge_impulse_rows;
        self.double_jump_impulse_rows += other.double_jump_impulse_rows;
        self.flip_car_time_rows += other.flip_car_time_rows;
        merge_usize_counts(&mut self.car_action_events, &other.car_action_events);
        self.demo_events += other.demo_events;
        merge_usize_counts(&mut self.demo_events_by_kind, &other.demo_events_by_kind);
        self.demo_events_without_victim += other.demo_events_without_victim;
        self.demo_events_self_demo_true += other.demo_events_self_demo_true;
        self.lifecycle_events += other.lifecycle_events;
        merge_usize_counts(
            &mut self.lifecycle_events_by_kind,
            &other.lifecycle_events_by_kind,
        );
        self.demo_started_events += other.demo_started_events;
        self.respawn_ended_events += other.respawn_ended_events;
        self.demo_started_without_same_frame_demo += other.demo_started_without_same_frame_demo;
        self.respawn_ended_without_prior_demo += other.respawn_ended_without_prior_demo;
        self.repeated_demo_started_without_respawn += other.repeated_demo_started_without_respawn;
        self.demoed_rows_after_respawn_ended_same_frame +=
            other.demoed_rows_after_respawn_ended_same_frame;
    }
}

#[derive(Debug, Default)]
struct RawAudit {
    pitch_attrs: BTreeMap<String, u64>,
    yaw_attrs: BTreeMap<String, u64>,
    roll_attrs: BTreeMap<String, u64>,
    contact_attrs: BTreeMap<String, u64>,
    supersonic_attrs: BTreeMap<String, u64>,
    bump_attrs: BTreeMap<String, u64>,
    respawn_attrs: BTreeMap<String, u64>,
    unknown_body_product_ids: BTreeSet<u32>,
    replicated_boost_updates: usize,
    replicated_boost_amount_updates: usize,
    unlinked_boost_amount_updates: usize,
    latest_boost_by_frame: Vec<BTreeMap<i32, f32>>,
}

fn raw_attribute_audit(bytes: &[u8]) -> Result<RawAudit, Box<dyn std::error::Error>> {
    let replay = boxcars::ParserBuilder::new(bytes)
        .must_parse_network_data()
        .parse()?;
    let frames = replay
        .network_frames
        .as_ref()
        .ok_or("replay has no network frames")?;
    let mut audit = RawAudit {
        latest_boost_by_frame: Vec::with_capacity(frames.frames.len()),
        ..RawAudit::default()
    };
    let mut boost_component_to_car = BTreeMap::<i32, i32>::new();
    let mut latest_boost_by_car = BTreeMap::<i32, f32>::new();
    for frame in &frames.frames {
        for new_actor in &frame.new_actors {
            let actor_id = new_actor.actor_id.0;
            boost_component_to_car.remove(&actor_id);
            latest_boost_by_car.remove(&actor_id);
        }
        for deleted_actor in &frame.deleted_actors {
            if let Some(car_actor_id) = boost_component_to_car.remove(&deleted_actor.0) {
                latest_boost_by_car.remove(&car_actor_id);
            }
            latest_boost_by_car.remove(&deleted_actor.0);
        }
        for updated in &frame.updated_actors {
            let Some(name) = replay.objects.get(usize::from(updated.object_id)) else {
                continue;
            };
            bump_if_contains(&mut audit.pitch_attrs, name, "Pitch");
            bump_if_contains(&mut audit.yaw_attrs, name, "Yaw");
            bump_if_contains(&mut audit.roll_attrs, name, "Roll");
            bump_if_contains(&mut audit.contact_attrs, name, "Contact");
            bump_if_contains(&mut audit.supersonic_attrs, name, "Supersonic");
            bump_if_contains(&mut audit.bump_attrs, name, "Bump");
            bump_if_contains(&mut audit.respawn_attrs, name, "Respawn");

            if name == CAR_COMPONENT_VEHICLE_ATTR {
                if let Some(car_actor_id) = active_actor_id(&updated.attribute) {
                    boost_component_to_car.insert(updated.actor_id.0, car_actor_id);
                }
            } else if name == CAR_REPLICATED_BOOST_ATTR || name == CAR_BOOST_AMOUNT_ATTR {
                let boost_amount = replicated_boost_amount(&updated.attribute);
                if name == CAR_REPLICATED_BOOST_ATTR {
                    audit.replicated_boost_updates += 1;
                } else {
                    audit.replicated_boost_amount_updates += 1;
                }
                if let Some(boost_amount) = boost_amount {
                    if let Some(car_actor_id) = boost_component_to_car.get(&updated.actor_id.0) {
                        latest_boost_by_car.insert(*car_actor_id, boost_amount);
                    } else {
                        audit.unlinked_boost_amount_updates += 1;
                    }
                }
            }
        }
        audit
            .latest_boost_by_frame
            .push(latest_boost_by_car.clone());
    }
    Ok(audit)
}

fn audit_output(output: &replay_to_rocketsim::ConversionOutput, raw: RawAudit) -> Audit {
    let latest_boost_by_frame = raw.latest_boost_by_frame.clone();
    let mut audit = Audit {
        replays: 1,
        frames: output.frames.len(),
        raw_pitch_attrs: raw.pitch_attrs,
        raw_yaw_attrs: raw.yaw_attrs,
        raw_roll_attrs: raw.roll_attrs,
        raw_contact_attrs: raw.contact_attrs,
        raw_supersonic_attrs: raw.supersonic_attrs,
        raw_bump_attrs: raw.bump_attrs,
        raw_respawn_attrs: raw.respawn_attrs,
        raw_unknown_body_product_ids: raw.unknown_body_product_ids,
        raw_replicated_boost_updates: raw.replicated_boost_updates,
        raw_replicated_boost_amount_updates: raw.replicated_boost_amount_updates,
        raw_unlinked_boost_amount_updates: raw.unlinked_boost_amount_updates,
        player_metadata_without_team: output
            .players
            .iter()
            .filter(|player| player.team.is_none())
            .count(),
        ..Audit::default()
    };

    let mut cars_currently_demoed = BTreeSet::<i32>::new();
    for (frame_idx, cars) in output.cars.iter().enumerate() {
        let Some(state) = output.states.get(frame_idx) else {
            continue;
        };
        let latest_boost_by_car = latest_boost_by_frame
            .get(frame_idx)
            .cloned()
            .unwrap_or_default();
        let mut respawn_ended_this_frame = BTreeSet::<i32>::new();
        let mut demo_victims_this_frame = BTreeSet::<i32>::new();
        if let Some(metadata) = output.frame_metadata.get(frame_idx) {
            for event in &metadata.events {
                match event {
                    FrameReplayEvent::Demo(event) => {
                        if let Some(victim_car_actor_id) = event.victim_car_actor_id {
                            demo_victims_this_frame.insert(victim_car_actor_id);
                        }
                    }
                    FrameReplayEvent::CarLifecycle(event)
                        if event.kind == CarLifecycleKind::RespawnEnded =>
                    {
                        respawn_ended_this_frame.insert(event.car_actor_id);
                    }
                    _ => {}
                }
            }
        }
        for car in cars {
            audit.car_frame_rows += 1;
            if car.pri_actor_id.is_none() {
                audit.car_metadata_without_pri += 1;
            }
            if car.body_product_id.is_none() {
                audit.car_metadata_without_body_product_id += 1;
            }
            let rb = car.rigid_body;
            if rb.is_repeat {
                audit.rb_repeated_rows += 1;
            }
            if rb.is_sleeping == Some(true) {
                audit.rb_sleeping_true_rows += 1;
            }
            if rb.vel_update_age.is_none() {
                audit.rb_missing_vel_age += 1;
            }
            if car.jump_is_active == Some(true) {
                audit.jump_component_active_rows += 1;
            }
            if car.dodge_is_active == Some(true) {
                audit.dodge_component_active_rows += 1;
            }
            if car.double_jump_is_active == Some(true) {
                audit.double_jump_component_active_rows += 1;
            }
            if car.flip_car_is_active == Some(true) {
                audit.flip_car_component_active_rows += 1;
            }
            if car.dodge_torque.is_some() {
                audit.dodge_torque_rows += 1;
            }
            if car.dodge_impulse.is_some() {
                audit.dodge_impulse_rows += 1;
            }
            if car.double_jump_impulse.is_some() {
                audit.double_jump_impulse_rows += 1;
            }
            if car.flip_car_time.is_some() {
                audit.flip_car_time_rows += 1;
            }
            if car.no_boost.is_some()
                || car.unlimited_boost.is_some()
                || car.boost_modifier.is_some()
                || car.boost_recharge_rate.is_some()
                || car.boost_recharge_delay.is_some()
            {
                audit.boost_config_metadata_rows += 1;
            }

            let Some((_, car_state)) = state.cars.get(car.car_idx) else {
                continue;
            };
            audit.car_state_rows += 1;
            if car_state.controls.pitch.abs() > 0.001 {
                audit.nonzero_pitch_state += 1;
            }
            if car_state.controls.yaw.abs() > 0.001 {
                audit.nonzero_yaw_state += 1;
            }
            if car_state.controls.roll.abs() > 0.001 {
                audit.nonzero_roll_state += 1;
            }
            if car_state.controls.jump {
                audit.jump_control_true += 1;
            }
            if car_state.controls.boost {
                audit.boost_control_true += 1;
            }
            if car_state.controls.handbrake {
                audit.handbrake_control_true += 1;
            }
            if !car_state.is_on_ground {
                audit.on_ground_false += 1;
            }
            if car_state
                .wheels_with_contact
                .iter()
                .any(|has_contact| *has_contact)
            {
                audit.wheels_contact_any += 1;
            }
            if car_state.has_jumped {
                audit.has_jumped += 1;
            }
            if car_state.has_double_jumped {
                audit.has_double_jumped += 1;
            }
            if car_state.has_flipped {
                audit.has_flipped += 1;
            }
            if car_state.is_jumping {
                audit.is_jumping += 1;
            }
            if car_state.is_flipping {
                audit.is_flipping += 1;
            }
            if car_state.flip_time > 0.001 {
                audit.flip_time_nonzero += 1;
            }
            if car_state.flip_rel_torque.length_squared() > 0.001 {
                audit.flip_torque_nonzero += 1;
            }
            if car_state.air_time > 0.001 || car_state.air_time_since_jump > 0.001 {
                audit.air_time_nonzero += 1;
            }
            if car_state.is_supersonic {
                audit.supersonic_true += 1;
            }
            if car_state.handbrake_val.abs() > 0.001 {
                audit.handbrake_val_nonzero += 1;
            }
            if car_state.bump_cooldown_timer > 0.001 {
                audit.bump_cooldown_nonzero += 1;
            }
            if car_state.world_contact_normal.is_some() {
                audit.world_contact_normal_some += 1;
            }
            if car_state.is_demoed {
                audit.demoed_state_rows += 1;
            }
            if (car_state.boost - 33.333_332).abs() > 0.01 {
                audit.boost_amount_state_non_spawn += 1;
            }
            if let Some(raw_boost) = latest_boost_by_car.get(&car.car_actor_id) {
                let diff = (car_state.boost - raw_boost).abs();
                audit.max_boost_state_raw_abs_diff = audit.max_boost_state_raw_abs_diff.max(diff);
                if diff <= 0.05 {
                    audit.boost_state_matches_latest_raw += 1;
                } else {
                    audit.boost_state_mismatches_latest_raw += 1;
                }
            } else {
                audit.boost_state_without_latest_raw += 1;
                if (car_state.boost - 33.333_332).abs() <= 0.01 {
                    audit.boost_default_without_latest_raw += 1;
                }
            }
            if respawn_ended_this_frame.contains(&car.car_actor_id) && car_state.is_demoed {
                audit.demoed_rows_after_respawn_ended_same_frame += 1;
            }
        }

        if let Some(metadata) = output.frame_metadata.get(frame_idx) {
            for event in &metadata.events {
                match event {
                    FrameReplayEvent::CarAction(event) => {
                        let key = match event.action {
                            CarActionKind::Jump => "jump",
                            CarActionKind::Boost => "boost",
                            CarActionKind::Handbrake => "handbrake",
                            CarActionKind::Dodge => "dodge",
                            CarActionKind::DoubleJump => "double_jump",
                            CarActionKind::FlipCar => "flip_car",
                        };
                        *audit.car_action_events.entry(key.to_owned()).or_default() += 1;
                    }
                    FrameReplayEvent::Demo(event) => {
                        audit.demo_events += 1;
                        let key = match event.kind {
                            replay_to_rocketsim::DemoKind::Standard => "standard",
                            replay_to_rocketsim::DemoKind::Extended => "extended",
                            replay_to_rocketsim::DemoKind::CustomFx => "custom_fx",
                            replay_to_rocketsim::DemoKind::GoalExplosion => "goal_explosion",
                        };
                        *audit.demo_events_by_kind.entry(key.to_owned()).or_default() += 1;
                        if event.victim_car_actor_id.is_none() {
                            audit.demo_events_without_victim += 1;
                        }
                        if event.self_demo == Some(true) {
                            audit.demo_events_self_demo_true += 1;
                        }
                    }
                    FrameReplayEvent::CarLifecycle(event) => {
                        audit.lifecycle_events += 1;
                        let key = match event.kind {
                            CarLifecycleKind::DemoStarted => "demo_started",
                            CarLifecycleKind::RespawnEnded => "respawn_ended",
                            CarLifecycleKind::CarActorSpawned => "car_actor_spawned",
                            CarLifecycleKind::CarActorDeleted => "car_actor_deleted",
                        };
                        *audit
                            .lifecycle_events_by_kind
                            .entry(key.to_owned())
                            .or_default() += 1;
                        match event.kind {
                            CarLifecycleKind::DemoStarted => {
                                audit.demo_started_events += 1;
                                if !demo_victims_this_frame.contains(&event.car_actor_id) {
                                    audit.demo_started_without_same_frame_demo += 1;
                                }
                                if !cars_currently_demoed.insert(event.car_actor_id) {
                                    audit.repeated_demo_started_without_respawn += 1;
                                }
                            }
                            CarLifecycleKind::RespawnEnded => {
                                audit.respawn_ended_events += 1;
                                if !cars_currently_demoed.remove(&event.car_actor_id) {
                                    audit.respawn_ended_without_prior_demo += 1;
                                }
                            }
                            CarLifecycleKind::CarActorSpawned => {}
                            CarLifecycleKind::CarActorDeleted => {
                                cars_currently_demoed.remove(&event.car_actor_id);
                            }
                        }
                    }
                    FrameReplayEvent::BoostPickup(_) => {}
                }
            }
        }
    }
    audit
}

fn print_audit(label: &str, audit: &Audit) {
    println!("\n=== {label} ===");
    println!(
        "replays={} frames={} car_rows={} state_rows={}",
        audit.replays, audit.frames, audit.car_frame_rows, audit.car_state_rows
    );
    println!(
        "raw attrs: pitch={} yaw={} roll={} contact={} supersonic={} bump={} respawn={}",
        sum_counts(&audit.raw_pitch_attrs),
        sum_counts(&audit.raw_yaw_attrs),
        sum_counts(&audit.raw_roll_attrs),
        sum_counts(&audit.raw_contact_attrs),
        sum_counts(&audit.raw_supersonic_attrs),
        sum_counts(&audit.raw_bump_attrs),
        sum_counts(&audit.raw_respawn_attrs),
    );
    print_top("  pitch attrs", &audit.raw_pitch_attrs);
    print_top("  yaw attrs", &audit.raw_yaw_attrs);
    print_top("  roll attrs", &audit.raw_roll_attrs);
    print_top("  contact attrs", &audit.raw_contact_attrs);
    print_top("  supersonic attrs", &audit.raw_supersonic_attrs);
    print_top("  bump attrs", &audit.raw_bump_attrs);
    print_top("  respawn attrs", &audit.raw_respawn_attrs);
    println!(
        "fallback signals: car_rows_without_pri={} car_rows_without_body_product_id={} players_without_team={}",
        audit.car_metadata_without_pri,
        audit.car_metadata_without_body_product_id,
        audit.player_metadata_without_team
    );
    println!(
        "returned controls/state: pitch_nonzero={} yaw_nonzero={} roll_nonzero={} jump={} boost={} handbrake={}",
        audit.nonzero_pitch_state,
        audit.nonzero_yaw_state,
        audit.nonzero_roll_state,
        audit.jump_control_true,
        audit.boost_control_true,
        audit.handbrake_control_true
    );
    println!(
        "returned physics/internal: off_ground={} any_wheel_contact={} has_jumped={} has_double_jumped={} has_flipped={} is_jumping={} is_flipping={} flip_time={} flip_torque={} air_time={} supersonic={} handbrake_val={} bump_cooldown={} contact_normal={} demoed={}",
        audit.on_ground_false,
        audit.wheels_contact_any,
        audit.has_jumped,
        audit.has_double_jumped,
        audit.has_flipped,
        audit.is_jumping,
        audit.is_flipping,
        audit.flip_time_nonzero,
        audit.flip_torque_nonzero,
        audit.air_time_nonzero,
        audit.supersonic_true,
        audit.handbrake_val_nonzero,
        audit.bump_cooldown_nonzero,
        audit.world_contact_normal_some,
        audit.demoed_state_rows,
    );
    println!(
        "metadata/events: rb_repeat={} rb_sleeping={} rb_missing_vel={} jump_component={} dodge_component={} double_jump_component={} flip_car_component={} dodge_torque={} dodge_impulse={} double_jump_impulse={} flip_car_time={} demo_events={} demo_by_kind={:?} demo_without_victim={} self_demo_true={} lifecycle_events={} lifecycle_by_kind={:?} action_events={:?}",
        audit.rb_repeated_rows,
        audit.rb_sleeping_true_rows,
        audit.rb_missing_vel_age,
        audit.jump_component_active_rows,
        audit.dodge_component_active_rows,
        audit.double_jump_component_active_rows,
        audit.flip_car_component_active_rows,
        audit.dodge_torque_rows,
        audit.dodge_impulse_rows,
        audit.double_jump_impulse_rows,
        audit.flip_car_time_rows,
        audit.demo_events,
        audit.demo_events_by_kind,
        audit.demo_events_without_victim,
        audit.demo_events_self_demo_true,
        audit.lifecycle_events,
        audit.lifecycle_events_by_kind,
        audit.car_action_events,
    );
    println!(
        "lifecycle checks: demo_started={} respawn_ended={} demo_started_without_same_frame_demo={} respawn_ended_without_prior_demo={} repeated_demo_started_without_respawn={} demoed_rows_after_respawn_ended_same_frame={}",
        audit.demo_started_events,
        audit.respawn_ended_events,
        audit.demo_started_without_same_frame_demo,
        audit.respawn_ended_without_prior_demo,
        audit.repeated_demo_started_without_respawn,
        audit.demoed_rows_after_respawn_ended_same_frame,
    );
    println!(
        "boost: raw_replicated_boost_updates={} raw_replicated_boost_amount_updates={} raw_unlinked_boost_amount_updates={} config_metadata_rows={} state_non_spawn_amount_rows={} state_matches_latest_raw={} state_mismatches_latest_raw={} state_without_latest_raw={} default_without_latest_raw={} max_state_raw_abs_diff={:.3}",
        audit.raw_replicated_boost_updates,
        audit.raw_replicated_boost_amount_updates,
        audit.raw_unlinked_boost_amount_updates,
        audit.boost_config_metadata_rows,
        audit.boost_amount_state_non_spawn,
        audit.boost_state_matches_latest_raw,
        audit.boost_state_mismatches_latest_raw,
        audit.boost_state_without_latest_raw,
        audit.boost_default_without_latest_raw,
        audit.max_boost_state_raw_abs_diff,
    );
}

fn bump_if_contains(counts: &mut BTreeMap<String, u64>, name: &str, needle: &str) {
    if name.contains(needle) {
        *counts.entry(name.to_owned()).or_default() += 1;
    }
}

fn sum_counts(counts: &BTreeMap<String, u64>) -> u64 {
    counts.values().sum()
}

fn print_top(label: &str, counts: &BTreeMap<String, u64>) {
    if counts.is_empty() {
        return;
    }
    let mut entries = counts.iter().collect::<Vec<_>>();
    entries.sort_by(|(name_a, count_a), (name_b, count_b)| {
        count_b.cmp(count_a).then_with(|| name_a.cmp(name_b))
    });
    let rendered = entries
        .into_iter()
        .take(5)
        .map(|(name, count)| format!("{count}x {name}"))
        .collect::<Vec<_>>()
        .join("; ");
    println!("{label}: {rendered}");
}

fn active_actor_id(attribute: &Attribute) -> Option<i32> {
    match attribute {
        Attribute::ActiveActor(active_actor) if active_actor.active => Some(active_actor.actor.0),
        _ => None,
    }
}

fn replicated_boost_amount(attribute: &Attribute) -> Option<f32> {
    match attribute {
        Attribute::ReplicatedBoost(boost) => Some(f32::from(boost.boost_amount) / 2.55),
        Attribute::Byte(value) | Attribute::FlaggedByte(_, value) => Some(f32::from(*value) / 2.55),
        _ => None,
    }
}

fn merge_counts(target: &mut BTreeMap<String, u64>, source: &BTreeMap<String, u64>) {
    for (key, value) in source {
        *target.entry(key.clone()).or_default() += value;
    }
}

fn merge_usize_counts(target: &mut BTreeMap<String, usize>, source: &BTreeMap<String, usize>) {
    for (key, value) in source {
        *target.entry(key.clone()).or_default() += value;
    }
}
