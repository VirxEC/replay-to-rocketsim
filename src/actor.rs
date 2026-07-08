use boxcars::{ActorId, Attribute, NewActor, ObjectId, UpdatedAttribute};
use rocketsim::{
    BallState, BoostPadConfig, BoostPadState, CarBodyConfig, CarControls, CarInfo, CarState, Team,
};
use rustc_hash::FxHashMap;

use crate::ConvertError;
use crate::attributes::*;
use crate::body::car_body_config_for_product_id;
use crate::controls::{car_controls, replicated_boost_amount};
use crate::metadata::*;
use crate::phys::{AccumulatedPhys, PhysFreshness};

mod attrs;
mod events;
mod metadata;

use attrs::*;
use events::*;
use metadata::*;

#[derive(Debug, Clone)]
pub(crate) struct ActorTracker<'a> {
    objects: &'a [String],
    replay_version: i32,
    actors: FxHashMap<ActorId, ActorState<'a>>,
    sorted_car_actor_ids: Vec<ActorId>,
    car_actor_indices: FxHashMap<ActorId, usize>,
    car_component_actor_ids: FxHashMap<ComponentKey, ActorId>,
    boost_pad_cooldowns: FxHashMap<ActorId, f32>,
    boost_pad_indices: FxHashMap<ActorId, usize>,
    previous_car_controls: FxHashMap<ActorId, CarControls>,
    previous_car_action_states: FxHashMap<ActorId, CarActionState>,
    previous_demo_respawn_timers: FxHashMap<ActorId, f32>,
    boost_timing: FxHashMap<ActorId, BoostTiming>,
    current_ball_hit_team_num: Option<u8>,
    current_replay_frame: usize,
    current_replay_time: f32,
    current_replay_delta: f32,
    active_replay_frame: Option<usize>,
    current_events: Vec<FrameReplayEvent>,
    persistent_players: FxHashMap<String, PlayerMetadata>,
    persistent_teams: FxHashMap<ActorId, TeamMetadata>,
}

impl<'a> ActorTracker<'a> {
    pub(crate) fn new(objects: &'a [String], replay_version: i32) -> Self {
        Self {
            objects,
            replay_version,
            actors: FxHashMap::default(),
            sorted_car_actor_ids: Vec::new(),
            car_actor_indices: FxHashMap::default(),
            car_component_actor_ids: FxHashMap::default(),
            boost_pad_cooldowns: FxHashMap::default(),
            boost_pad_indices: FxHashMap::default(),
            previous_car_controls: FxHashMap::default(),
            previous_car_action_states: FxHashMap::default(),
            previous_demo_respawn_timers: FxHashMap::default(),
            boost_timing: FxHashMap::default(),
            current_ball_hit_team_num: None,
            current_replay_frame: 0,
            current_replay_time: 0.0,
            current_replay_delta: 0.0,
            active_replay_frame: None,
            current_events: Vec::new(),
            persistent_players: FxHashMap::default(),
            persistent_teams: FxHashMap::default(),
        }
    }

    pub(crate) fn begin_frame(&mut self, replay_delta: f32, replay_frame: usize, replay_time: f32) {
        self.current_ball_hit_team_num = None;
        self.current_replay_frame = replay_frame;
        self.current_replay_time = replay_time;
        self.current_replay_delta = replay_delta;
        self.active_replay_frame = Some(replay_frame);
        self.current_events.clear();
        for actor in self.actors.values_mut() {
            actor.demo_respawn_timer = (actor.demo_respawn_timer - replay_delta).max(0.0);
        }
        for cooldown in self.boost_pad_cooldowns.values_mut() {
            *cooldown = (*cooldown - replay_delta).max(0.0);
        }
    }

    pub(crate) fn apply_deleted_actors(&mut self, deleted_actors: &[ActorId]) {
        for actor_id in deleted_actors {
            if let Some(actor) = self.actors.get(actor_id)
                && actor.kind == ActorKind::Car
            {
                self.current_events
                    .push(FrameReplayEvent::CarLifecycle(self.car_lifecycle_event(
                        *actor_id,
                        CarLifecycleKind::CarActorDeleted,
                        None,
                    )));
            }
            self.actors.remove(actor_id);
            self.refresh_indices();
            self.boost_pad_cooldowns.remove(actor_id);
            self.boost_pad_indices.remove(actor_id);
            self.previous_car_controls.remove(actor_id);
            self.previous_car_action_states.remove(actor_id);
            self.previous_demo_respawn_timers.remove(actor_id);
            self.boost_timing.remove(actor_id);
        }
    }

    pub(crate) fn apply_new_actors(
        &mut self,
        new_actors: &[NewActor],
        replay_frame: usize,
        replay_time: f32,
    ) {
        let mut spawned_car_actor_ids = Vec::new();
        for new_actor in new_actors {
            let kind = self.actor_kind(new_actor.object_id);
            self.actors.insert(
                new_actor.actor_id,
                ActorState {
                    new_actor: *new_actor,
                    kind,
                    attributes: FxHashMap::default(),
                    phys: AccumulatedPhys::from_spawn(new_actor, replay_frame, replay_time),
                    demo_respawn_timer: 0.0,
                },
            );
            if kind == ActorKind::Car {
                spawned_car_actor_ids.push(new_actor.actor_id);
            }
        }
        if !spawned_car_actor_ids.is_empty() {
            self.refresh_indices();
            self.current_replay_frame = replay_frame;
            self.current_replay_time = replay_time;
            for actor_id in spawned_car_actor_ids {
                self.current_events
                    .push(FrameReplayEvent::CarLifecycle(self.car_lifecycle_event(
                        actor_id,
                        CarLifecycleKind::CarActorSpawned,
                        Some(0.0),
                    )));
            }
        }
    }

    pub(crate) fn apply_updated_actors(
        &mut self,
        updated_actors: &[UpdatedAttribute],
        replay_delta: f32,
        replay_frame: usize,
        replay_time: f32,
    ) -> Result<(), ConvertError> {
        if self.active_replay_frame != Some(replay_frame) {
            self.begin_frame(replay_delta, replay_frame, replay_time);
        }

        let mut demo_victims = Vec::new();
        let mut frame_events = std::mem::take(&mut self.current_events);
        for updated in updated_actors {
            let attr_name = self.object_name(updated.object_id)?;
            let demo_event = self.demo_event_for_attribute(attr_name, &updated.attribute);
            let pickup_event = if attr_name == PICKUP_ATTR || attr_name == PICKUP_NEW_ATTR {
                self.pickup_event_for_attribute(updated.actor_id, &updated.attribute, None)
            } else {
                None
            };
            let actor = self
                .actors
                .get_mut(&updated.actor_id)
                .ok_or(ConvertError::MissingActor(updated.actor_id))?;
            if attr_name == RB_STATE_ATTR {
                actor.phys.apply_rigid_body(
                    updated.actor_id,
                    &updated.attribute,
                    self.replay_version,
                    replay_frame,
                    replay_time,
                )?;
            } else if let Some(demo_event) = demo_event {
                if let Some(victim_id) = demolish_victim_id(&updated.attribute) {
                    demo_victims.push(victim_id);
                }
                frame_events.push(FrameReplayEvent::Demo(demo_event));
            } else if attr_name == BALL_HIT_TEAM_NUM_ATTR {
                self.current_ball_hit_team_num = byte_attribute(&updated.attribute);
            } else if attr_name == PICKUP_ATTR || attr_name == PICKUP_NEW_ATTR {
                match pickup_event.as_ref().map(|event| event.kind) {
                    Some(BoostPickupKind::PickedUp) => {
                        self.boost_pad_cooldowns
                            .insert(updated.actor_id, actor.boost_pad_cooldown());
                    }
                    Some(BoostPickupKind::Released) => {
                        self.boost_pad_cooldowns.remove(&updated.actor_id);
                    }
                    None => {}
                }
                if let Some(pickup_event) = pickup_event {
                    frame_events.push(FrameReplayEvent::BoostPickup(pickup_event));
                }
            }
            actor
                .attributes
                .insert(attr_name, updated.attribute.clone());
        }

        for victim_id in demo_victims {
            if let Some(victim) = self.actors.get_mut(&victim_id) {
                victim.demo_respawn_timer = rocketsim::consts::car::spawn::RESPAWN_TIME;
            }
        }
        self.current_events = frame_events;
        self.emit_demo_lifecycle_edges();
        self.refresh_persistent_metadata();
        Ok(())
    }

    pub(crate) fn refresh_indices(&mut self) {
        self.sorted_car_actor_ids.clear();
        self.car_actor_indices.clear();
        self.car_component_actor_ids.clear();

        for actor in self.actors.values() {
            match actor.kind {
                ActorKind::Car => self.sorted_car_actor_ids.push(actor.new_actor.actor_id),
                ActorKind::BoostComponent
                | ActorKind::JumpComponent
                | ActorKind::DodgeComponent
                | ActorKind::DoubleJumpComponent
                | ActorKind::FlipCarComponent => {
                    if let Some(car_actor_id) =
                        active_actor_id(actor.attributes.get(CAR_COMPONENT_VEHICLE_ATTR))
                    {
                        self.car_component_actor_ids.insert(
                            ComponentKey {
                                car_actor_id,
                                kind: actor.kind,
                            },
                            actor.new_actor.actor_id,
                        );
                    }
                }
                _ => {}
            }
        }
        self.sorted_car_actor_ids.sort_by_key(|actor_id| actor_id.0);
        for (idx, actor_id) in self.sorted_car_actor_ids.iter().copied().enumerate() {
            self.car_actor_indices.insert(actor_id, idx);
        }
    }

    pub(crate) fn ball_state(&self) -> Result<Option<ReplayBallState>, ConvertError> {
        let Some(ball_actor) = self
            .actors
            .values()
            .filter(|actor| actor.kind == ActorKind::Ball)
            .min_by_key(|actor| actor.new_actor.actor_id.0)
        else {
            return Ok(None);
        };

        Ok(Some(ReplayBallState {
            state: BallState {
                phys: ball_actor
                    .phys
                    .to_phys_state(ball_actor.new_actor.actor_id)?,
                ..BallState::default()
            },
            phys_freshness: ball_actor.phys.freshness(),
            metadata: ball_metadata(
                ball_actor,
                self.current_ball_hit_team_num,
                self.current_replay_frame,
                self.current_replay_time,
            ),
        }))
    }

    pub(crate) fn car_states(
        &mut self,
        car_body: CarBodyConfig,
    ) -> Result<Vec<ReplayCarState>, ConvertError> {
        let car_actor_ids = self.sorted_car_actor_ids.clone();
        let mut cars = Vec::with_capacity(car_actor_ids.len());
        let mut current_controls = Vec::with_capacity(car_actor_ids.len());
        let mut current_boost_timing = Vec::with_capacity(car_actor_ids.len());
        for (idx, actor_id) in car_actor_ids.into_iter().enumerate() {
            let actor = self
                .actors
                .get(&actor_id)
                .ok_or(ConvertError::MissingActor(actor_id))?;
            let boost_component = self.boost_component_for_car(actor_id);
            let jump_component = self.car_component_for_car(actor_id, ActorKind::JumpComponent);
            let controls = car_controls(
                &actor.attributes,
                boost_component.map(|actor| &actor.attributes),
                jump_component.map(|actor| &actor.attributes),
            );
            let prev_controls = self
                .previous_car_controls
                .get(&actor_id)
                .copied()
                .unwrap_or(controls);
            let phys = actor.phys.to_phys_state(actor_id)?;
            let mut car = CarState {
                phys,
                controls,
                prev_controls,
                ..CarState::default()
            };
            if let Some(boost_component) = boost_component {
                if bool_attr(&boost_component.attributes, BOOST_NO_BOOST_ATTR) == Some(true) {
                    car.boost = 0.0;
                    car.controls.boost = false;
                    car.prev_controls.boost = false;
                } else if let Some(boost_amount) =
                    replicated_boost_amount(&boost_component.attributes)
                {
                    car.boost = boost_amount;
                }
            }
            let dodge_component = self.car_component_for_car(actor_id, ActorKind::DodgeComponent);
            let flip_car_component =
                self.car_component_for_car(actor_id, ActorKind::FlipCarComponent);
            if let Some(jump_active) = jump_component.and_then(component_active_attr) {
                car.is_jumping = jump_active;
            }
            if let Some(is_flipping) = dodge_component
                .and_then(component_active_attr)
                .or_else(|| flip_car_component.and_then(component_active_attr))
            {
                car.is_flipping = is_flipping;
            }
            if let Some(flip_time) = flip_car_component
                .and_then(|flip_car| float_attr(&flip_car.attributes, FLIP_CAR_TIME_ATTR))
            {
                car.flip_time = flip_time;
            }
            if let Some(dodge_torque) =
                dodge_component.and_then(|dodge| vec3_attr(&dodge.attributes, DODGE_TORQUE_ATTR))
            {
                car.flip_rel_torque = dodge_torque;
            }
            let boost_timing = self.boost_timing(actor_id, car.controls.boost);
            car.is_boosting = car.controls.boost;
            car.boosting_time = boost_timing.boosting_time;
            car.time_since_boosted = boost_timing.time_since_boosted;

            if actor.demo_respawn_timer > 0.0 {
                car.is_demoed = true;
                car.demo_respawn_timer = actor.demo_respawn_timer;
            }

            let team = self
                .team_for_car(actor)
                .unwrap_or_else(|| infer_team(actor, idx));
            let config = self.car_body_for_car(actor, team, car_body);
            cars.push(ReplayCarState {
                info: CarInfo { idx, team, config },
                state: car,
                phys_freshness: actor.phys.freshness(),
            });
            let action_state = CarActionState {
                jump: car.controls.jump,
                boost: car.controls.boost,
                handbrake: car.controls.handbrake,
                dodge: dodge_component
                    .and_then(component_active_attr)
                    .unwrap_or(false),
                double_jump: self
                    .car_component_for_car(actor_id, ActorKind::DoubleJumpComponent)
                    .and_then(component_active_attr)
                    .unwrap_or(false),
                flip_car: flip_car_component
                    .and_then(component_active_attr)
                    .unwrap_or(false),
            };
            self.emit_car_action_edges(actor_id, action_state);
            current_controls.push((actor_id, car.controls));
            current_boost_timing.push((actor_id, boost_timing));
        }
        for (actor_id, controls) in current_controls {
            self.previous_car_controls.insert(actor_id, controls);
        }
        for (actor_id, timing) in current_boost_timing {
            self.boost_timing.insert(actor_id, timing);
        }
        Ok(cars)
    }

    pub(crate) fn boost_pad_states(
        &mut self,
        boost_pad_configs: &[BoostPadConfig],
    ) -> Vec<(usize, BoostPadState)> {
        let mut indexed_pickups = Vec::new();
        let mut events = std::mem::take(&mut self.current_events);
        for event in &mut events {
            let FrameReplayEvent::BoostPickup(event) = event else {
                continue;
            };
            let actor_id = ActorId(event.boost_pad_actor_id);
            if event.boost_pad_index.is_none() {
                let resolved = self
                    .actors
                    .get(&actor_id)
                    .and_then(|actor| actor.phys.pos())
                    .and_then(|pos| nearest_boost_pad(boost_pad_configs, pos))
                    .or_else(|| {
                        event
                            .instigator_car_actor_id
                            .and_then(|actor_id| self.actors.get(&ActorId(actor_id)))
                            .and_then(|car| car.phys.pos())
                            .and_then(|pos| nearest_boost_pad(boost_pad_configs, pos))
                    });
                if let Some((idx, distance)) = resolved {
                    event.boost_pad_index = Some(idx);
                    event.nearest_boost_pad_distance = Some(distance);
                }
            }
            if let Some(idx) = event.boost_pad_index {
                if let Some(config) = boost_pad_configs.get(idx) {
                    event.boost_pad_pos = Some(config.pos);
                    event.boost_pad_is_big = Some(config.is_big);
                }
                if event.instigator_car_idx.is_none()
                    && let Some(instigator_id) = event.instigator_car_actor_id.map(ActorId)
                {
                    event.instigator_car_idx = self.car_idx_for_actor_id(instigator_id);
                    event.instigator_player_name =
                        self.pri_for_car_id(instigator_id).and_then(player_name);
                    event.instigator_team = self
                        .actors
                        .get(&instigator_id)
                        .and_then(|car| self.team_for_car(car));
                    event.instigator_boost_amount = self
                        .boost_component_for_car(instigator_id)
                        .and_then(|boost| replicated_boost_amount(&boost.attributes));
                }
                indexed_pickups.push((actor_id, idx));
            }
        }
        self.current_events = events;
        for (actor_id, idx) in indexed_pickups {
            self.boost_pad_indices.insert(actor_id, idx);
        }

        self.boost_pad_cooldowns
            .iter()
            .filter_map(|(actor_id, cooldown)| {
                if *cooldown <= 0.0 {
                    return None;
                }
                let idx = self
                    .actors
                    .get(actor_id)
                    .and_then(|actor| actor.phys.pos())
                    .and_then(|pos| nearest_boost_pad(boost_pad_configs, pos).map(|(idx, _)| idx))
                    .or_else(|| self.boost_pad_indices.get(actor_id).copied())?;
                Some((
                    idx,
                    BoostPadState {
                        cooldown: *cooldown,
                    },
                ))
            })
            .collect()
    }

    pub(crate) fn frame_metadata(&self) -> ReplayFrameMetadata {
        ReplayFrameMetadata {
            game_event: self.frame_game_event(),
            ball: self.frame_ball_metadata(),
            score: self.frame_score(),
            mutators: self.frame_mutators(),
            events: self.current_events.clone(),
        }
    }

    pub(crate) fn car_metadata(&self, fallback_body: CarBodyConfig) -> Vec<FrameCarMetadata> {
        self.sorted_car_actor_ids
            .iter()
            .filter_map(|actor_id| self.actors.get(actor_id))
            .enumerate()
            .map(|(car_idx, car)| {
                let pri = self.pri_for_car(car);
                let boost_component = self.boost_component_for_car(car.new_actor.actor_id);
                let jump_component =
                    self.car_component_for_car(car.new_actor.actor_id, ActorKind::JumpComponent);
                let dodge_component =
                    self.car_component_for_car(car.new_actor.actor_id, ActorKind::DodgeComponent);
                let double_jump_component = self
                    .car_component_for_car(car.new_actor.actor_id, ActorKind::DoubleJumpComponent);
                let flip_car_component =
                    self.car_component_for_car(car.new_actor.actor_id, ActorKind::FlipCarComponent);
                let team = self
                    .team_for_car(car)
                    .unwrap_or_else(|| infer_team(car, car_idx));
                let body_product_id = pri.and_then(|pri| body_product_id_for_pri(pri, team));
                FrameCarMetadata {
                    car_idx,
                    car_actor_id: car.new_actor.actor_id.0,
                    pri_actor_id: pri.map(|pri| pri.new_actor.actor_id.0),
                    unique_id: pri.and_then(player_unique_id),
                    player_name: pri.and_then(player_name),
                    team,
                    body_product_id,
                    body_config: body_product_id
                        .and_then(car_body_config_for_product_id)
                        .unwrap_or(fallback_body),
                    rigid_body: rigid_body_metadata(
                        car,
                        self.current_replay_frame,
                        self.current_replay_time,
                        car.demo_respawn_timer > 0.0,
                    ),
                    input_restriction: byte_attr(&car.attributes, CAR_INPUT_RESTRICTION_ATTR),
                    is_driving: bool_attr(&car.attributes, CAR_IS_DRIVING_ATTR),
                    is_demoed: car.demo_respawn_timer > 0.0,
                    demo_respawn_timer: car.demo_respawn_timer,
                    jump_is_active: jump_component.and_then(component_active_attr),
                    dodge_is_active: dodge_component.and_then(component_active_attr),
                    dodge_torque: dodge_component
                        .and_then(|dodge| vec3_attr(&dodge.attributes, DODGE_TORQUE_ATTR)),
                    dodge_impulse: dodge_component
                        .and_then(|dodge| vec3_attr(&dodge.attributes, DODGE_IMPULSE_ATTR)),
                    double_jump_is_active: double_jump_component.and_then(component_active_attr),
                    double_jump_impulse: double_jump_component.and_then(|double_jump| {
                        vec3_attr(&double_jump.attributes, DOUBLE_JUMP_IMPULSE_ATTR)
                    }),
                    flip_car_is_active: flip_car_component.and_then(component_active_attr),
                    flip_car_time: flip_car_component
                        .and_then(|flip_car| float_attr(&flip_car.attributes, FLIP_CAR_TIME_ATTR)),
                    flip_car_right: flip_car_component
                        .and_then(|flip_car| bool_attr(&flip_car.attributes, FLIP_CAR_RIGHT_ATTR)),
                    dodges_refreshed_counter: int_attr(
                        &car.attributes,
                        CAR_DODGES_REFRESHED_COUNTER_ATTR,
                    ),
                    unlimited_jumps: bool_attr(&car.attributes, CAR_UNLIMITED_JUMPS_ATTR),
                    unlimited_time_for_dodge: bool_attr(
                        &car.attributes,
                        CAR_UNLIMITED_TIME_FOR_DODGE_ATTR,
                    ),
                    boost_modifier: boost_component
                        .and_then(|boost| float_attr(&boost.attributes, BOOST_MODIFIER_ATTR)),
                    boost_restriction: boost_component
                        .and_then(|boost| byte_attr(&boost.attributes, BOOST_RESTRICTION_ATTR)),
                    boost_recharge_delay: boost_component
                        .and_then(|boost| float_attr(&boost.attributes, BOOST_RECHARGE_DELAY_ATTR)),
                    boost_recharge_rate: boost_component
                        .and_then(|boost| float_attr(&boost.attributes, BOOST_RECHARGE_RATE_ATTR)),
                    boost_unlimited_ref_count: boost_component.and_then(|boost| {
                        int_attr(&boost.attributes, BOOST_UNLIMITED_REF_COUNT_ATTR)
                    }),
                    no_boost: boost_component
                        .and_then(|boost| bool_attr(&boost.attributes, BOOST_NO_BOOST_ATTR)),
                    unlimited_boost: boost_component
                        .and_then(|boost| bool_attr(&boost.attributes, BOOST_UNLIMITED_ATTR)),
                    recharge_ground_only: boost_component.and_then(|boost| {
                        bool_attr(&boost.attributes, BOOST_RECHARGE_GROUND_ONLY_ATTR)
                    }),
                }
            })
            .collect()
    }

    pub(crate) fn player_metadata(&self) -> Vec<PlayerMetadata> {
        let mut players = self
            .persistent_players
            .values()
            .cloned()
            .collect::<Vec<_>>();
        players.sort_by_key(|player| player.pri_actor_id);
        players
    }

    pub(crate) fn team_metadata(&self) -> Vec<TeamMetadata> {
        let mut teams = self.persistent_teams.values().copied().collect::<Vec<_>>();
        teams.sort_by_key(|team| team.actor_id);
        teams
    }

    fn refresh_persistent_metadata(&mut self) {
        let player_updates = self
            .actors
            .values()
            .filter(|actor| actor.kind == ActorKind::PlayerReplicationInfo)
            .map(|pri| {
                let metadata = PlayerMetadata {
                    pri_actor_id: pri.new_actor.actor_id.0,
                    unique_id: player_unique_id(pri),
                    name: player_name(pri),
                    team: self.team_for_pri(pri),
                    player_id: int_attr(&pri.attributes, PRI_PLAYER_ID_ATTR),
                    score: int_attr(&pri.attributes, PRI_SCORE_ATTR),
                    is_bot: bool_attr(&pri.attributes, PRI_IS_BOT_ATTR),
                    is_spectator: bool_attr(&pri.attributes, PRI_IS_SPECTATOR_ATTR),
                    match_score: int_attr(&pri.attributes, PRI_MATCH_SCORE_ATTR),
                    match_goals: int_attr(&pri.attributes, PRI_MATCH_GOALS_ATTR),
                    match_assists: int_attr(&pri.attributes, PRI_MATCH_ASSISTS_ATTR),
                    match_saves: int_attr(&pri.attributes, PRI_MATCH_SAVES_ATTR),
                    match_shots: int_attr(&pri.attributes, PRI_MATCH_SHOTS_ATTR),
                    match_demolishes: int_attr(&pri.attributes, PRI_MATCH_DEMOLISHES_ATTR),
                    car_demolitions: int_attr(&pri.attributes, PRI_CAR_DEMOLITIONS_ATTR),
                    self_demolitions: int_attr(&pri.attributes, PRI_SELF_DEMOLITIONS_ATTR),
                    ping: byte_attr(&pri.attributes, PRI_PING_ATTR),
                };
                (player_metadata_key(pri, &metadata), metadata)
            })
            .collect::<Vec<_>>();
        for (key, metadata) in player_updates {
            self.persistent_players
                .entry(key)
                .and_modify(|existing| merge_player_metadata(existing, &metadata))
                .or_insert(metadata);
        }

        let team_updates = self
            .actors
            .values()
            .filter_map(|actor| match actor.kind {
                ActorKind::BlueTeam => Some((
                    actor.new_actor.actor_id,
                    TeamMetadata {
                        actor_id: actor.new_actor.actor_id.0,
                        team: Team::Blue,
                        score: int_attr(&actor.attributes, TEAM_SCORE_ATTR),
                    },
                )),
                ActorKind::OrangeTeam => Some((
                    actor.new_actor.actor_id,
                    TeamMetadata {
                        actor_id: actor.new_actor.actor_id.0,
                        team: Team::Orange,
                        score: int_attr(&actor.attributes, TEAM_SCORE_ATTR),
                    },
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (actor_id, metadata) in team_updates {
            self.persistent_teams.insert(actor_id, metadata);
        }
    }

    fn frame_game_event(&self) -> FrameGameEvent {
        self.actors
            .values()
            .filter(|actor| actor.kind == ActorKind::GameEvent)
            .min_by_key(|actor| actor.new_actor.actor_id.0)
            .map_or_else(FrameGameEvent::default, |actor| FrameGameEvent {
                seconds_remaining: int_attr(&actor.attributes, GAME_SECONDS_REMAINING_ATTR),
                replicated_game_state_time_remaining: int_attr(
                    &actor.attributes,
                    GAME_STATE_TIME_REMAINING_ATTR,
                ),
                is_overtime: bool_attr(&actor.attributes, GAME_IS_OVERTIME_ATTR),
                ball_has_been_hit: bool_attr(&actor.attributes, GAME_BALL_HAS_BEEN_HIT_ATTR),
                game_mode: game_mode_attr(&actor.attributes, GAME_MODE_ATTR),
                replicated_state_name: int_attr(&actor.attributes, GAME_STATE_NAME_ATTR),
                replicated_state_index: byte_attr(&actor.attributes, GAME_STATE_INDEX_ATTR),
                scored_on_team: byte_attr(&actor.attributes, GAME_SCORED_ON_TEAM_ATTR),
                match_ended: bool_attr(&actor.attributes, GAME_MATCH_ENDED_ATTR),
                playlist: self.game_replication_int(GRI_PLAYLIST_ATTR),
                mutator_index: self.game_replication_int(GRI_MUTATOR_INDEX_ATTR),
            })
    }

    fn frame_ball_metadata(&self) -> FrameBallMetadata {
        self.actors
            .values()
            .filter(|actor| actor.kind == ActorKind::Ball)
            .min_by_key(|actor| actor.new_actor.actor_id.0)
            .map_or_else(FrameBallMetadata::default, |actor| {
                ball_metadata(
                    actor,
                    self.current_ball_hit_team_num,
                    self.current_replay_frame,
                    self.current_replay_time,
                )
            })
    }

    fn frame_score(&self) -> FrameScore {
        self.actors
            .values()
            .fold(FrameScore::default(), |mut score, actor| {
                match actor.kind {
                    ActorKind::BlueTeam => {
                        score.blue = int_attr(&actor.attributes, TEAM_SCORE_ATTR);
                    }
                    ActorKind::OrangeTeam => {
                        score.orange = int_attr(&actor.attributes, TEAM_SCORE_ATTR);
                    }
                    _ => {}
                }
                score
            })
    }

    fn frame_mutators(&self) -> ReplayMutatorMetadata {
        let ball = self
            .actors
            .values()
            .filter(|actor| actor.kind == ActorKind::Ball)
            .min_by_key(|actor| actor.new_actor.actor_id.0);
        let boost = self
            .actors
            .values()
            .filter(|actor| actor.kind == ActorKind::BoostComponent)
            .min_by_key(|actor| actor.new_actor.actor_id.0);
        ReplayMutatorMetadata {
            ball_scale: ball.and_then(|actor| float_attr(&actor.attributes, BALL_SCALE_ATTR)),
            ball_gravity_scale: ball
                .and_then(|actor| float_attr(&actor.attributes, BALL_GRAVITY_SCALE_ATTR)),
            ball_max_linear_speed_scale: ball
                .and_then(|actor| float_attr(&actor.attributes, BALL_MAX_SPEED_SCALE_ATTR)),
            boost_recharge_delay: boost
                .and_then(|actor| float_attr(&actor.attributes, BOOST_RECHARGE_DELAY_ATTR)),
            boost_recharge_rate: boost
                .and_then(|actor| float_attr(&actor.attributes, BOOST_RECHARGE_RATE_ATTR)),
            unlimited_boost: boost
                .and_then(|actor| bool_attr(&actor.attributes, BOOST_UNLIMITED_ATTR)),
            no_boost: boost.and_then(|actor| bool_attr(&actor.attributes, BOOST_NO_BOOST_ATTR)),
        }
    }

    fn game_replication_int(&self, attr_name: &str) -> Option<i32> {
        self.actors
            .values()
            .filter(|actor| actor.kind == ActorKind::GameReplicationInfo)
            .min_by_key(|actor| actor.new_actor.actor_id.0)
            .and_then(|actor| int_attr(&actor.attributes, attr_name))
    }

    fn demo_event_for_attribute(
        &self,
        attr_name: &str,
        attribute: &Attribute,
    ) -> Option<FrameDemoEvent> {
        let kind = match attr_name {
            CAR_DEMOLISH_ATTR => DemoKind::Standard,
            CAR_DEMOLISH_EXTENDED_ATTR => DemoKind::Extended,
            CAR_DEMOLISH_CUSTOM_FX_ATTR => DemoKind::CustomFx,
            CAR_DEMOLISH_GOAL_EXPLOSION_ATTR => DemoKind::GoalExplosion,
            _ => return None,
        };
        let (attacker, victim, self_demo) = demolish_event_ids(attribute);
        Some(FrameDemoEvent {
            frame: self.current_replay_frame,
            time: self.current_replay_time,
            kind,
            attacker_car_actor_id: attacker.map(|id| id.0),
            attacker_pri_actor_id: attacker.and_then(|id| self.pri_actor_id_for_car_id(id)),
            attacker_unique_id: attacker.and_then(|id| self.unique_id_for_car_id(id)),
            victim_car_actor_id: victim.map(|id| id.0),
            victim_pri_actor_id: victim.and_then(|id| self.pri_actor_id_for_car_id(id)),
            victim_unique_id: victim.and_then(|id| self.unique_id_for_car_id(id)),
            self_demo,
        })
    }

    fn pickup_event_for_attribute(
        &self,
        boost_pad_actor_id: ActorId,
        attribute: &Attribute,
        boost_pad_index: Option<usize>,
    ) -> Option<FrameBoostPickupEvent> {
        let (kind, instigator, source) = pickup_event_kind_and_instigator(attribute)?;
        Some(FrameBoostPickupEvent {
            frame: self.current_replay_frame,
            time: self.current_replay_time,
            boost_pad_actor_id: boost_pad_actor_id.0,
            boost_pad_index,
            boost_pad_is_big: None,
            boost_pad_pos: None,
            nearest_boost_pad_distance: None,
            instigator_car_actor_id: instigator.map(|id| id.0),
            instigator_car_idx: instigator.and_then(|id| self.car_idx_for_actor_id(id)),
            instigator_pri_actor_id: instigator.and_then(|id| self.pri_actor_id_for_car_id(id)),
            instigator_unique_id: instigator.and_then(|id| self.unique_id_for_car_id(id)),
            instigator_player_name: instigator
                .and_then(|id| self.pri_for_car_id(id))
                .and_then(player_name),
            instigator_team: instigator
                .and_then(|id| self.actors.get(&id))
                .and_then(|car| self.team_for_car(car)),
            instigator_boost_amount: instigator
                .and_then(|id| self.boost_component_for_car(id))
                .and_then(|boost| replicated_boost_amount(&boost.attributes)),
            kind,
            source,
        })
    }

    fn boost_component_for_car(&self, car_actor_id: ActorId) -> Option<&ActorState<'_>> {
        self.car_component_for_car(car_actor_id, ActorKind::BoostComponent)
    }

    fn car_component_for_car(
        &self,
        car_actor_id: ActorId,
        kind: ActorKind,
    ) -> Option<&ActorState<'_>> {
        self.car_component_actor_ids
            .get(&ComponentKey { car_actor_id, kind })
            .and_then(|component_actor_id| self.actors.get(component_actor_id))
    }

    fn pri_for_car(&self, car: &ActorState<'_>) -> Option<&ActorState<'_>> {
        active_actor_id(car.attributes.get(CAR_PRI_ATTR))
            .and_then(|actor_id| self.actors.get(&actor_id))
    }

    fn pri_for_car_id(&self, car_actor_id: ActorId) -> Option<&ActorState<'_>> {
        self.actors
            .get(&car_actor_id)
            .and_then(|car| self.pri_for_car(car))
    }

    fn pri_actor_id_for_car_id(&self, car_actor_id: ActorId) -> Option<i32> {
        self.pri_for_car_id(car_actor_id)
            .map(|pri| pri.new_actor.actor_id.0)
    }

    fn unique_id_for_car_id(&self, car_actor_id: ActorId) -> Option<PlayerUniqueId> {
        self.pri_for_car_id(car_actor_id).and_then(player_unique_id)
    }

    fn car_idx_for_actor_id(&self, car_actor_id: ActorId) -> Option<usize> {
        self.car_actor_indices.get(&car_actor_id).copied()
    }

    fn car_action_event(
        &self,
        car_actor_id: ActorId,
        action: CarActionKind,
        previous: bool,
        current: bool,
    ) -> FrameCarActionEvent {
        let car = self.actors.get(&car_actor_id);
        let pri = car.and_then(|car| self.pri_for_car(car));
        FrameCarActionEvent {
            frame: self.current_replay_frame,
            time: self.current_replay_time,
            car_actor_id: car_actor_id.0,
            car_idx: self.car_idx_for_actor_id(car_actor_id),
            pri_actor_id: pri.map(|pri| pri.new_actor.actor_id.0),
            unique_id: pri.and_then(player_unique_id),
            player_name: pri.and_then(player_name),
            team: car.and_then(|car| self.team_for_car(car)),
            action,
            previous,
            current,
        }
    }

    fn emit_car_action_edges(&mut self, car_actor_id: ActorId, current: CarActionState) {
        let previous = self
            .previous_car_action_states
            .get(&car_actor_id)
            .copied()
            .unwrap_or(current);
        for (action, previous_value, current_value) in [
            (CarActionKind::Jump, previous.jump, current.jump),
            (CarActionKind::Boost, previous.boost, current.boost),
            (
                CarActionKind::Handbrake,
                previous.handbrake,
                current.handbrake,
            ),
            (CarActionKind::Dodge, previous.dodge, current.dodge),
            (
                CarActionKind::DoubleJump,
                previous.double_jump,
                current.double_jump,
            ),
            (CarActionKind::FlipCar, previous.flip_car, current.flip_car),
        ] {
            if previous_value != current_value {
                self.current_events
                    .push(FrameReplayEvent::CarAction(self.car_action_event(
                        car_actor_id,
                        action,
                        previous_value,
                        current_value,
                    )));
            }
        }
        self.previous_car_action_states
            .insert(car_actor_id, current);
    }

    fn car_lifecycle_event(
        &self,
        car_actor_id: ActorId,
        kind: CarLifecycleKind,
        demo_respawn_timer: Option<f32>,
    ) -> FrameCarLifecycleEvent {
        let car = self.actors.get(&car_actor_id);
        let pri = car.and_then(|car| self.pri_for_car(car));
        FrameCarLifecycleEvent {
            frame: self.current_replay_frame,
            time: self.current_replay_time,
            car_actor_id: car_actor_id.0,
            car_idx: self.car_idx_for_actor_id(car_actor_id),
            pri_actor_id: pri.map(|pri| pri.new_actor.actor_id.0),
            unique_id: pri.and_then(player_unique_id),
            player_name: pri.and_then(player_name),
            team: car.and_then(|car| self.team_for_car(car)),
            kind,
            demo_respawn_timer,
        }
    }

    fn emit_demo_lifecycle_edges(&mut self) {
        let mut current_timers = Vec::new();
        let mut events = Vec::new();
        for actor in self
            .actors
            .values()
            .filter(|actor| actor.kind == ActorKind::Car)
        {
            let actor_id = actor.new_actor.actor_id;
            let previous = self
                .previous_demo_respawn_timers
                .get(&actor_id)
                .copied()
                .unwrap_or(0.0);
            let current = actor.demo_respawn_timer;
            if previous <= 0.0 && current > 0.0 {
                events.push(FrameReplayEvent::CarLifecycle(self.car_lifecycle_event(
                    actor_id,
                    CarLifecycleKind::DemoStarted,
                    Some(current),
                )));
            } else if previous > 0.0 && current <= 0.0 {
                events.push(FrameReplayEvent::CarLifecycle(self.car_lifecycle_event(
                    actor_id,
                    CarLifecycleKind::RespawnEnded,
                    Some(0.0),
                )));
            }
            current_timers.push((actor_id, current));
        }
        self.current_events.extend(events);
        for (actor_id, timer) in current_timers {
            self.previous_demo_respawn_timers.insert(actor_id, timer);
        }
    }

    fn team_for_car(&self, car: &ActorState) -> Option<Team> {
        self.pri_for_car(car).and_then(|pri| self.team_for_pri(pri))
    }

    fn team_for_pri(&self, pri: &ActorState) -> Option<Team> {
        let team_actor_id = active_actor_id(pri.attributes.get(PRI_TEAM_ATTR))?;
        let team_actor = self.actors.get(&team_actor_id)?;
        match team_actor.kind {
            ActorKind::BlueTeam => Some(Team::Blue),
            ActorKind::OrangeTeam => Some(Team::Orange),
            _ => None,
        }
    }

    fn boost_timing(&self, car_actor_id: ActorId, is_boosting: bool) -> BoostTiming {
        let previous = self
            .boost_timing
            .get(&car_actor_id)
            .copied()
            .unwrap_or_default();
        if is_boosting {
            BoostTiming {
                boosting_time: previous.boosting_time + self.current_replay_delta.max(0.0),
                time_since_boosted: 0.0,
            }
        } else {
            BoostTiming {
                boosting_time: 0.0,
                time_since_boosted: previous.time_since_boosted
                    + self.current_replay_delta.max(0.0),
            }
        }
    }

    fn car_body_for_car(
        &self,
        car: &ActorState,
        team: Team,
        fallback: CarBodyConfig,
    ) -> CarBodyConfig {
        self.pri_for_car(car)
            .and_then(|pri| body_product_id_for_pri(pri, team))
            .and_then(car_body_config_for_product_id)
            .unwrap_or(fallback)
    }

    fn actor_kind(&self, object_id: ObjectId) -> ActorKind {
        self.objects
            .get(usize::from(object_id))
            .map_or(ActorKind::Other, |object_name| {
                if object_name.starts_with("Archetypes.Ball.") {
                    ActorKind::Ball
                } else if object_name == "Archetypes.Car.Car_Default"
                    || object_name.starts_with("Archetypes.Car.Car_")
                {
                    ActorKind::Car
                } else if object_name == "Archetypes.CarComponents.CarComponent_Boost"
                    || object_name.ends_with(":CarArchetype.Boost")
                {
                    ActorKind::BoostComponent
                } else if object_name == "Archetypes.CarComponents.CarComponent_Jump"
                    || object_name.ends_with(":CarArchetype.Jump")
                {
                    ActorKind::JumpComponent
                } else if object_name == "Archetypes.CarComponents.CarComponent_Dodge"
                    || object_name.ends_with(":CarArchetype.Dodge")
                {
                    ActorKind::DodgeComponent
                } else if object_name == "Archetypes.CarComponents.CarComponent_DoubleJump"
                    || object_name.ends_with(":CarArchetype.DoubleJump")
                {
                    ActorKind::DoubleJumpComponent
                } else if object_name == "Archetypes.CarComponents.CarComponent_FlipCar"
                    || object_name.ends_with(":CarArchetype.FlipCar")
                {
                    ActorKind::FlipCarComponent
                } else if object_name == "Archetypes.Teams.Team0" {
                    ActorKind::BlueTeam
                } else if object_name == "Archetypes.Teams.Team1" {
                    ActorKind::OrangeTeam
                } else if object_name == "TAGame.Default__PRI_TA"
                    || object_name.ends_with(".Default__PRI_TA")
                {
                    ActorKind::PlayerReplicationInfo
                } else if object_name.starts_with("Archetypes.GameEvent.GameEvent") {
                    ActorKind::GameEvent
                } else if object_name == "Engine.Default__GameReplicationInfo"
                    || object_name == "ProjectX.Default__GRI_X"
                    || object_name.ends_with(".Default__GRI_X")
                {
                    ActorKind::GameReplicationInfo
                } else if object_name.contains("VehiclePickup")
                    || object_name.contains("Pickup_Boost")
                    || object_name.contains("BoostPickup")
                {
                    ActorKind::BoostPad
                } else {
                    ActorKind::Other
                }
            })
    }

    fn object_name(&self, object_id: ObjectId) -> Result<&'a str, ConvertError> {
        self.objects
            .get(usize::from(object_id))
            .map(String::as_str)
            .ok_or(ConvertError::InvalidObjectId(object_id))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ReplayBallState {
    pub(crate) state: BallState,
    pub(crate) phys_freshness: PhysFreshness,
    pub(crate) metadata: FrameBallMetadata,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReplayCarState {
    pub(crate) info: CarInfo,
    pub(crate) state: CarState,
    pub(crate) phys_freshness: PhysFreshness,
}

#[derive(Debug, Clone, Copy, Default)]
struct BoostTiming {
    boosting_time: f32,
    time_since_boosted: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CarActionState {
    jump: bool,
    boost: bool,
    handbrake: bool,
    dodge: bool,
    double_jump: bool,
    flip_car: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ComponentKey {
    car_actor_id: ActorId,
    kind: ActorKind,
}

#[derive(Debug, Clone)]
struct ActorState<'a> {
    new_actor: NewActor,
    kind: ActorKind,
    attributes: FxHashMap<&'a str, Attribute>,
    phys: AccumulatedPhys,
    demo_respawn_timer: f32,
}

impl ActorState<'_> {
    fn boost_pad_cooldown(&self) -> f32 {
        // Standard Soccar cooldowns. This assumes pickup actors have normal pad locations.
        if self.phys.pos().is_some_and(|pos| pos.z > 71.0) {
            10.0
        } else {
            4.0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ActorKind {
    Ball,
    Car,
    BoostComponent,
    JumpComponent,
    DodgeComponent,
    DoubleJumpComponent,
    FlipCarComponent,
    BlueTeam,
    OrangeTeam,
    BoostPad,
    PlayerReplicationInfo,
    GameEvent,
    GameReplicationInfo,
    Other,
}

fn active_actor_id(attribute: Option<&Attribute>) -> Option<ActorId> {
    match attribute {
        Some(Attribute::ActiveActor(active_actor)) if active_actor.active => {
            Some(active_actor.actor)
        }
        _ => None,
    }
}

fn nearest_boost_pad(
    boost_pad_configs: &[BoostPadConfig],
    pos: rocketsim::Vec3A,
) -> Option<(usize, f32)> {
    boost_pad_configs
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.pos
                .distance_squared(pos)
                .total_cmp(&b.pos.distance_squared(pos))
        })
        .map(|(idx, config)| (idx, config.pos.distance(pos)))
}

fn infer_team(actor: &ActorState, idx: usize) -> Team {
    if let Some(Attribute::ActiveActor(active_actor)) = actor.attributes.get(CAR_PRI_ATTR) {
        // Deterministic fallback when PRI->team links are unavailable.
        if active_actor.actor.0.rem_euclid(2) == 1 {
            return Team::Orange;
        }
    }

    if idx.is_multiple_of(2) {
        Team::Blue
    } else {
        Team::Orange
    }
}

#[cfg(test)]
mod tests {
    use boxcars::{Pickup, StreamId, Trajectory};

    use super::*;
    use crate::metadata::BoostPickupSource;

    #[test]
    fn pickup_released_clears_replay_pad_cooldown() {
        let objects = vec![
            "Archetypes.Pickups.Pickup_Boost_TA".to_owned(),
            PICKUP_ATTR.to_owned(),
        ];
        let mut tracker = ActorTracker::new(&objects, 7);
        let pad_actor_id = ActorId(10);
        tracker.apply_new_actors(&[new_actor(pad_actor_id, ObjectId(0))], 0, 0.0);

        tracker
            .apply_updated_actors(
                &[pickup_update(pad_actor_id, ObjectId(1), true)],
                0.0,
                1,
                0.1,
            )
            .unwrap();
        assert!(tracker.boost_pad_cooldowns.contains_key(&pad_actor_id));

        tracker
            .apply_updated_actors(
                &[pickup_update(pad_actor_id, ObjectId(1), false)],
                0.0,
                2,
                0.2,
            )
            .unwrap();
        assert_eq!(tracker.boost_pad_cooldowns.get(&pad_actor_id), None);
        assert!(matches!(
            tracker.current_events.as_slice(),
            [FrameReplayEvent::BoostPickup(FrameBoostPickupEvent {
                kind: BoostPickupKind::Released,
                ..
            })]
        ));
    }

    #[test]
    fn replay_events_are_fresh_per_frame_and_use_active_frame_time() {
        let objects = vec![
            "Archetypes.Pickups.Pickup_Boost_TA".to_owned(),
            PICKUP_ATTR.to_owned(),
        ];
        let mut tracker = ActorTracker::new(&objects, 7);
        let pad_actor_id = ActorId(10);
        tracker.apply_new_actors(&[new_actor(pad_actor_id, ObjectId(0))], 0, 0.0);

        tracker
            .apply_updated_actors(
                &[pickup_update(pad_actor_id, ObjectId(1), true)],
                0.1,
                2,
                0.2,
            )
            .unwrap();

        let frame_metadata = tracker.frame_metadata();
        assert!(matches!(
            frame_metadata.events.as_slice(),
            [FrameReplayEvent::BoostPickup(FrameBoostPickupEvent {
                frame: 2,
                time: 0.2,
                kind: BoostPickupKind::PickedUp,
                ..
            })]
        ));

        tracker.begin_frame(0.1, 3, 0.3);
        assert!(tracker.frame_metadata().events.is_empty());
    }

    #[test]
    fn boost_timing_uses_direct_boost_active_history() {
        let mut tracker = ActorTracker::new(&[], 7);
        let car_actor_id = ActorId(7);

        tracker.current_replay_delta = 0.1;
        let first_boost = tracker.boost_timing(car_actor_id, true);
        assert_float_eq(first_boost.boosting_time, 0.1);
        assert_float_eq(first_boost.time_since_boosted, 0.0);
        tracker.boost_timing.insert(car_actor_id, first_boost);

        tracker.current_replay_delta = 0.2;
        let second_boost = tracker.boost_timing(car_actor_id, true);
        assert_float_eq(second_boost.boosting_time, 0.3);
        assert_float_eq(second_boost.time_since_boosted, 0.0);
        tracker.boost_timing.insert(car_actor_id, second_boost);

        tracker.current_replay_delta = 0.3;
        let released = tracker.boost_timing(car_actor_id, false);
        assert_float_eq(released.boosting_time, 0.0);
        assert_float_eq(released.time_since_boosted, 0.3);
    }

    #[test]
    fn pickup_event_kind_tracks_pickup_and_release() {
        let instigator = Some(ActorId(3));
        assert_eq!(
            pickup_event_kind_and_instigator(&Attribute::Pickup(Pickup {
                instigator,
                picked_up: true,
            })),
            Some((
                BoostPickupKind::PickedUp,
                instigator,
                BoostPickupSource::ReplicatedPickupData
            ))
        );
        assert_eq!(
            pickup_event_kind_and_instigator(&Attribute::Pickup(Pickup {
                instigator,
                picked_up: false,
            })),
            Some((
                BoostPickupKind::Released,
                instigator,
                BoostPickupSource::ReplicatedPickupData
            ))
        );
    }

    fn new_actor(actor_id: ActorId, object_id: ObjectId) -> NewActor {
        NewActor {
            actor_id,
            name_id: None,
            object_id,
            initial_trajectory: Trajectory {
                location: None,
                rotation: None,
            },
        }
    }

    fn pickup_update(actor_id: ActorId, object_id: ObjectId, picked_up: bool) -> UpdatedAttribute {
        UpdatedAttribute {
            actor_id,
            stream_id: StreamId(0),
            object_id,
            attribute: Attribute::Pickup(Pickup {
                instigator: Some(ActorId(3)),
                picked_up,
            }),
        }
    }

    fn assert_float_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.000_1,
            "expected {expected}, got {actual}"
        );
    }
}
