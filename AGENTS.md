# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project overview

`replay-to-rocketsim` converts raw Rocket League replay bytes into a sequence of replay-aligned `rocketsim::ArenaState` snapshots. Replays are sparse network/delta data nominally recorded at 30Hz; RocketSim physics runs at 120Hz. The converter should preserve replay-frame timing metadata and make the replay-time → RocketSim tick relationship explicit. Treat the fixed 30Hz → 120Hz `4 ticks/frame` relationship as nominal only; conversion should use replay frame timestamps/deltas when stepping RocketSim.

Useful reference repository in this workspace:

- `../rust-carball` — older replay analysis code. Most useful patterns are actor-state accumulation, rigid-body extraction, and cleanup passes.

Primary layout:

- `src/lib.rs` — library entrypoint and replay conversion API.
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
- **Controls / action signals:** Do not infer `pitch` from throttle or otherwise conflate ground and aerial controls. Replay data directly exposes throttle/steer/handbrake/boost more reliably than pitch/yaw/roll; only map pitch/yaw/roll/jump/dodge when a direct replay attribute or validated reconstruction supports it.
- **Boost pad / pickup state:** Extract pickup actors/attributes (`Attribute::Pickup`, `PickupNew`, `PickupInfo`, and related object names) so `ArenaState` can reflect boost pad availability before stepping RocketSim between replay frames.
- **Demolitions and respawns:** Extract `TAGame.Car_TA:ReplicatedDemolish` (`Attribute::Demolish` / `DemolishExtended`) and related car lifecycle state so demo/respawn periods do not get simulated as normal free car physics. Demo state should override stale car physics application.
- **Game mode and arena setup:** Prefer replay metadata/actor data that identifies game mode, map, mutators, or non-standard arena setup over hardcoded `GameMode::Soccar` when it affects collision, gravity, ball behavior, or spawn/boost layout.
- **Ball state beyond rigid body:** Preserve simulation-affecting ball attributes such as sleep/contact/reset state when available. Ball touch/team metadata can be useful for validation, but should not be prioritized over state needed to reproduce physics.
- **rlgym-tools/carball metadata parity:** Preserve typed replay/game/frame/player/team/car metadata that downstream tooling needs (`metadata`, `frame_metadata`, `derived_frame_metadata`, `gameplay_periods`, `players`, `teams`, `cars`). Treat per-frame signals such as `TAGame.Ball_TA:HitTeamNum` as fresh only on frames where the replay updates them; do not let accumulated stale values drive gameplay-period inference. Prefer live `Engine.TeamInfo:Score` fields over synthesized goal increments when both are present. Avoid implementing inverse controls or action inference unless direct replay attributes support it.

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
