use boxcars::{HeaderProp, Replay};
use rocketsim::{
    Arena, ArenaConfig, ArenaEvent, ArenaState, CarBodyConfig, DemoMode, GameMode, MutatorConfig,
};

use crate::ConvertError;
use crate::actor::ActorTracker;
use crate::arena_config::replay_arena_config;
use crate::arena_sync::{arena_car_layout_changed, sync_arena_to_replay_state};
use crate::metadata::{
    DerivedFrameMetadata, FrameCarMetadata, GameplayPeriod, PlayerMetadata, ReplayFrameMetadata,
    ReplayGameMetadata, TeamMetadata, derive_frame_metadata, infer_gameplay_periods,
};
use crate::timing::{FrameTiming, REPLAY_HZ, ROCKETSIM_HZ, replay_time_to_rocketsim_tick};

/// Parse replay bytes and convert every network frame into a `RocketSim` arena snapshot.
///
/// Returned snapshots are aligned by replay timestamp, not by assuming a fixed frame-index
/// mapping. In nominal 30Hz replays this is usually four `RocketSim` ticks per replay frame, but
/// irregular replay timing is mapped with [`replay_time_to_rocketsim_tick`].
///
/// # Errors
///
/// Returns [`ConvertError`] if the replay cannot be parsed or required replay metadata/network data
/// is missing.
pub fn replay_bytes_to_rocketsim_states(bytes: &[u8]) -> Result<Vec<ArenaState>, ConvertError> {
    Converter::default()
        .convert_bytes(bytes)
        .map(|output| output.states)
}

/// Configurable replay-to-`RocketSim` converter.
#[derive(Debug, Clone)]
pub struct Converter {
    game_mode: GameMode,
    car_body: CarBodyConfig,
}

impl Default for Converter {
    fn default() -> Self {
        Self {
            game_mode: GameMode::Soccar,
            car_body: CarBodyConfig::OCTANE,
        }
    }
}

impl Converter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `RocketSim` game mode attached to returned [`ArenaState`] values.
    #[must_use]
    pub fn with_game_mode(mut self, game_mode: GameMode) -> Self {
        self.game_mode = game_mode;
        self
    }

    /// Set the fallback body config used when replay loadout data is missing or unknown.
    #[must_use]
    pub fn with_car_body(mut self, car_body: CarBodyConfig) -> Self {
        self.car_body = car_body;
        self
    }

    /// Parse replay bytes and convert them into replay-aligned arena snapshots and metadata.
    ///
    /// # Errors
    ///
    /// Returns [`ConvertError`] if parsing fails or required replay metadata/network data is missing.
    pub fn convert_bytes(&self, bytes: &[u8]) -> Result<ConversionOutput, ConvertError> {
        let replay = boxcars::ParserBuilder::new(bytes)
            .must_parse_network_data()
            .parse()?;
        self.convert_replay(&replay)
    }

    /// Convert parsed replay network frames into replay-aligned `RocketSim` arena snapshots.
    ///
    /// For each replay frame, `RocketSim` is stepped at 120Hz from the previous replay timestamp to
    /// `replay_time_to_rocketsim_tick(frame.time)`, then fresh replay-authored state is merged into
    /// the arena. This means [`FrameTiming::rocketsim_tick`] and [`ArenaState::tick_count`] follow
    /// replay timestamps rather than the nominal `replay_frame * 4` helper.
    ///
    /// # Errors
    ///
    /// Returns [`ConvertError`] if required replay metadata/network data is missing or actor state
    /// extraction fails.
    pub fn convert_replay(&self, replay: &Replay) -> Result<ConversionOutput, ConvertError> {
        let replay_version = replay_version(replay)?;
        let network_frames = replay
            .network_frames
            .as_ref()
            .ok_or(ConvertError::MissingNetworkFrames)?;

        let mut tracker = ActorTracker::new(&replay.objects, replay_version);
        let metadata = ReplayGameMetadata::from_replay(replay, replay_version);
        let arena_config = replay_arena_config(replay, replay_version)?.unwrap_or_else(|| {
            let mut config = ArenaConfig::new(self.game_mode);
            config.mutators = MutatorConfig::new(self.game_mode);
            config.mutators.demo_mode = DemoMode::Disabled;
            config
        });
        let mut arena = Arena::new_with_config(arena_config.clone());
        let boost_pad_configs = arena.get_all_boost_pad_configs();
        let mut arena_synced = false;
        let mut current_rocketsim_tick = 0;
        let mut states = Vec::with_capacity(network_frames.frames.len());
        let mut predicted_states = Vec::with_capacity(network_frames.frames.len());
        let mut prediction_valid = Vec::with_capacity(network_frames.frames.len());
        let mut arena_events = Vec::with_capacity(network_frames.frames.len());
        let mut frames = Vec::with_capacity(network_frames.frames.len());
        let mut frame_metadata = Vec::with_capacity(network_frames.frames.len());
        let mut cars_metadata = Vec::with_capacity(network_frames.frames.len());
        let mut previous_car_actor_ids: Option<Vec<i32>> = None;
        let mut previous_frame_was_kickoff = false;

        for (replay_frame, frame) in network_frames.frames.iter().enumerate() {
            tracker.begin_frame(frame.delta, replay_frame, frame.time);
            tracker.apply_deleted_actors(&frame.deleted_actors);
            tracker.apply_new_actors(&frame.new_actors, replay_frame, frame.time);
            tracker.apply_updated_actors(
                &frame.updated_actors,
                frame.delta,
                replay_frame,
                frame.time,
            )?;
            tracker.refresh_indices();
            let replay_frame_metadata = tracker.frame_metadata();
            // The kickoff flag can be absent on sparse initial frames. Until the replay
            // explicitly reports that the ball has been hit, treat the prediction as kickoff.
            let frame_is_kickoff = replay_frame_metadata.game_event.ball_has_been_hit != Some(true);

            let target_rocketsim_tick = replay_time_to_rocketsim_tick(frame.time);
            let mut replay_frame_arena_events = Vec::new();
            if arena_synced {
                for tick in current_rocketsim_tick..target_rocketsim_tick {
                    replay_frame_arena_events.extend(arena.step_tick().iter().copied().map(
                        |event| FrameArenaEvent {
                            tick: tick + 1,
                            event,
                        },
                    ));
                }
            }
            current_rocketsim_tick = target_rocketsim_tick;

            let ball = tracker.ball_state()?.unwrap_or_default();
            let cars = tracker.car_states(self.car_body)?;
            let replay_cars_metadata = tracker.car_metadata(self.car_body);
            let current_car_actor_ids = replay_cars_metadata
                .iter()
                .map(|car| car.car_actor_id)
                .collect::<Vec<_>>();
            let actor_layout_changed = previous_car_actor_ids
                .as_ref()
                .is_some_and(|previous| previous != &current_car_actor_ids);
            let arena_recreated = actor_layout_changed || arena_car_layout_changed(&arena, &cars);
            if arena_recreated {
                arena = Arena::new_with_config(arena_config.clone());
            }
            let boost_pads = tracker.boost_pad_states(&boost_pad_configs);
            let mut predicted_state = arena.get_arena_state();
            predicted_state.tick_count = current_rocketsim_tick;
            predicted_states.push(predicted_state);
            // RocketSim does not model replay kickoff freezing in this prediction path. Mark the
            // kickoff frames and the first post-kickoff prediction invalid rather than reporting
            // the expected stationary-car versus simulated-car divergence as a prediction error.
            prediction_valid.push(
                arena_synced
                    && !arena_recreated
                    && !frame_is_kickoff
                    && !previous_frame_was_kickoff,
            );
            sync_arena_to_replay_state(
                &mut arena,
                &ball,
                &cars,
                &boost_pads,
                replay_frame,
                arena_recreated,
            );
            arena_synced = true;

            let mut state = arena.get_arena_state();
            state.tick_count = current_rocketsim_tick;
            state.cars.truncate(cars.len());
            for ((state_info, _), replay_car) in state.cars.iter_mut().zip(cars.iter()) {
                *state_info = replay_car.info;
            }

            frames.push(FrameTiming {
                replay_frame,
                time: frame.time,
                delta: frame.delta,
                rocketsim_tick: state.tick_count,
            });
            frame_metadata.push(ReplayFrameMetadata {
                ball: ball.metadata,
                ..replay_frame_metadata
            });
            previous_car_actor_ids = Some(current_car_actor_ids);
            previous_frame_was_kickoff = frame_is_kickoff;
            cars_metadata.push(replay_cars_metadata);
            arena_events.push(replay_frame_arena_events);
            states.push(state);
        }

        let gameplay_periods = infer_gameplay_periods(&metadata, &frame_metadata);
        let derived_frame_metadata =
            derive_frame_metadata(&frames, &frame_metadata, &gameplay_periods, &metadata);
        let players = tracker.player_metadata();
        let teams = tracker.team_metadata();

        Ok(ConversionOutput {
            states,
            predicted_states,
            prediction_valid,
            arena_events,
            frames,
            metadata,
            frame_metadata,
            derived_frame_metadata,
            gameplay_periods,
            players,
            teams,
            cars: cars_metadata,
            arena_config,
            replay_hz: REPLAY_HZ,
            rocketsim_hz: ROCKETSIM_HZ,
        })
    }
}

/// Full conversion result with timing metadata.
#[derive(Debug, Clone)]
pub struct ConversionOutput {
    /// One replay-aligned arena snapshot per replay network frame, after replay state is merged.
    pub states: Vec<ArenaState>,
    /// The `RocketSim` prediction immediately before that frame's fresh replay state is merged.
    ///
    /// This is intended for diagnostics and model-fidelity tooling. The first frame can contain
    /// `RocketSim` defaults because there is no preceding replay state to predict from.
    pub predicted_states: Vec<ArenaState>,
    /// Whether each pre-sync prediction is comparable with the replay frame.
    ///
    /// Kickoff frames and the first frame after kickoff are marked invalid because `RocketSim` does
    /// not model the replay's stationary kickoff phase in the prediction path.
    pub prediction_valid: Vec<bool>,
    /// `RocketSim`-native events emitted while stepping from the previous replay timestamp to the
    /// corresponding entry in [`frames`](Self::frames).
    ///
    /// This vector is indexed by replay frame, but each [`FrameArenaEvent::tick`] is a 120Hz
    /// `RocketSim` tick. These are simulated events; replay-authored events observed directly in
    /// network data are stored in [`frame_metadata`](Self::frame_metadata).
    pub arena_events: Vec<Vec<FrameArenaEvent>>,
    /// Replay-frame timing and the `RocketSim` tick reached for each returned state.
    pub frames: Vec<FrameTiming>,
    pub metadata: ReplayGameMetadata,
    /// Replay-authored per-frame metadata and events observed in network data.
    ///
    /// Events in this stream are replay-frame aligned via their `frame`/`time` fields. They are not
    /// sub-frame `RocketSim` tick events; use [`arena_events`](Self::arena_events) for events emitted
    /// by `RocketSim` during 120Hz stepping.
    pub frame_metadata: Vec<ReplayFrameMetadata>,
    pub derived_frame_metadata: Vec<DerivedFrameMetadata>,
    pub gameplay_periods: Vec<GameplayPeriod>,
    pub players: Vec<PlayerMetadata>,
    pub teams: Vec<TeamMetadata>,
    pub cars: Vec<Vec<FrameCarMetadata>>,
    pub arena_config: ArenaConfig,
    pub replay_hz: u32,
    pub rocketsim_hz: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct FrameArenaEvent {
    /// The 120Hz `RocketSim` tick on which `event` was emitted.
    pub tick: u64,
    pub event: ArenaEvent,
}

fn replay_version(replay: &Replay) -> Result<i32, ConvertError> {
    replay
        .properties
        .iter()
        .find_map(|(key, value)| match (key.as_str(), value) {
            ("ReplayVersion", HeaderProp::Int(version)) => Some(*version),
            _ => None,
        })
        .or(replay.net_version)
        .ok_or(ConvertError::MissingReplayVersion)
}
