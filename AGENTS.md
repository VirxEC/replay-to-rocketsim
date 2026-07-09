# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project overview

`replay-to-rocketsim` converts raw Rocket League replay bytes into a sequence of replay-aligned `rocketsim::ArenaState` snapshots. Replays are sparse network/delta data nominally recorded at 30Hz; RocketSim physics runs at 120Hz. The converter should preserve replay-frame timing metadata and make the replay-time → RocketSim tick relationship explicit. Treat the fixed 30Hz → 120Hz `4 ticks/frame` relationship as nominal only; conversion should use replay frame timestamps/deltas when stepping RocketSim.

Useful reference repository in this workspace:

- `../rust-carball` — older replay analysis code. Most useful patterns are actor-state accumulation, rigid-body extraction, and cleanup passes.

Primary layout:

- `src/lib.rs` — library entrypoint and replay conversion API.
- `src/aerial.rs` — inverse aerial control solver for pitch/yaw/roll reconstruction.
- `examples/` — small runnable examples. Examples should take explicit replay paths; do not scan `replays/`.
- `tests/` — integration tests when replay fixtures or public API tests are needed.
- `collision_meshes/` — RocketSim collision meshes used when initializing/stepping arenas.
- `replays/` — local replay corpus. Subdirectories contain thousands of files.

## Important constraints

- **Do not list or recursively scan `replays/` or its subdirectories with tools.** The corpus is huge and can flood context. If a replay fixture is needed, ask for an explicit path or use `find` with a very narrow known filename pattern and bounded output.
- The library API should accept raw bytes (`&[u8]`) and return typed RocketSim state data; keep file IO in examples/tests/CLI wrappers.
- RocketSim uses `glam` types and re-exports them. Prefer `rocketsim::{Vec3A, Mat3A, Quat, ...}` to avoid version mismatches with a separate direct `glam` dependency.
- Use `boxcars` network parsing (`ParserBuilder::must_parse_network_data`) for replay ingestion.
- Replays contain sparse actor updates. Maintain actor state across frames before extracting car/ball state, but preserve per-frame freshness/update-age metadata so stale accumulated values do not overwrite RocketSim interpolation.

## Common commands

Run from repository root (`replay-to-rocketsim`).

- Check compilation: `cargo check`
- Run tests: `cargo test`
- Lint: `cargo clippy --all-targets --all-features`
- Format: `cargo +nightly fmt`
- Run example with an explicit replay: `cargo run --example convert -- <path-to-replay>`

## Simulation-relevant replay extraction TODOs

When improving replay fidelity, prioritize data that affects RocketSim state or forward simulation accuracy. Ignore cosmetic-only loadout data except where needed to infer the physics hitbox.

- **Timing and interpolation:** Use replay frame `time`/`delta` to determine RocketSim tick advancement. Keep `ROCKETSIM_TICKS_PER_REPLAY_FRAME` and frame-index tick helpers as nominal/public conveniences, not as the authoritative stepping rule for conversion. `FrameTiming.rocketsim_tick` should match the returned `ArenaState.tick_count`.
- **Event semantics:** Keep replay-authored events and RocketSim-simulated events distinct. `ConversionOutput::arena_events` are RocketSim `ArenaEvent`s emitted while stepping 120Hz ticks between replay timestamps and should carry RocketSim tick numbers. `ReplayFrameMetadata::events` are observed from replay network data and are replay-frame/time aligned, not sub-frame RocketSim tick events. Do not silently merge or deduplicate these streams without preserving provenance.
- **Sparse rigid-body freshness:** Actor rigid-body state is accumulated, but each physics field (`pos`, `rot_mat`, `vel`, `ang_vel`) may have different freshness because replay RB updates can omit velocity fields. When syncing to RocketSim, merge only fields updated on the current replay frame unless the arena/car was just initialized or recreated. Do not assign an entire stale accumulated `PhysState` over RocketSim-simulated state.

- **Car body / hitbox:** Replays can include PRI loadout attributes such as `TAGame.PRI_TA:ClientLoadout`, `ClientLoadouts`, `ClientLoadoutOnline`, and `ClientLoadoutsOnline`. `boxcars::Loadout::body` is a product id, not directly a `rocketsim::CarBodyConfig`; map body product ids to RocketSim hitbox families (`OCTANE`, `DOMINUS`, `PLANK`, `BREAKOUT`, `HYBRID`, `MERC`) and fall back conservatively when unknown.
- **Accurate car-player-team links:** Avoid team/order heuristics where possible. Link car actors through `Engine.Pawn:PlayerReplicationInfo`, then PRI actors through `Engine.PlayerReplicationInfo:Team`, then team actors/score through `Engine.TeamInfo:Score`. `../rust-carball/src/actor_handlers/{car,player,team}.rs` has useful patterns.
- **Boost amount and boost activation:** Track boost component actors linked through `TAGame.CarComponent_TA:Vehicle`. Use `TAGame.CarComponent_Boost_TA:ReplicatedBoostAmount` / `ReplicatedBoost` for car boost amount and `TAGame.CarComponent_TA:ReplicatedActive` for boost input/activation when available.
- **Controls / action signals:** Replay data directly exposes `throttle`, `steer`, `handbrake` as replicated byte/bool attributes, and `jump`, `boost` via component active states. Do **not** infer `pitch` from `throttle` or otherwise conflate ground and aerial controls.

  **Pitch/yaw/roll reconstruction (aerial controls):** Replays do **not** contain direct analog pitch/yaw/roll inputs, but these can be **reconstructed** from the car's observed rotation and angular velocity deltas between replay frames using an inverse aerial control solver. The rlgym-tools project implements this via `aerial_inputs` (based on Sam Mish/ZealanL's work): given `(ang_vel_start, ang_vel_end, rot_mat_start, rot_mat_end, dt, is_flipping)`, it computes the net torque required to produce the observed angular velocity change, transforms to local coordinates, and solves for stick inputs using known torque/drag coefficients. Implement this pattern. The function signature `aerial_inputs(ang_vel_start, ang_vel_end, rot_mat_start, rot_mat_end, dt, is_flipping) -> (pitch, yaw, roll)` is the reference.

  **Dodge → pitch/yaw/roll:** When a dodge edge is detected (`TAGame.CarComponent_Dodge_TA:ReplicatedActive` rising), compute pitch/yaw/roll from the dodge torque vector. rlgym-tools does: `pitch = -dodge_torque_y / 2.24`, `roll = -dodge_torque_x / 2.6`, then projects to a unit square. For a stall (both torque components zero beyond deadzone), set `roll = ±1, yaw = -roll, pitch = 0`.

  **Throttle/steer processing:** Apply a small deadzone — zero out throttle/steer when `abs(value) < 0.01`.

  **Jump/dodge/boost edge detection:** Detect rising edges (`current_active && !previous_active`) of `ReplicatedActive` for jump, dodge, double-jump, flip-car, and boost components. Ideally these edges should be shifted forward by 1 frame in the emitted action to account for frame delay between player input and replicated state (rlgym-tools uses a dataframe-wide `shift(-1)`). Our sequential per-frame processing applies the action on the frame the replay data shows it, which means aerial inputs and discrete actions are delayed by ~1 replay frame (~33ms) compared to the rlgym-tools model. The car's internal jump/dodge state flags (`has_jumped`, `has_double_jumped`, `has_flipped`, `air_time_since_jump`, `flip_time`, etc.) should be manipulated to align with replay-observed events when preparing state for RocketSim stepping.
- **Boost pad / pickup state:** Extract pickup actors/attributes (`Attribute::Pickup`, `PickupNew`, `PickupInfo`, and related object names) so `ArenaState` can reflect boost pad availability before stepping RocketSim between replay frames. When a boost pickup is detected, link it to the nearest boost pad by position (within a radius threshold, ~200 units). rlgym-tools uses this proximity approach for pad indexing.
- **Demolitions and respawns:** Extract `TAGame.Car_TA:ReplicatedDemolish` (`Attribute::Demolish` / `DemolishExtended`) and related car lifecycle state so demo/respawn periods do not get simulated as normal free car physics. Demo state should override stale car physics application. When a car is demoed, detect the bumping car by proximity (nearest non-demoed car within a reasonable distance threshold, e.g. `200 + 2 * CAR_MAX_SPEED * avg_tick_rate / TICKS_PER_SECOND`). Note: RocketSim's `CarState` does not expose a `bump_victim_id` field, so bump victim tracking is limited to metadata/events rather than being set on the arena car state.

- **Game mode and arena setup:** Prefer replay metadata/actor data that identifies game mode, map, mutators, or non-standard arena setup over hardcoded `GameMode::Soccar` when it affects collision, gravity, ball behavior, or spawn/boost layout.
- **Ball state beyond rigid body:** Preserve simulation-affecting ball attributes such as sleep/contact/reset state when available. Ball touch/team metadata can be useful for validation, but should not be prioritized over state needed to reproduce physics.
- **rlgym-tools/carball metadata parity:** Preserve typed replay/game/frame/player/team/car metadata that downstream tooling needs (`metadata`, `frame_metadata`, `derived_frame_metadata`, `gameplay_periods`, `players`, `teams`, `cars`). Treat per-frame signals such as `TAGame.Ball_TA:HitTeamNum` as fresh only on frames where the replay updates them; do not let accumulated stale values drive gameplay-period inference. Prefer live `Engine.TeamInfo:Score` fields over synthesized goal increments when both are present.

  **Flip resets:** Detect when `TAGame.Car_TA:DodgesRefreshedCounter` changes (rlgym-tools uses `pdf["got_flip_reset"] = pdf["dodges_refreshed_counter"].diff() > 0`). When a flip reset occurs and the car is airborne without a flip, reset jump/dodge state flags (`has_jumped`, `has_double_jumped`, `has_flipped`, `air_time_since_jump`, `flip_time`) so the car can flip again.

  **Scoreboard / game timer reconstruction:** Do not trust `seconds_remaining` at face value — it is the ceil of the true game timer and can be 1 second off. Reconstruct the true timer by finding the first decrease in `seconds_remaining` after the ball has been hit, then computing elapsed time from that anchor. Handle overtime (infinite timer) and prevent negative values.

  **Kickoff timer:** At kickoff (periods where `ball_has_been_hit` is false), compute the kickoff countdown as `(5.0 - elapsed_time_since_start).clip(0, 5)` rather than leaving it unset.

  **Gameplay period splitting:** When the last gameplay period in the analyzer spans both regulation and overtime (detect via overtime flag change within the period), split it into two periods at the boundary.

  **Goal detection (not yet implemented):** When ball sleep state changes at a position away from the origin (`pos_x != 0 || pos_y != 0`) before the frame the analyzer marks as the goal frame, treat the earlier sleep frame as the actual goal. Ball data after that point is unreliable.

## Development conventions

- Keep changes focused and minimal.
- Prefer existing dependencies. If adding dependencies, use `cargo add` / `cargo search` and prefer current stable versions compatible with the project.
- Do not hardcode replay paths or generated output paths.
- Do not commit generated outputs, replay files, benchmark artifacts, or `target/` contents.
- Preserve typed domain structures instead of loosely passing JSON/maps through public APIs.
- Return `Result` errors from library code rather than panicking on malformed replay data.

## Validation expectations

Before finishing Rust changes:

1. Run `cargo +nightly fmt`.
2. Run `cargo check`.
3. Run `cargo test` when behavior changed.
4. Run `cargo clippy --all-targets --all-features` for non-trivial changes.

If validation cannot run because nightly is unavailable, RocketSim build prerequisites are missing, or a replay fixture path is unavailable, mention that clearly.
