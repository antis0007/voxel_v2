//! Generator stages folded into a few groups for the segmented progress bar.
use super::{StageRow, StageState};

pub struct Group {
    pub name: &'static str,
    pub state: StageState,
    /// Finished share of the group's stages, counting partial work.
    pub fraction: f32,
}

/// Groups in first-seen order of the stage list.
pub fn collect(rows: &[StageRow]) -> Vec<Group> {
    let mut names: Vec<&'static str> = Vec::new();
    for r in rows {
        if !names.contains(&r.group) {
            names.push(r.group);
        }
    }
    names
        .into_iter()
        .map(|name| {
            let members: Vec<&StageRow> = rows.iter().filter(|r| r.group == name).collect();
            let all = |s| members.iter().all(|r| r.state == s);
            let state = if all(StageState::Done) {
                StageState::Done
            } else if all(StageState::Queued) {
                StageState::Queued
            } else {
                StageState::Running
            };
            let work: f32 = members
                .iter()
                .map(|r| match r.state {
                    StageState::Done => 1.0,
                    StageState::Running => r.fraction.unwrap_or(0.0),
                    StageState::Queued => 0.0,
                })
                .sum();
            Group { name, state, fraction: work / members.len().max(1) as f32 }
        })
        .collect()
}
