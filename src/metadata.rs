mod derived;
mod gameplay;
mod replay_header;
mod types;

pub(crate) use derived::derive_frame_metadata;
pub use derived::{DerivedFrameMetadata, ScoreboardInfo};
pub(crate) use gameplay::infer_gameplay_periods;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timing::FrameTiming;

    #[test]
    fn infers_gameplay_period_from_kickoff_hit_and_goal() {
        let metadata = ReplayGameMetadata {
            id: None,
            replay_version: 1,
            num_frames: None,
            replay_name: None,
            map_name: None,
            date: None,
            match_type: None,
            team_0_score: Some(0),
            team_1_score: Some(0),
            goals: vec![ReplayGoal {
                frame: 4,
                player_name: None,
                is_orange: false,
            }],
        };
        let frames = vec![
            frame_metadata(Some(0), None, Some(false), None),
            frame_metadata(Some(0), Some(0), Some(false), None),
            frame_metadata(Some(0), Some(1), Some(true), Some(0)),
            frame_metadata(Some(0), Some(2), Some(true), Some(0)),
            frame_metadata(Some(0), Some(3), Some(true), Some(0)),
            frame_metadata(Some(0), Some(4), Some(true), None),
        ];

        let periods = infer_gameplay_periods(&metadata, &frames);

        assert_eq!(
            periods,
            vec![GameplayPeriod {
                start_frame: 1,
                end_frame: 4,
                first_hit_frame: 2,
                goal_frame: Some(4),
            }]
        );
    }

    #[test]
    fn derived_scoreboard_uses_live_score_instead_of_double_counting_goal() {
        let metadata = ReplayGameMetadata {
            id: None,
            replay_version: 1,
            num_frames: None,
            replay_name: None,
            map_name: None,
            date: None,
            match_type: None,
            team_0_score: Some(1),
            team_1_score: Some(0),
            goals: vec![ReplayGoal {
                frame: 2,
                player_name: None,
                is_orange: false,
            }],
        };
        let frames = vec![
            FrameTiming {
                replay_frame: 0,
                time: 0.0,
                delta: 0.0,
                rocketsim_tick: 0,
            },
            FrameTiming {
                replay_frame: 1,
                time: 1.0,
                delta: 1.0,
                rocketsim_tick: 120,
            },
            FrameTiming {
                replay_frame: 2,
                time: 2.0,
                delta: 1.0,
                rocketsim_tick: 240,
            },
        ];
        let mut raw = vec![
            frame_metadata(Some(300), Some(0), Some(true), Some(0)),
            frame_metadata(Some(299), Some(1), Some(true), Some(0)),
            frame_metadata(Some(298), Some(2), Some(true), Some(0)),
        ];
        raw[2].score.blue = Some(1);
        raw[2].score.orange = Some(0);
        let periods = vec![GameplayPeriod {
            start_frame: 0,
            end_frame: 2,
            first_hit_frame: 0,
            goal_frame: Some(2),
        }];

        let derived = derive_frame_metadata(&frames, &raw, &periods, &metadata);

        assert_eq!(derived[2].scoreboard.unwrap().blue_score, 1);
        assert_eq!(derived[2].scoreboard.unwrap().orange_score, 0);
    }

    fn frame_metadata(
        seconds_remaining: Option<i32>,
        game_state_time: Option<i32>,
        ball_has_been_hit: Option<bool>,
        hit_team_num: Option<u8>,
    ) -> ReplayFrameMetadata {
        ReplayFrameMetadata {
            game_event: FrameGameEvent {
                seconds_remaining,
                replicated_game_state_time_remaining: game_state_time,
                is_overtime: Some(false),
                ball_has_been_hit,
                ..FrameGameEvent::default()
            },
            ball: FrameBallMetadata {
                hit_team_num,
                ..FrameBallMetadata::default()
            },
            score: FrameScore::default(),
            mutators: ReplayMutatorMetadata::default(),
            events: Vec::new(),
        }
    }
}
