//! One-time recovery of saved V20 commitments, never authority for new work.
use super::*;

pub(in crate::world::farming::jobs) fn migrate(j: &mut State, checkpoint: u64) -> bool {
    let Some(c) = j.activity.as_ref().filter(|c| c.configuration == 0) else {
        return false;
    };
    let start = c.start;
    let end = c.end;
    let resting = matches!(c.kind, Kind::Rest { .. });
    let saved_work = j.work;
    if checkpoint >= start {
        // The old certificate represents elapsed state only through this saved
        // clock. Do not trust it for a single tick after loading.
        interrupt(j, checkpoint.min(end));
        j.work = j.work.max(saved_work);
        if let Some(batch) = &mut j.batch {
            batch.retain_progress(j.work);
        }
        if resting {
            j.routine.last = checkpoint;
        }
    } else {
        // A future-dated legacy record cannot justify movement or progress.
        j.activity = None;
    }
    j.path.clear();
    j.navigation_wait = false;
    j.last_walk = None;
    j.stalled = 0;
    j.motion_at = checkpoint;
    j.retry_at = checkpoint;
    true
}

#[cfg(test)]
mod tests;