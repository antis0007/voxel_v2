//! Per-player undo and redo of committed creative edits.
//!
//! Both stacks hold changes as they were applied, so undo and redo are one
//! operation in opposite directions: invert the newest entry, write it through
//! `blueprint::Job`, and keep what was actually written on the other stack.
//! `Job` writes a cell only where the recorded block still stands, so undo never
//! overwrites a later edit by another player, a machine or the simulation.
use super::blueprint::{Change, Job, Sink};
use super::{BlockId, World, WorldSampleConfig, factory};
use engine_core::Budget;
use glam::IVec3;
use std::collections::VecDeque;

#[cfg(test)]
mod tests;

/// Undo steps kept per history.
pub const MAX_ENTRIES: usize = 1024;
/// Recorded cells for a local player: one 16-byte change each, 64 MiB at most.
pub const LOCAL_CELLS: usize = 1 << 22;
/// Recorded cells per remote player, which the host holds for every joiner.
#[cfg_attr(not(feature = "multiplayer"), allow(dead_code))]
pub const REMOTE_CELLS: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    Undo,
    Redo,
}

struct Entry {
    cfg: WorldSampleConfig,
    changes: Vec<Change>,
}

pub struct EditHistory {
    cells_cap: usize,
    undo: VecDeque<Entry>,
    redo: VecDeque<Entry>,
    /// Cells held by `undo` and `redo`.
    cells: usize,
    /// The stroke being recorded; `seal` makes it one undo step.
    open: Option<Entry>,
    overflow: bool,
    running: Option<(Direction, Job)>,
    /// An edit sealed while an undo runs makes that undo's redo step stale.
    stale_redo: bool,
}

/// Machines and fittings keep state beside the block, so a bare block write
/// cannot restore them.
pub fn undoable(block: BlockId) -> bool {
    factory::Kind::from_block(block).is_none() && engine_content::fittings::state(block).is_none()
}

fn invert(changes: Vec<Change>) -> Vec<Change> {
    changes.into_iter().rev().map(|c| Change { p: c.p, old: c.new, new: c.old }).collect()
}

impl EditHistory {
    pub fn new(cells_cap: usize) -> Self {
        Self {
            cells_cap,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            cells: 0,
            open: None,
            overflow: false,
            running: None,
            stale_redo: false,
        }
    }

    /// Undo and redo steps available.
    pub fn depth(&self) -> (usize, usize) {
        (self.undo.len() + usize::from(self.open.as_ref().is_some_and(|e| !e.changes.is_empty())), self.redo.len())
    }

    /// Adds one committed cell change to the open stroke.
    pub fn record(&mut self, world: &World, p: IVec3, old: BlockId, new: BlockId) {
        if old == new || !undoable(old) || !undoable(new) {
            return;
        }
        let cfg = world.sample_config();
        if self.open.as_ref().is_some_and(|e| e.cfg != cfg) {
            self.seal();
        }
        let entry = self.open.get_or_insert_with(|| Entry { cfg, changes: Vec::new() });
        if entry.changes.len() < self.cells_cap {
            entry.changes.push(Change { p, old, new });
        } else {
            self.overflow = true;
        }
    }

    /// Closes the open stroke as one undo step; false when it exceeded the cap.
    pub fn seal(&mut self) -> bool {
        let overflow = std::mem::take(&mut self.overflow);
        let Some(entry) = self.open.take() else { return true };
        if overflow {
            return false;
        }
        if entry.changes.is_empty() {
            return true;
        }
        self.redo_cleared();
        self.cells += entry.changes.len();
        self.undo.push_back(entry);
        self.trim();
        true
    }

    /// Keeps a finished batch, such as a paste, as one undo step.
    pub fn push(&mut self, world: &World, changes: Vec<Change>) -> bool {
        self.seal();
        if changes.len() > self.cells_cap {
            return false;
        }
        self.open = Some(Entry { cfg: world.sample_config(), changes });
        self.seal()
    }

    fn redo_cleared(&mut self) {
        self.cells -= self.redo.iter().map(|e| e.changes.len()).sum::<usize>();
        self.redo.clear();
        if matches!(self.running, Some((Direction::Undo, _))) {
            self.stale_redo = true;
        }
    }

    fn trim(&mut self) {
        while self.undo.len() > MAX_ENTRIES {
            self.drop_oldest();
        }
        while self.cells > self.cells_cap && (!self.undo.is_empty() || !self.redo.is_empty()) {
            self.drop_oldest();
        }
    }

    fn drop_oldest(&mut self) {
        if let Some(e) = self.undo.pop_front().or_else(|| self.redo.pop_front()) {
            self.cells -= e.changes.len();
        }
    }

    /// Starts reversing the newest undo or redo step for this world.
    pub fn start(&mut self, world: &World, direction: Direction) -> Result<(), &'static str> {
        if self.running.is_some() {
            return Err("Edit History Busy");
        }
        self.seal();
        let cfg = world.sample_config();
        for stack in [&mut self.undo, &mut self.redo] {
            let before: usize = stack.iter().map(|e| e.changes.len()).sum();
            stack.retain(|e| e.cfg == cfg);
            self.cells -= before - stack.iter().map(|e| e.changes.len()).sum::<usize>();
        }
        let source = match direction {
            Direction::Undo => &mut self.undo,
            Direction::Redo => &mut self.redo,
        };
        let Some(entry) = source.pop_back() else {
            return Err(match direction {
                Direction::Undo => "Undo History Empty",
                Direction::Redo => "Redo History Empty",
            });
        };
        self.cells -= entry.changes.len();
        self.stale_redo = false;
        self.running = Some((direction, Job::changes(world, invert(entry.changes))));
        Ok(())
    }

    /// Advances a running undo or redo; the status line once it finishes.
    pub fn step(&mut self, world: &mut World, sink: &mut Sink<'_>, budget: Budget, max_cells: usize) -> Option<String> {
        let (_, job) = self.running.as_mut()?;
        if !job.same_world(world) {
            self.running = None;
            return None;
        }
        if !job.step(world, sink, budget, max_cells) {
            return None;
        }
        let (direction, job) = self.running.take()?;
        let skipped = job.skipped;
        let changes = job.into_record();
        let count = changes.len();
        let entry = Entry { cfg: world.sample_config(), changes };
        let target = match direction {
            Direction::Undo if std::mem::take(&mut self.stale_redo) => None,
            Direction::Undo => Some(&mut self.redo),
            Direction::Redo => Some(&mut self.undo),
        };
        if let Some(stack) = target.filter(|_| count > 0) {
            self.cells += count;
            stack.push_back(entry);
            self.trim();
        }
        let verb = if direction == Direction::Undo { "Undone" } else { "Redone" };
        let mut text = format!("{verb}: {count} Blocks");
        if skipped > 0 {
            text += &format!(" · Skipped: {skipped}");
        }
        Some(text)
    }
}
