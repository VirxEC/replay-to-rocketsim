//! Convert Rocket League replay bytes into replay-aligned `RocketSim` arena snapshots.
//!
//! The primary entrypoint is [`Converter::convert_bytes`], which parses replay network data with
//! `boxcars`, steps `RocketSim` at 120Hz according to replay frame timestamps, and returns one
//! [`rocketsim::ArenaState`] per replay network frame.
//!
//! # Timing model
//!
//! Rocket League replay network frames are nominally 30Hz, while `RocketSim` runs at 120Hz. The
//! fixed [`ROCKETSIM_TICKS_PER_REPLAY_FRAME`] ratio is exposed for callers that want nominal
//! frame-index math, but conversion itself aligns snapshots with replay timestamps via
//! [`replay_time_to_rocketsim_tick`]. Each [`FrameTiming::rocketsim_tick`] should match the
//! corresponding returned [`rocketsim::ArenaState::tick_count`].
//!
//! # Event streams
//!
//! [`ConversionOutput`] intentionally keeps two event streams separate:
//!
//! - [`ConversionOutput::arena_events`] contains `RocketSim` [`rocketsim::ArenaEvent`] values emitted
//!   while stepping simulated 120Hz ticks between replay timestamps. These events carry `RocketSim`
//!   tick numbers through [`FrameArenaEvent::tick`].
//! - [`ConversionOutput::frame_metadata`] contains [`ReplayFrameMetadata::events`], which are
//!   replay-authored events observed directly from replay network data. These events are aligned to
//!   replay frame/time fields, not sub-frame `RocketSim` ticks.
//!
//! Replay rigid-body updates are sparse. The converter accumulates actor state across frames, but
//! only fresh per-field physics values are merged into `RocketSim` on normal frame syncs so stale
//! replay fields do not overwrite RocketSim-simulated interpolation.

mod actor;
mod arena_config;
mod arena_sync;
mod attributes;
mod body;
mod controls;
mod converter;
mod error;
mod metadata;
mod phys;
mod timing;

pub use converter::{
    ConversionOutput, Converter, FrameArenaEvent, replay_bytes_to_rocketsim_states,
};
pub use error::ConvertError;
pub use metadata::{
    BoostPickupKind, BoostPickupSource, CarActionKind, CarLifecycleKind, DemoKind,
    DerivedFrameMetadata, FrameBallMetadata, FrameBoostPickupEvent, FrameCarActionEvent,
    FrameCarLifecycleEvent, FrameCarMetadata, FrameDemoEvent, FrameGameEvent, FrameReplayEvent,
    FrameRigidBodyMetadata, FrameScore, GameplayPeriod, PlayerMetadata, PlayerRemoteId,
    PlayerUniqueId, ReplayFrameMetadata, ReplayGameMetadata, ReplayGameMode, ReplayGoal,
    ReplayMutatorMetadata, ScoreboardInfo, TeamMetadata,
};
pub use rocketsim;
pub use timing::{
    FrameTiming, REPLAY_HZ, ROCKETSIM_HZ, ROCKETSIM_TICKS_PER_REPLAY_FRAME,
    replay_frame_to_rocketsim_tick, replay_time_to_rocketsim_tick,
};
