use super::{Cell, Command, IVec3, Region, Snapshot, backend};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditStatus {
    Applied,
    NoChange,
    Conflict,
    Protected,
}

#[derive(Clone, Copy, Debug)]
pub struct EditOutcome {
    pub sequence: u64,
    pub tick: u64,
    pub at: IVec3,
    pub before: Cell,
    pub after: Cell,
    pub status: EditStatus,
}

pub struct StepResult {
    pub snapshot: Snapshot,
    pub outcomes: Vec<EditOutcome>,
}

impl Region {
    /// Replace only the exact cell observed by the player.
    pub fn submit_replace(
        &mut self,
        player_tick: u64,
        at: IVec3,
        expected: Cell,
        replacement: Cell,
    ) -> Result<Command, String> {
        backend::validate(expected)?;
        self.enqueue(player_tick, at, replacement, Some(expected))
    }

    /// Consuming a resolved command releases one admission slot.
    pub fn take_outcome(&mut self) -> Option<EditOutcome> {
        if self.outcomes.front().is_some_and(|outcome| self.owned.contains_key(&outcome.sequence)) {
            return None;
        }
        self.outcomes.pop_front()
    }
}
