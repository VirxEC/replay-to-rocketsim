use boxcars::Attribute;
use rocketsim::Team;

use super::ActorState;
use super::attrs::{bool_attr, float_attr, vec3_attr};
use crate::attributes::{
    BALL_ADDED_CAR_BOUNCE_SCALE_ATTR, BALL_AIR_RESISTANCE_ATTR, BALL_GRAVITY_SCALE_ATTR,
    BALL_HIT_SPIN_SCALE_ATTR, BALL_MAX_SPEED_SCALE_ATTR, BALL_SCALE_ATTR, BALL_WARN_RESET_ATTR,
    BALL_WORLD_BOUNCE_SCALE_ATTR, PRI_CLIENT_LOADOUT_ATTR, PRI_CLIENT_LOADOUT_ONLINE_ATTR,
    PRI_CLIENT_LOADOUTS_ATTR, PRI_CLIENT_LOADOUTS_ONLINE_ATTR, PRI_PLAYER_NAME_ATTR,
    PRI_UNIQUE_ID_ATTR,
};
use crate::metadata::{FrameBallMetadata, FrameRigidBodyMetadata, PlayerMetadata, PlayerUniqueId};

pub(super) fn body_product_id_for_pri(pri: &ActorState, team: Team) -> Option<u32> {
    if let Some(Attribute::TeamLoadout(loadouts)) = pri
        .attributes
        .get(PRI_CLIENT_LOADOUTS_ATTR)
        .or_else(|| pri.attributes.get(PRI_CLIENT_LOADOUTS_ONLINE_ATTR))
    {
        return Some(match team {
            Team::Blue => loadouts.blue.body,
            Team::Orange => loadouts.orange.body,
        });
    }

    match pri
        .attributes
        .get(PRI_CLIENT_LOADOUT_ATTR)
        .or_else(|| pri.attributes.get(PRI_CLIENT_LOADOUT_ONLINE_ATTR))
    {
        Some(Attribute::Loadout(loadout)) => Some(loadout.body),
        _ => None,
    }
}

pub(super) fn player_metadata_key(pri: &ActorState, metadata: &PlayerMetadata) -> String {
    metadata.unique_id.as_ref().map_or_else(
        || format!("pri:{}", pri.new_actor.actor_id.0),
        PlayerUniqueId::stable_key,
    )
}

pub(super) fn merge_player_metadata(existing: &mut PlayerMetadata, incoming: &PlayerMetadata) {
    if should_replace_player_metadata(existing, incoming) {
        *existing = incoming.clone();
        return;
    }
    existing.unique_id = existing
        .unique_id
        .clone()
        .or_else(|| incoming.unique_id.clone());
    existing.name = existing.name.clone().or_else(|| incoming.name.clone());
    existing.team = existing.team.or(incoming.team);
    existing.player_id = existing.player_id.or(incoming.player_id);
    existing.score = existing.score.or(incoming.score);
    existing.is_bot = existing.is_bot.or(incoming.is_bot);
    existing.is_spectator = existing.is_spectator.or(incoming.is_spectator);
    existing.match_score = existing.match_score.or(incoming.match_score);
    existing.match_goals = existing.match_goals.or(incoming.match_goals);
    existing.match_assists = existing.match_assists.or(incoming.match_assists);
    existing.match_saves = existing.match_saves.or(incoming.match_saves);
    existing.match_shots = existing.match_shots.or(incoming.match_shots);
    existing.match_demolishes = existing.match_demolishes.or(incoming.match_demolishes);
    existing.car_demolitions = existing.car_demolitions.or(incoming.car_demolitions);
    existing.self_demolitions = existing.self_demolitions.or(incoming.self_demolitions);
    existing.ping = existing.ping.or(incoming.ping);
}

pub(super) fn should_replace_player_metadata(
    existing: &PlayerMetadata,
    incoming: &PlayerMetadata,
) -> bool {
    incoming.match_score.unwrap_or(i32::MIN) > existing.match_score.unwrap_or(i32::MIN)
        || incoming.score.unwrap_or(i32::MIN) > existing.score.unwrap_or(i32::MIN)
}

pub(super) fn player_unique_id(pri: &ActorState) -> Option<PlayerUniqueId> {
    match pri.attributes.get(PRI_UNIQUE_ID_ATTR) {
        Some(Attribute::UniqueId(unique_id)) => Some(PlayerUniqueId::from(unique_id.as_ref())),
        _ => None,
    }
}

pub(super) fn player_name(pri: &ActorState) -> Option<String> {
    match pri.attributes.get(PRI_PLAYER_NAME_ATTR) {
        Some(Attribute::String(name)) => Some(name.clone()),
        _ => None,
    }
}

pub(super) fn ball_metadata(
    ball: &ActorState,
    current_hit_team_num: Option<u8>,
    current_replay_frame: usize,
    current_replay_time: f32,
) -> FrameBallMetadata {
    // Goal detection: ball sleeping while away from origin indicates a goal.
    let ball_goal_sleep = ball
        .phys
        .is_sleeping()
        .zip(ball.phys.pos())
        .is_some_and(|(is_sleeping, pos)| is_sleeping && (pos.x != 0.0 || pos.y != 0.0));
    FrameBallMetadata {
        rigid_body: rigid_body_metadata(ball, current_replay_frame, current_replay_time, false),
        hit_team_num: current_hit_team_num,
        ball_goal_sleep,
        scale: float_attr(&ball.attributes, BALL_SCALE_ATTR),
        gravity_scale: float_attr(&ball.attributes, BALL_GRAVITY_SCALE_ATTR),
        max_linear_speed_scale: float_attr(&ball.attributes, BALL_MAX_SPEED_SCALE_ATTR),
        world_bounce_scale: float_attr(&ball.attributes, BALL_WORLD_BOUNCE_SCALE_ATTR),
        added_car_bounce_scale: float_attr(&ball.attributes, BALL_ADDED_CAR_BOUNCE_SCALE_ATTR),
        hit_spin_scale: float_attr(&ball.attributes, BALL_HIT_SPIN_SCALE_ATTR),
        air_resistance: vec3_attr(&ball.attributes, BALL_AIR_RESISTANCE_ATTR),
        warn_ball_reset: bool_attr(&ball.attributes, BALL_WARN_RESET_ATTR),
    }
}

pub(super) fn rigid_body_metadata(
    actor: &ActorState,
    current_replay_frame: usize,
    current_replay_time: f32,
    force_zero_update_age: bool,
) -> FrameRigidBodyMetadata {
    let freshness = actor.phys.freshness();
    let age = |time: Option<f32>| time.map(|time| (current_replay_time - time).max(0.0));
    let any_phys_update_age = if force_zero_update_age {
        Some(0.0)
    } else {
        age(freshness.any_phys_time())
    };
    FrameRigidBodyMetadata {
        is_sleeping: actor.phys.is_sleeping(),
        pos_update_age: age(freshness.pos_time),
        rot_update_age: age(freshness.rot_time),
        vel_update_age: age(freshness.vel_time),
        ang_vel_update_age: age(freshness.ang_vel_time),
        sleeping_update_age: age(freshness.sleeping_time),
        any_phys_update_age,
        is_repeat: !freshness.any_current(current_replay_frame),
    }
}
