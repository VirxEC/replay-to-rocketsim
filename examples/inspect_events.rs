use std::{env, fs};

use replay_to_rocketsim::rocketsim::init_from_default;
use replay_to_rocketsim::{BoostPickupKind, Converter, FrameReplayEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let replay_path = env::args()
        .nth(1)
        .ok_or("usage: inspect_events <path-to-replay> [max-events]")?;
    let max_events = env::args()
        .nth(2)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200);

    init_from_default(true)?;

    let bytes = fs::read(&replay_path)?;
    let output = Converter::new().convert_bytes(&bytes)?;

    let simulated_arena_events = output.arena_events.iter().map(Vec::len).sum::<usize>();
    let mut demo_events = 0usize;
    let mut boost_pickups = 0usize;
    let mut boost_releases = 0usize;
    let mut car_action_events = 0usize;
    let mut car_lifecycle_events = 0usize;
    for metadata in &output.frame_metadata {
        for event in &metadata.events {
            match event {
                FrameReplayEvent::Demo(_) => demo_events += 1,
                FrameReplayEvent::BoostPickup(event) => match event.kind {
                    BoostPickupKind::PickedUp => boost_pickups += 1,
                    BoostPickupKind::Released => boost_releases += 1,
                },
                FrameReplayEvent::CarAction(_) => car_action_events += 1,
                FrameReplayEvent::CarLifecycle(_) => car_lifecycle_events += 1,
            }
        }
    }

    println!(
        "frames={} states={} players={} gameplay_periods={} simulated_arena_events={} replay_demo_events={} replay_boost_pickups={} replay_boost_releases={} replay_car_action_events={} replay_car_lifecycle_events={} demo_mode={:?}",
        output.frames.len(),
        output.states.len(),
        output.players.len(),
        output.gameplay_periods.len(),
        simulated_arena_events,
        demo_events,
        boost_pickups,
        boost_releases,
        car_action_events,
        car_lifecycle_events,
        output.arena_config.mutators.demo_mode,
    );

    let mut printed = 0usize;
    for (frame_idx, (timing, cars)) in output.frames.iter().zip(output.cars.iter()).enumerate() {
        for event in &output.arena_events[frame_idx] {
            println!(
                "frame={} t={:.3} rocketsim_tick={} simulated_arena_event={:?}",
                timing.replay_frame, timing.time, event.tick, event.event
            );
            printed += 1;
            if printed >= max_events {
                return Ok(());
            }
        }

        let metadata = &output.frame_metadata[frame_idx];
        for event in &metadata.events {
            println!(
                "frame={} t={:.3} replay_event={event:?}",
                timing.replay_frame, timing.time
            );
            printed += 1;
            if printed >= max_events {
                return Ok(());
            }
        }

        let state = &output.states[frame_idx];
        for car in cars {
            let state_car = state.cars.get(car.car_idx).map(|(_, state)| state);
            let has_interesting_state = state_car.is_some_and(|state| {
                state.controls.jump
                    || state.controls.boost
                    || state.prev_controls.jump
                    || state.prev_controls.boost
                    || state.is_boosting
                    || state.boosting_time > 0.0
                    || state.time_since_boosted > 0.0
                    || state.is_demoed
            });
            if car.jump_is_active == Some(true)
                || car.dodge_is_active == Some(true)
                || car.double_jump_is_active == Some(true)
                || car.flip_car_is_active == Some(true)
                || car.dodge_torque.is_some()
                || car.dodge_impulse.is_some()
                || car.double_jump_impulse.is_some()
                || car.dodges_refreshed_counter.is_some()
                || has_interesting_state
            {
                println!(
                    "frame={} t={:.3} car_idx={} actor={} player={:?} jump={:?} dodge={:?} dodge_torque={:?} dodge_impulse={:?} double_jump={:?} double_jump_impulse={:?} flip={:?} flip_time={:?} flip_right={:?} resets={:?} state={}",
                    timing.replay_frame,
                    timing.time,
                    car.car_idx,
                    car.car_actor_id,
                    car.player_name,
                    car.jump_is_active,
                    car.dodge_is_active,
                    car.dodge_torque,
                    car.dodge_impulse,
                    car.double_jump_is_active,
                    car.double_jump_impulse,
                    car.flip_car_is_active,
                    car.flip_car_time,
                    car.flip_car_right,
                    car.dodges_refreshed_counter,
                    state_car.map_or_else(
                        || "<missing>".to_owned(),
                        |state| format!(
                            "controls(jump={} boost={}) prev(jump={} boost={}) is_boosting={} boosting_time={:.3} time_since_boosted={:.3} boost={:.1} demoed={} demo_timer={:.3}",
                            state.controls.jump,
                            state.controls.boost,
                            state.prev_controls.jump,
                            state.prev_controls.boost,
                            state.is_boosting,
                            state.boosting_time,
                            state.time_since_boosted,
                            state.boost,
                            state.is_demoed,
                            state.demo_respawn_timer,
                        )
                    ),
                );
                printed += 1;
                if printed >= max_events {
                    return Ok(());
                }
            }
        }
    }

    if printed == 0 {
        println!(
            "no simulated arena events, replay events, or jump/dodge/flip component activity found"
        );
    }

    Ok(())
}
