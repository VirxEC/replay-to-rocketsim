# replay-to-rocketsim

Convert Rocket League replay bytes into replay-aligned [`rocketsim`](https://github.com/VirxEC/RocketSim) arena snapshots.

This crate parses replay network data with [`boxcars`](https://crates.io/crates/boxcars), accumulates sparse replay actor updates, steps RocketSim at 120Hz between replay timestamps, and returns typed `rocketsim::ArenaState` values aligned to replay frames.

## Why this exists

Rocket League replays are not full physics recordings. They are sparse network/delta data, nominally recorded around 30Hz. RocketSim, meanwhile, runs physics at 120Hz.

`replay-to-rocketsim` bridges those worlds:

- replay data provides ground-truth corrections for positions, rotations, velocities, controls, boost, demos, pickups, and metadata when those fields are fresh;
- RocketSim fills the 120Hz simulation between replay timestamps;
- output states remain aligned to replay frames so downstream tools can join physics, metadata, players, teams, scores, and events.

## Current status

This is a focused conversion library, not a perfect replay resimulator. It aims to preserve replay truth while producing RocketSim-native state that is useful for analysis and downstream ML/gameplay tooling.

Important caveats:

- Replay actor updates are sparse. A frame might update position but not velocity, or rotation but not angular velocity.
- The converter tracks per-field freshness and avoids overwriting RocketSim-simulated fields with stale accumulated replay values.
- Replay-authored events and RocketSim-simulated events are intentionally separate streams.
- Exact sub-frame timing of replay-authored events is generally not recovered; replay events are aligned to replay frame/time.
- Player aerial inputs (pitch/yaw/roll) are not directly recorded in replay data. They are **reconstructed** from angular velocity deltas between frames using an inverse aerial control solver. Discrete actions (jump, boost, dodge, handbrake) are read from replicated component states.

## Quick start

Add the crate in a Rust project, initialize RocketSim, read replay bytes, and convert:

```rust
use std::fs;

use replay_to_rocketsim::Converter;
use replay_to_rocketsim::rocketsim::init_from_default;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_from_default(true)?;

    let replay_bytes = fs::read("match.replay")?;
    let output = Converter::new().convert_bytes(&replay_bytes)?;

    println!("frames: {}", output.frames.len());
    println!("states: {}", output.states.len());

    if let Some(last_state) = output.states.last() {
        println!("cars: {}", last_state.num_cars());
        println!("ball position: {:?}", last_state.ball.phys.pos);
    }

    Ok(())
}
```

The lower-level helper is also available if you only need snapshots:

```rust
use replay_to_rocketsim::replay_bytes_to_rocketsim_states;

let states = replay_bytes_to_rocketsim_states(&replay_bytes)?;
```

For most callers, prefer `Converter::convert_bytes` because `ConversionOutput` includes timing, metadata, cars, players, teams, replay events, and RocketSim arena events.

## Timing model: 30Hz replay vs 120Hz RocketSim

Replay network frames are nominally 30Hz. RocketSim physics ticks are 120Hz. That gives a nominal relationship of:

```text
1 replay frame ≈ 4 RocketSim ticks
```

The crate exposes that as:

```rust
use replay_to_rocketsim::ROCKETSIM_TICKS_PER_REPLAY_FRAME;
```

But conversion does **not** blindly use `replay_frame * 4` as the authoritative tick. Instead it maps replay timestamps to RocketSim ticks:

```rust
use replay_to_rocketsim::replay_time_to_rocketsim_tick;

let tick = replay_time_to_rocketsim_tick(replay_time_seconds);
```

For every returned frame:

```text
output.frames[i].rocketsim_tick == output.states[i].tick_count
```

That means irregular replay timing is represented explicitly, while the fixed 4-ticks-per-frame mapping remains available only as a nominal helper.

## How replay state is merged with RocketSim

The converter accumulates actor state across frames because replay updates are sparse. However, each rigid-body physics field tracks freshness independently:

- position
- rotation matrix
- linear velocity
- angular velocity

On normal frame syncs, only fields updated on the current replay frame overwrite RocketSim state. Stale accumulated replay fields are not repeatedly applied over the simulated state.

For example, if a replay frame updates car position and rotation but omits velocity, the converter corrects position/rotation while preserving RocketSim's simulated velocity.

Full physics state is applied when a ball/car is initialized, added, or the arena/car layout is recreated.

## Event streams

`ConversionOutput` intentionally separates two event sources.

### `arena_events`: RocketSim-simulated events

```rust
output.arena_events[frame_index]
```

These are `rocketsim::ArenaEvent`s emitted while RocketSim steps the 120Hz ticks between the previous replay timestamp and the current replay timestamp.

Each event has a RocketSim tick:

```rust
for event in &output.arena_events[frame_index] {
    println!("tick={} event={:?}", event.tick, event.event);
}
```

Use this stream when you care about events generated by RocketSim's simulation.

### `frame_metadata.events`: replay-authored events

```rust
output.frame_metadata[frame_index].events
```

These are events observed directly in replay network data, such as:

- demos
- boost pickups/releases
- car actions
- car lifecycle edges

They are aligned to replay frame/time, not sub-frame RocketSim ticks:

```rust
for event in &output.frame_metadata[frame_index].events {
    println!("replay event: {event:?}");
}
```

Do not silently merge or deduplicate `arena_events` and `frame_metadata.events` unless your downstream code preserves provenance.

## Examples

Run examples with explicit replay paths:

```sh
cargo run --example convert -- <path-to-replay>
```

Print basic conversion/timing/state information:

```sh
cargo run --example convert -- <path-to-replay>
```

Inspect replay-authored events and RocketSim-simulated arena events:

```sh
cargo run --example inspect_events -- <path-to-replay> [max-events]
```

Audit car state extraction against raw replay attributes:

```sh
cargo run --example audit_car_state -- <path-to-replay> [...more-replays]
```

Inspect raw attributes in a replay:

```sh
cargo run --example inspect_attributes -- <path-to-replay>
```

## Public API overview

Common exports:

- `Converter` — configurable conversion entrypoint.
- `ConversionOutput` — full result containing states, timing, metadata, players, teams, cars, and event streams.
- `FrameTiming` — replay frame/time/delta and corresponding RocketSim tick.
- `FrameArenaEvent` — RocketSim event plus RocketSim tick.
- `ReplayFrameMetadata` — replay-authored per-frame metadata and events.
- `replay_time_to_rocketsim_tick` — timestamp-based tick mapping used by conversion.
- `replay_frame_to_rocketsim_tick` — nominal fixed-rate helper.
- `rocketsim` — re-exported RocketSim crate, including its math types.

RocketSim uses `glam` types and re-exports them. Prefer the re-exported RocketSim types from this crate to avoid version mismatches:

```rust
use replay_to_rocketsim::rocketsim::{ArenaState, Vec3A, Mat3A};
```

## Development

Run from the repository root:

```sh
cargo +nightly fmt
cargo check
cargo test
cargo clippy --all-targets --all-features
```

Notes for contributors:

- Keep file IO in examples/tests/CLI wrappers; library APIs should accept raw bytes.
- Do not hardcode replay paths.
- Do not recursively scan `replays/`; it is a large local corpus.
- Prefer typed domain structures over loose JSON/maps.
- Return `Result` errors from library code rather than panicking on malformed replay data.

## License

MIT. See `LICENSE`.
