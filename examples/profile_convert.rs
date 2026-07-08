use std::error::Error;
use std::hint::black_box;
use std::time::Instant;
use std::{env, fs};

use replay_to_rocketsim::Converter;
use replay_to_rocketsim::rocketsim::init_from_default;

const DEFAULT_ITERATIONS: usize = 10;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "profile_convert".to_owned());
    let replay_path = args
        .next()
        .ok_or_else(|| format!("usage: {program} <path-to-replay> [iterations]"))?;
    let iterations = args.next().map_or(Ok(DEFAULT_ITERATIONS), |value| {
        value
            .parse::<usize>()
            .map_err(|err| format!("invalid iteration count {value:?}: {err}"))
    })?;

    if iterations == 0 {
        return Err("iteration count must be greater than 0".into());
    }

    init_from_default(true)?;

    let replay_bytes = fs::read(&replay_path)?;
    let converter = Converter::new();
    let start = Instant::now();
    let mut total_frames = 0usize;
    let mut total_states = 0usize;

    for _ in 0..iterations {
        let output = converter.convert_bytes(black_box(&replay_bytes))?;
        total_frames += output.frames.len();
        total_states += output.states.len();
        black_box(output);
    }

    let elapsed = start.elapsed();
    println!(
        "converted {replay_path} {iterations} times in {elapsed:.2?} ({total_frames} replay frames, {total_states} RocketSim snapshots total)"
    );

    Ok(())
}
