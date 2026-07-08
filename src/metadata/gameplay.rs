use super::{GameplayPeriod, ReplayFrameMetadata, ReplayGameMetadata};

pub(crate) fn infer_gameplay_periods(
    game_metadata: &ReplayGameMetadata,
    frame_metadata: &[ReplayFrameMetadata],
) -> Vec<GameplayPeriod> {
    let mut periods = Vec::new();
    let frame_count = frame_metadata.len();
    if frame_count == 0 {
        return periods;
    }

    let mut start_search_at = 0;
    for goal in &game_metadata.goals {
        let Ok(goal_frame) = usize::try_from(goal.frame) else {
            continue;
        };
        if goal_frame >= frame_count {
            continue;
        }

        let Some(start_frame) = find_start_frame(frame_metadata, start_search_at, goal_frame)
        else {
            continue;
        };
        let Some(first_hit_frame) = find_first_hit_frame(frame_metadata, start_frame, goal_frame)
        else {
            continue;
        };
        let end_frame = find_end_frame(frame_metadata, goal_frame, frame_count - 1);

        periods.push(GameplayPeriod {
            start_frame,
            end_frame,
            first_hit_frame,
            goal_frame: Some(goal_frame),
        });
        start_search_at = end_frame.saturating_add(1);
    }

    if start_search_at < frame_count.saturating_sub(20)
        && let Some(start_frame) =
            find_start_frame(frame_metadata, start_search_at, frame_count - 1)
        && let Some(first_hit_frame) =
            find_first_hit_frame(frame_metadata, start_frame, frame_count - 1)
    {
        periods.push(GameplayPeriod {
            start_frame,
            end_frame: frame_count - 1,
            first_hit_frame,
            goal_frame: None,
        });
    }

    periods
}

fn find_start_frame(
    frame_metadata: &[ReplayFrameMetadata],
    start_search_at: usize,
    end_search_at: usize,
) -> Option<usize> {
    (start_search_at..=end_search_at).find(|&idx| {
        frame_metadata[idx]
            .game_event
            .replicated_game_state_time_remaining
            == Some(0)
    })
}

fn find_first_hit_frame(
    frame_metadata: &[ReplayFrameMetadata],
    start_search_at: usize,
    end_search_at: usize,
) -> Option<usize> {
    (start_search_at..=end_search_at).find(|&idx| frame_metadata[idx].ball.hit_team_num.is_some())
}

fn find_end_frame(
    frame_metadata: &[ReplayFrameMetadata],
    start_search_at: usize,
    end_search_at: usize,
) -> usize {
    let bounded_end = end_search_at.min(start_search_at.saturating_add(500));
    for (idx, metadata) in frame_metadata
        .iter()
        .enumerate()
        .take(bounded_end + 1)
        .skip(start_search_at)
    {
        if metadata.ball.hit_team_num.is_none() {
            return idx.saturating_sub(1);
        }
    }
    bounded_end
}
