use std::error::Error;
use std::{env, fs};

use replay_to_rocketsim::Converter;
use replay_to_rocketsim::rocketsim::init_from_default;

fn main() -> Result<(), Box<dyn Error>> {
    let replay_path = env::args()
        .nth(1)
        .ok_or("usage: cargo run --example convert -- <path-to-replay>")?;

    init_from_default(true)?;

    let replay_bytes = fs::read(replay_path)?;
    let output = Converter::new().convert_bytes(&replay_bytes)?;

    println!(
        "converted {} replay frames into {} RocketSim snapshots ({}Hz -> {}Hz)",
        output.frames.len(),
        output.states.len(),
        output.replay_hz,
        output.rocketsim_hz
    );

    if let Some(first_frame) = output.frames.first() {
        println!(
            "first frame: replay_frame={} time={:.3}s rocketsim_tick={}",
            first_frame.replay_frame, first_frame.time, first_frame.rocketsim_tick
        );
    }

    if let Some(last_frame) = output.frames.last() {
        println!(
            "last frame: replay_frame={} time={:.3}s rocketsim_tick={}",
            last_frame.replay_frame, last_frame.time, last_frame.rocketsim_tick
        );
    }

    if let Some(last_state) = output.states.last() {
        println!(
            "last state: cars={} ball_pos={:?}",
            last_state.num_cars(),
            last_state.ball.phys.pos
        );
    }

    Ok(())
}
