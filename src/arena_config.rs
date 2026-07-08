use boxcars::Replay;
use rocketsim::{ArenaConfig, DemoMode, GameMode, MutatorConfig};

use crate::actor::ActorTracker;
use crate::error::ConvertError;
use crate::metadata::{ReplayGameMode, ReplayMutatorMetadata};

pub(crate) fn replay_arena_config(
    replay: &Replay,
    replay_version: i32,
) -> Result<Option<ArenaConfig>, ConvertError> {
    let Some(network_frames) = replay.network_frames.as_ref() else {
        return Ok(None);
    };
    let mut tracker = ActorTracker::new(&replay.objects, replay_version);
    let mut selected_game_mode = None;
    let mut selected_mutators = ReplayMutatorMetadata::default();

    for (replay_frame, frame) in network_frames.frames.iter().take(300).enumerate() {
        tracker.begin_frame(frame.delta, replay_frame, frame.time);
        tracker.apply_deleted_actors(&frame.deleted_actors);
        tracker.apply_new_actors(&frame.new_actors, replay_frame, frame.time);
        tracker.apply_updated_actors(
            &frame.updated_actors,
            frame.delta,
            replay_frame,
            frame.time,
        )?;
        let metadata = tracker.frame_metadata();
        selected_game_mode =
            selected_game_mode.or_else(|| rocket_sim_game_mode(metadata.game_event.game_mode));
        selected_mutators = merge_mutators(selected_mutators, metadata.mutators);
        if selected_game_mode.is_some() && selected_mutators.has_simulation_values() {
            break;
        }
    }

    if selected_game_mode.is_none() && !selected_mutators.has_simulation_values() {
        return Ok(None);
    }

    let game_mode = selected_game_mode.unwrap_or(GameMode::Soccar);
    let mut config = ArenaConfig::new(game_mode);
    apply_replay_mutators(&mut config.mutators, game_mode, selected_mutators);
    Ok(Some(config))
}

fn rocket_sim_game_mode(game_mode: Option<ReplayGameMode>) -> Option<GameMode> {
    match game_mode? {
        ReplayGameMode::Soccar => Some(GameMode::Soccar),
        ReplayGameMode::Hoops => Some(GameMode::Hoops),
        ReplayGameMode::Heatseeker => Some(GameMode::Heatseeker),
        ReplayGameMode::Snowday => Some(GameMode::Snowday),
        ReplayGameMode::Dropshot => Some(GameMode::Dropshot),
        ReplayGameMode::Unknown | ReplayGameMode::Other(_, _) => None,
    }
}

fn merge_mutators(
    mut current: ReplayMutatorMetadata,
    incoming: ReplayMutatorMetadata,
) -> ReplayMutatorMetadata {
    current.ball_scale = current.ball_scale.or(incoming.ball_scale);
    current.ball_gravity_scale = current.ball_gravity_scale.or(incoming.ball_gravity_scale);
    current.ball_max_linear_speed_scale = current
        .ball_max_linear_speed_scale
        .or(incoming.ball_max_linear_speed_scale);
    current.boost_recharge_delay = current
        .boost_recharge_delay
        .or(incoming.boost_recharge_delay);
    current.boost_recharge_rate = current.boost_recharge_rate.or(incoming.boost_recharge_rate);
    current.unlimited_boost = current.unlimited_boost.or(incoming.unlimited_boost);
    current.no_boost = current.no_boost.or(incoming.no_boost);
    current
}

trait ReplayMutatorMetadataExt {
    fn has_simulation_values(self) -> bool;
}

impl ReplayMutatorMetadataExt for ReplayMutatorMetadata {
    fn has_simulation_values(self) -> bool {
        self.ball_scale.is_some()
            || self.ball_gravity_scale.is_some()
            || self.ball_max_linear_speed_scale.is_some()
            || self.boost_recharge_delay.is_some()
            || self.boost_recharge_rate.is_some()
            || self.unlimited_boost.is_some()
            || self.no_boost.is_some()
    }
}

fn apply_replay_mutators(
    mutators: &mut MutatorConfig,
    game_mode: GameMode,
    replay_mutators: ReplayMutatorMetadata,
) {
    mutators.demo_mode = DemoMode::Disabled;
    if let Some(scale) = replay_mutators
        .ball_scale
        .filter(|scale| scale.is_finite() && *scale > 0.0)
    {
        mutators.ball_radius = rocketsim::consts::ball::get_radius(game_mode) * scale;
    }
    if let Some(scale) = replay_mutators
        .ball_gravity_scale
        .filter(|scale| scale.is_finite())
    {
        mutators.gravity = rocketsim::Vec3A::new(0.0, 0.0, rocketsim::consts::GRAVITY_Z * scale);
    }
    if let Some(scale) = replay_mutators
        .ball_max_linear_speed_scale
        .filter(|scale| scale.is_finite() && *scale > 0.0)
    {
        mutators.ball_max_speed = rocketsim::consts::ball::MAX_SPEED * scale;
    }
    if let Some(delay) = replay_mutators
        .boost_recharge_delay
        .filter(|delay| delay.is_finite() && *delay >= 0.0)
    {
        mutators.recharge_boost_enabled = true;
        mutators.recharge_boost_delay = delay;
    }
    if let Some(rate) = replay_mutators
        .boost_recharge_rate
        .filter(|rate| rate.is_finite() && *rate >= 0.0)
    {
        mutators.recharge_boost_enabled = true;
        mutators.recharge_boost_per_second = rate;
    }
    if replay_mutators.unlimited_boost == Some(true) {
        mutators.boost_used_per_second = 0.0;
    }
    if replay_mutators.no_boost == Some(true) {
        mutators.car_spawn_boost_amount = 0.0;
        mutators.car_max_boost_amount = 0.0;
        mutators.boost_pad_amount_big = 0.0;
        mutators.boost_pad_amount_small = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use rocketsim::{DemoMode, GameMode, MutatorConfig, Vec3A};

    use super::*;

    #[test]
    fn replay_mutators_disable_simulated_demos_and_apply_direct_values() {
        let mut mutators = MutatorConfig::new(GameMode::Soccar);

        apply_replay_mutators(
            &mut mutators,
            GameMode::Soccar,
            ReplayMutatorMetadata {
                ball_scale: Some(2.0),
                ball_gravity_scale: Some(0.5),
                ball_max_linear_speed_scale: Some(1.5),
                boost_recharge_delay: Some(0.25),
                boost_recharge_rate: Some(33.0),
                unlimited_boost: Some(true),
                no_boost: Some(true),
            },
        );

        assert_eq!(mutators.demo_mode, DemoMode::Disabled);
        assert!(
            (mutators.ball_radius - rocketsim::consts::ball::get_radius(GameMode::Soccar) * 2.0)
                .abs()
                < f32::EPSILON
        );
        assert_eq!(
            mutators.gravity,
            Vec3A::new(0.0, 0.0, rocketsim::consts::GRAVITY_Z * 0.5)
        );
        assert!(
            (mutators.ball_max_speed - rocketsim::consts::ball::MAX_SPEED * 1.5).abs()
                < f32::EPSILON
        );
        assert!(mutators.recharge_boost_enabled);
        assert!((mutators.recharge_boost_delay - 0.25).abs() < f32::EPSILON);
        assert!((mutators.recharge_boost_per_second - 33.0).abs() < f32::EPSILON);
        assert!(mutators.boost_used_per_second.abs() < f32::EPSILON);
        assert!(mutators.car_max_boost_amount.abs() < f32::EPSILON);
        assert!(mutators.boost_pad_amount_big.abs() < f32::EPSILON);
        assert!(mutators.boost_pad_amount_small.abs() < f32::EPSILON);
    }

    #[test]
    fn replay_mutators_ignore_invalid_physics_values() {
        let mut mutators = MutatorConfig::new(GameMode::Soccar);
        let original_ball_radius = mutators.ball_radius;
        let original_ball_max_speed = mutators.ball_max_speed;
        let original_recharge_enabled = mutators.recharge_boost_enabled;

        apply_replay_mutators(
            &mut mutators,
            GameMode::Soccar,
            ReplayMutatorMetadata {
                ball_scale: Some(0.0),
                ball_gravity_scale: Some(f32::NAN),
                ball_max_linear_speed_scale: Some(-1.0),
                boost_recharge_delay: Some(-1.0),
                boost_recharge_rate: Some(f32::INFINITY),
                ..ReplayMutatorMetadata::default()
            },
        );

        assert_eq!(mutators.demo_mode, DemoMode::Disabled);
        assert!((mutators.ball_radius - original_ball_radius).abs() < f32::EPSILON);
        assert!((mutators.ball_max_speed - original_ball_max_speed).abs() < f32::EPSILON);
        assert_eq!(mutators.recharge_boost_enabled, original_recharge_enabled);
    }

    #[test]
    fn maps_known_replay_game_modes_to_rocketsim() {
        assert_eq!(
            rocket_sim_game_mode(Some(ReplayGameMode::Soccar)),
            Some(GameMode::Soccar)
        );
        assert_eq!(
            rocket_sim_game_mode(Some(ReplayGameMode::Hoops)),
            Some(GameMode::Hoops)
        );
        assert_eq!(rocket_sim_game_mode(Some(ReplayGameMode::Unknown)), None);
        assert_eq!(
            rocket_sim_game_mode(Some(ReplayGameMode::Other(9, 9))),
            None
        );
    }
}
