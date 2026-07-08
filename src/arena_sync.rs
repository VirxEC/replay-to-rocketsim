use rocketsim::{Arena, PhysState};

use crate::actor::{ReplayBallState, ReplayCarState};
use crate::phys::PhysFreshness;

pub(crate) fn arena_car_layout_changed(arena: &Arena, cars: &[ReplayCarState]) -> bool {
    arena.num_cars() > cars.len()
        || cars.iter().enumerate().any(|(car_idx, replay_car)| {
            car_idx < arena.num_cars()
                && (arena.get_car_info(car_idx).team != replay_car.info.team
                    || arena.get_car_info(car_idx).config != replay_car.info.config)
        })
}

pub(crate) fn sync_arena_to_replay_state(
    arena: &mut Arena,
    ball: &ReplayBallState,
    cars: &[ReplayCarState],
    boost_pads: &[(usize, rocketsim::BoostPadState)],
    replay_frame: usize,
    force_phys_sync: bool,
) {
    if force_phys_sync {
        arena.set_ball_state(ball.state);
    } else if ball.phys_freshness.any_current(replay_frame) {
        let mut current_ball = *arena.get_ball_state();
        merge_fresh_phys_fields(
            &mut current_ball.phys,
            ball.state.phys,
            ball.phys_freshness,
            replay_frame,
        );
        arena.set_ball_state(current_ball);
    }

    for &(idx, boost_pad) in boost_pads {
        if idx < arena.num_boost_pads() {
            arena.set_boost_pad_state(idx, boost_pad);
        }
    }

    for (car_idx, replay_car) in cars.iter().enumerate() {
        let added_car = car_idx >= arena.num_cars();
        if added_car {
            arena.add_car(replay_car.info.team, replay_car.info.config);
        }

        let mut car = *arena.get_car_state(car_idx);
        if force_phys_sync || added_car {
            car.phys = replay_car.state.phys;
        } else {
            merge_fresh_phys_fields(
                &mut car.phys,
                replay_car.state.phys,
                replay_car.phys_freshness,
                replay_frame,
            );
        }
        car.controls = replay_car.state.controls;
        car.prev_controls = replay_car.state.prev_controls;
        car.boost = replay_car.state.boost;
        car.is_boosting = replay_car.state.is_boosting;
        car.boosting_time = replay_car.state.boosting_time;
        car.time_since_boosted = replay_car.state.time_since_boosted;
        car.is_demoed = replay_car.state.is_demoed;
        car.demo_respawn_timer = replay_car.state.demo_respawn_timer;
        arena.set_car_state(car_idx, car);
    }
}

fn merge_fresh_phys_fields(
    target: &mut PhysState,
    replay: PhysState,
    freshness: PhysFreshness,
    replay_frame: usize,
) {
    if freshness.pos_frame == Some(replay_frame) {
        target.pos = replay.pos;
    }
    if freshness.rot_frame == Some(replay_frame) {
        target.rot_mat = replay.rot_mat;
    }
    if freshness.vel_frame == Some(replay_frame) {
        target.vel = replay.vel;
    }
    if freshness.ang_vel_frame == Some(replay_frame) {
        target.ang_vel = replay.ang_vel;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use rocketsim::{CarBodyConfig, CarInfo, CarState, GameMode, Team, Vec3A, init_from_default};

    use super::*;

    static INIT_ROCKETSIM: Once = Once::new();

    #[test]
    fn stale_replay_car_phys_does_not_overwrite_existing_arena_phys() {
        init_rocketsim();
        let mut arena = Arena::new(GameMode::Soccar);
        arena.add_car(Team::Blue, CarBodyConfig::OCTANE);

        let mut simulated_car = *arena.get_car_state(0);
        simulated_car.phys.pos = Vec3A::new(10.0, 20.0, 30.0);
        arena.set_car_state(0, simulated_car);

        let replay_car = replay_car_at(Vec3A::new(100.0, 200.0, 300.0), PhysFreshness::default());

        sync_arena_to_replay_state(
            &mut arena,
            &ReplayBallState::default(),
            &[replay_car],
            &[],
            7,
            false,
        );

        assert_vec3a_near(
            arena.get_car_state(0).phys.pos,
            Vec3A::new(10.0, 20.0, 30.0),
        );
    }

    #[test]
    fn fresh_replay_car_phys_overwrites_existing_arena_phys() {
        init_rocketsim();
        let mut arena = Arena::new(GameMode::Soccar);
        arena.add_car(Team::Blue, CarBodyConfig::OCTANE);

        let mut simulated_car = *arena.get_car_state(0);
        simulated_car.phys.pos = Vec3A::new(10.0, 20.0, 30.0);
        arena.set_car_state(0, simulated_car);

        let replay_car = replay_car_at(
            Vec3A::new(100.0, 200.0, 300.0),
            PhysFreshness {
                pos_frame: Some(7),
                ..PhysFreshness::default()
            },
        );

        sync_arena_to_replay_state(
            &mut arena,
            &ReplayBallState::default(),
            &[replay_car],
            &[],
            7,
            false,
        );

        assert_vec3a_near(
            arena.get_car_state(0).phys.pos,
            Vec3A::new(100.0, 200.0, 300.0),
        );
    }

    #[test]
    fn partial_replay_phys_update_preserves_stale_velocity_fields() {
        init_rocketsim();
        let mut arena = Arena::new(GameMode::Soccar);
        arena.add_car(Team::Blue, CarBodyConfig::OCTANE);

        let mut simulated_car = *arena.get_car_state(0);
        simulated_car.phys.pos = Vec3A::new(10.0, 20.0, 30.0);
        simulated_car.phys.vel = Vec3A::new(1.0, 2.0, 3.0);
        arena.set_car_state(0, simulated_car);

        let mut replay_car = replay_car_at(
            Vec3A::new(100.0, 200.0, 300.0),
            PhysFreshness {
                pos_frame: Some(7),
                ..PhysFreshness::default()
            },
        );
        replay_car.state.phys.vel = Vec3A::new(9.0, 9.0, 9.0);

        sync_arena_to_replay_state(
            &mut arena,
            &ReplayBallState::default(),
            &[replay_car],
            &[],
            7,
            false,
        );

        let car = arena.get_car_state(0);
        assert_vec3a_near(car.phys.pos, Vec3A::new(100.0, 200.0, 300.0));
        assert_vec3a_near(car.phys.vel, Vec3A::new(1.0, 2.0, 3.0));
    }

    fn init_rocketsim() {
        INIT_ROCKETSIM.call_once(|| {
            init_from_default(true).expect("RocketSim collision mesh initialization failed");
        });
    }

    fn replay_car_at(pos: Vec3A, phys_freshness: PhysFreshness) -> ReplayCarState {
        let mut state = CarState::default();
        state.phys.pos = pos;
        ReplayCarState {
            info: CarInfo {
                idx: 0,
                team: Team::Blue,
                config: CarBodyConfig::OCTANE,
            },
            state,
            phys_freshness,
        }
    }

    fn assert_vec3a_near(actual: Vec3A, expected: Vec3A) {
        assert!(
            actual.distance_squared(expected) < 0.001,
            "expected {expected:?}, got {actual:?}"
        );
    }
}
