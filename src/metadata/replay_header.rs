use boxcars::{HeaderProp, Replay};

use super::{ReplayGameMetadata, ReplayGoal};

impl ReplayGameMetadata {
    pub(crate) fn from_replay(replay: &Replay, replay_version: i32) -> Self {
        Self {
            id: header_str(replay, "Id"),
            replay_version,
            num_frames: header_int(replay, "NumFrames"),
            replay_name: header_str(replay, "ReplayName"),
            map_name: header_str(replay, "MapName"),
            date: header_str(replay, "Date"),
            match_type: header_name_or_str(replay, "MatchType"),
            team_0_score: header_int(replay, "Team0Score"),
            team_1_score: header_int(replay, "Team1Score"),
            goals: replay_goals(replay),
        }
    }
}

fn header_prop<'a>(replay: &'a Replay, key: &str) -> Option<&'a HeaderProp> {
    replay
        .properties
        .iter()
        .find_map(|(prop_key, value)| (prop_key == key).then_some(value))
}

fn header_str(replay: &Replay, key: &str) -> Option<String> {
    match header_prop(replay, key) {
        Some(HeaderProp::Str(value) | HeaderProp::Name(value)) => Some(value.clone()),
        _ => None,
    }
}

fn header_name_or_str(replay: &Replay, key: &str) -> Option<String> {
    header_str(replay, key)
}

fn header_int(replay: &Replay, key: &str) -> Option<i32> {
    match header_prop(replay, key) {
        Some(HeaderProp::Int(value)) => Some(*value),
        _ => None,
    }
}

fn replay_goals(replay: &Replay) -> Vec<ReplayGoal> {
    let Some(HeaderProp::Array(goals)) = header_prop(replay, "Goals") else {
        return Vec::new();
    };
    goals
        .iter()
        .filter_map(|goal_fields| {
            let frame = goal_field_int(goal_fields, "frame")?;
            let player_name = goal_field_str(goal_fields, "PlayerName");
            let player_team = goal_field_int(goal_fields, "PlayerTeam").unwrap_or(0);
            Some(ReplayGoal {
                frame,
                player_name,
                is_orange: player_team == 1,
            })
        })
        .collect()
}

fn goal_field_int(fields: &[(String, HeaderProp)], key: &str) -> Option<i32> {
    fields
        .iter()
        .find_map(|(field_key, value)| match (field_key.as_str(), value) {
            (field, HeaderProp::Int(value)) if field == key => Some(*value),
            _ => None,
        })
}

fn goal_field_str(fields: &[(String, HeaderProp)], key: &str) -> Option<String> {
    fields
        .iter()
        .find_map(|(field_key, value)| match (field_key.as_str(), value) {
            (field, HeaderProp::Str(value) | HeaderProp::Name(value)) if field == key => {
                Some(value.clone())
            }
            _ => None,
        })
}
