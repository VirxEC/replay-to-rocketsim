/// Rocket League replay network frames are recorded at 30Hz.
pub const REPLAY_HZ: u32 = 30;

/// `RocketSim` physics ticks run at 120Hz.
pub const ROCKETSIM_HZ: u32 = 120;

/// Number of `RocketSim` ticks represented by one replay network frame.
pub const ROCKETSIM_TICKS_PER_REPLAY_FRAME: u32 = ROCKETSIM_HZ / REPLAY_HZ;

/// Timing metadata for one replay network frame and its corresponding `RocketSim` tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameTiming {
    pub replay_frame: usize,
    pub time: f32,
    pub delta: f32,
    pub rocketsim_tick: u64,
}

/// Nominal 30Hz replay-frame to 120Hz `RocketSim` tick mapping.
///
/// Conversion uses replay timestamps when available; this helper remains useful for callers that
/// intentionally want the fixed-rate frame-index mapping.
#[must_use]
pub fn replay_frame_to_rocketsim_tick(replay_frame: usize) -> u64 {
    replay_frame as u64 * u64::from(ROCKETSIM_TICKS_PER_REPLAY_FRAME)
}

#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub fn replay_time_to_rocketsim_tick(time_seconds: f32) -> u64 {
    (time_seconds * ROCKETSIM_HZ as f32).round().max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_nominal_replay_frames_to_120hz_ticks() {
        assert_eq!(replay_frame_to_rocketsim_tick(0), 0);
        assert_eq!(replay_frame_to_rocketsim_tick(1), 4);
        assert_eq!(replay_frame_to_rocketsim_tick(30), 120);
    }

    #[test]
    fn maps_replay_time_to_120hz_ticks() {
        assert_eq!(replay_time_to_rocketsim_tick(0.0), 0);
        assert_eq!(replay_time_to_rocketsim_tick(1.0 / 60.0), 2);
        assert_eq!(replay_time_to_rocketsim_tick(1.0), 120);
    }

    #[test]
    fn timestamp_mapping_can_differ_from_nominal_frame_index_mapping() {
        assert_eq!(replay_frame_to_rocketsim_tick(1), 4);
        assert_eq!(replay_time_to_rocketsim_tick(0.050), 6);
    }
}
