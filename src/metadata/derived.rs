use rocketsim::Team;

use super::{FrameScore, GameplayPeriod, ReplayFrameMetadata, ReplayGameMetadata, ReplayGoal};
use crate::timing::FrameTiming;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoreboardInfo {
    pub game_timer_seconds: f32,
    pub kickoff_timer_seconds: f32,
    pub blue_score: i32,
    pub orange_score: i32,
    pub go_to_kickoff: bool,
    pub is_over: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DerivedFrameMetadata {
    pub scoreboard: Option<ScoreboardInfo>,
    pub gameplay_period: Option<usize>,
    pub episode_seconds_remaining: Option<f32>,
    pub next_scoring_team: Option<Team>,
    pub winning_team: Option<Team>,
}

pub(crate) fn derive_frame_metadata(
    frames: &[FrameTiming],
    frame_metadata: &[ReplayFrameMetadata],
    gameplay_periods: &[GameplayPeriod],
    game_metadata: &ReplayGameMetadata,
) -> Vec<DerivedFrameMetadata> {
    let winning_team = winning_team(game_metadata);
    let mut derived = vec![DerivedFrameMetadata::default(); frame_metadata.len()];
    let mut blue_score = game_metadata.team_0_score.unwrap_or(0);
    let mut orange_score = game_metadata.team_1_score.unwrap_or(0);
    if !game_metadata.goals.is_empty() {
        blue_score = 0;
        orange_score = 0;
    }

    for (period_idx, period) in gameplay_periods.iter().enumerate() {
        let end_time = frames.get(period.end_frame).map_or(0.0, |frame| frame.time);
        let next_scoring_team = period
            .goal_frame
            .and_then(|goal_frame| goal_for_frame(game_metadata, goal_frame))
            .map(|goal| {
                if goal.is_orange {
                    Team::Orange
                } else {
                    Team::Blue
                }
            });

        for frame_idx in period.start_frame..=period.end_frame.min(derived.len().saturating_sub(1))
        {
            let frame = &frames[frame_idx];
            let raw = &frame_metadata[frame_idx];
            let used_live_score =
                if let Some(score) = live_or_goal_score(raw.score, blue_score, orange_score) {
                    blue_score = score.0;
                    orange_score = score.1;
                    true
                } else {
                    false
                };

            let is_period_end = frame_idx == period.end_frame;
            let goal_scored = period.goal_frame == Some(frame_idx);
            if goal_scored && !used_live_score {
                if next_scoring_team == Some(Team::Blue) {
                    blue_score += 1;
                } else if next_scoring_team == Some(Team::Orange) {
                    orange_score += 1;
                }
            }

            derived[frame_idx] = DerivedFrameMetadata {
                scoreboard: Some(ScoreboardInfo {
                    game_timer_seconds: scoreboard_timer(frame_idx, frames, frame_metadata),
                    kickoff_timer_seconds: kickoff_timer(frame_idx, frames, frame_metadata),
                    blue_score,
                    orange_score,
                    go_to_kickoff: is_period_end && period_idx + 1 < gameplay_periods.len(),
                    is_over: is_period_end
                        && period_idx + 1 == gameplay_periods.len()
                        && blue_score != orange_score,
                }),
                gameplay_period: Some(period_idx),
                episode_seconds_remaining: Some((end_time - frame.time).max(0.0)),
                next_scoring_team,
                winning_team,
            };
        }
    }

    derived
}

#[allow(clippy::cast_precision_loss)]
fn scoreboard_timer(
    frame_idx: usize,
    frames: &[FrameTiming],
    frame_metadata: &[ReplayFrameMetadata],
) -> f32 {
    if frame_metadata[frame_idx].game_event.is_overtime == Some(true) {
        return f32::INFINITY;
    }
    let Some(seconds_remaining) = frame_metadata[frame_idx].game_event.seconds_remaining else {
        return 0.0;
    };
    let Some(first_hit_idx) = (0..=frame_idx)
        .rev()
        .find(|&idx| frame_metadata[idx].game_event.ball_has_been_hit == Some(false))
        .map(|idx| idx.saturating_add(1))
        .or(Some(0))
    else {
        return seconds_remaining as f32;
    };
    let elapsed = frames[frame_idx].time
        - frames
            .get(first_hit_idx)
            .map_or(frames[frame_idx].time, |f| f.time);
    (seconds_remaining as f32 - elapsed).max(0.0)
}

fn kickoff_timer(
    frame_idx: usize,
    frames: &[FrameTiming],
    frame_metadata: &[ReplayFrameMetadata],
) -> f32 {
    if frame_metadata[frame_idx].game_event.ball_has_been_hit == Some(true) {
        return 0.0;
    }
    let start_idx = (0..=frame_idx)
        .rev()
        .find(|&idx| frame_metadata[idx].game_event.ball_has_been_hit == Some(true))
        .map_or(0, |idx| idx.saturating_add(1));
    let elapsed = frames[frame_idx].time
        - frames
            .get(start_idx)
            .map_or(frames[frame_idx].time, |f| f.time);
    (5.0 - elapsed).clamp(0.0, 5.0)
}

fn live_or_goal_score(
    score: FrameScore,
    fallback_blue: i32,
    fallback_orange: i32,
) -> Option<(i32, i32)> {
    match (score.blue, score.orange) {
        (Some(blue), Some(orange)) => Some((blue, orange)),
        (Some(blue), None) => Some((blue, fallback_orange)),
        (None, Some(orange)) => Some((fallback_blue, orange)),
        (None, None) => None,
    }
}

fn winning_team(game_metadata: &ReplayGameMetadata) -> Option<Team> {
    if let (Some(blue_score), Some(orange_score)) =
        (game_metadata.team_0_score, game_metadata.team_1_score)
    {
        return match blue_score.cmp(&orange_score) {
            std::cmp::Ordering::Greater => Some(Team::Blue),
            std::cmp::Ordering::Less => Some(Team::Orange),
            std::cmp::Ordering::Equal => None,
        };
    }

    let mut goal_diff = 0;
    for goal in &game_metadata.goals {
        if goal.is_orange {
            goal_diff -= 1;
        } else {
            goal_diff += 1;
        }
    }
    match goal_diff.cmp(&0) {
        std::cmp::Ordering::Greater => Some(Team::Blue),
        std::cmp::Ordering::Less => Some(Team::Orange),
        std::cmp::Ordering::Equal => None,
    }
}

fn goal_for_frame(game_metadata: &ReplayGameMetadata, frame: usize) -> Option<&ReplayGoal> {
    game_metadata
        .goals
        .iter()
        .find(|goal| usize::try_from(goal.frame) == Ok(frame))
}
