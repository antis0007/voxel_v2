//! Budgeted world writes for pastes, cuts and undo through the tool edit path.
//!
//! Every write is `effects::replace` plus `publish`, the same commit normal
//! building uses, so meshing, saves and region files see ordinary edits.
use super::Blueprint;
use crate::world::{BlockId, World, WorldSampleConfig, effects};
use engine_core::Budget;
use glam::IVec3;
use std::collections::HashSet;
use std::sync::Arc;

const BUDGET_STRIDE: usize = 32;
/// Frames a deferred cell is retried before it is skipped, so a saturated
/// wake queue slows a paste without wedging it.
const MAX_STALLS: u32 = 240;

/// Where a batch writes: residency admission and the dirty sets meshing reads.
pub struct Sink<'a> {
    pub prepare: &'a mut dyn FnMut(&mut World, IVec3) -> bool,
    pub blocks: &'a mut HashSet<IVec3>,
    pub chunks: &'a mut HashSet<IVec3>,
}

/// One committed cell change; undo writes `old` back where `new` still stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub p: IVec3,
    pub old: BlockId,
    pub new: BlockId,
}

enum Source {
    /// Pass 0 writes everything but attachments; pass 1 adds torches and
    /// liquids once their supports and walls exist.
    Paste { blueprint: Arc<Blueprint>, origin: IVec3, skip_air: bool, pass: u8, index: usize },
    /// Writes `new` only where `old` still stands.
    List { ops: Vec<Change>, index: usize },
}

enum Outcome {
    Written(Option<Change>),
    Deferred,
    Skipped,
}

pub struct Job {
    cfg: WorldSampleConfig,
    source: Source,
    record: Vec<Change>,
    total: usize,
    done: usize,
    stalls: u32,
    pub skipped: usize,
}

fn late(b: BlockId) -> bool {
    b.is_torch() || b.is_liquid()
}

impl Job {
    pub fn paste(world: &World, blueprint: Arc<Blueprint>, origin: IVec3, skip_air: bool) -> Self {
        let total = if skip_air { blueprint.solid_count() } else { blueprint.volume() };
        Self::new(world, Source::Paste { blueprint, origin, skip_air, pass: 0, index: 0 }, total)
    }

    /// Applies `ops` in order; each writes only where its `old` block remains.
    pub fn changes(world: &World, ops: Vec<Change>) -> Self {
        let total = ops.len();
        Self::new(world, Source::List { ops, index: 0 }, total)
    }

    fn new(world: &World, source: Source, total: usize) -> Self {
        Self { cfg: world.sample_config(), source, record: Vec::new(), total, done: 0, stalls: 0, skipped: 0 }
    }

    pub fn same_world(&self, world: &World) -> bool {
        self.cfg == world.sample_config()
    }

    pub fn progress(&self) -> (usize, usize) {
        (self.done.min(self.total), self.total)
    }

    /// Committed changes so far, in write order.
    pub fn record(&self) -> &[Change] {
        &self.record
    }

    pub fn into_record(self) -> Vec<Change> {
        self.record
    }

    /// Next cell and its wanted block, without consuming it.
    fn peek(&mut self) -> Option<(IVec3, Option<BlockId>, BlockId)> {
        match &mut self.source {
            Source::List { ops, index } => ops.get(*index).map(|c| (c.p, Some(c.old), c.new)),
            Source::Paste { blueprint, origin, skip_air, pass, index } => loop {
                if *index >= blueprint.volume() {
                    if *pass == 1 {
                        return None;
                    }
                    *pass = 1;
                    *index = 0;
                }
                let b = blueprint.cells()[*index];
                if late(b) != (*pass == 1) || (*skip_air && b == BlockId::Air) {
                    *index += 1;
                    continue;
                }
                return Some((*origin + blueprint.local(*index), None, b));
            },
        }
    }

    fn advance(&mut self) {
        match &mut self.source {
            Source::List { index, .. } | Source::Paste { index, .. } => *index += 1,
        }
        self.done += 1;
        self.stalls = 0;
    }

    /// Writes up to `max_cells` cells within `budget`; true once nothing is left.
    pub fn step(&mut self, world: &mut World, sink: &mut Sink, budget: Budget, max_cells: usize) -> bool {
        let mut visited = 0;
        while visited < max_cells {
            if visited % BUDGET_STRIDE == 0 && visited > 0 && !budget.has_time() {
                return false;
            }
            let Some((p, expected, wanted)) = self.peek() else { return true };
            match write(world, sink, p, expected, wanted) {
                Outcome::Written(change) => self.record.extend(change),
                Outcome::Skipped => self.skipped += 1,
                Outcome::Deferred => {
                    self.stalls += 1;
                    if self.stalls < MAX_STALLS {
                        return false;
                    }
                    self.skipped += 1;
                }
            }
            self.advance();
            visited += 1;
        }
        self.peek().is_none()
    }
}

fn write(world: &mut World, sink: &mut Sink, p: IVec3, expected: Option<BlockId>, wanted: BlockId) -> Outcome {
    if !(sink.prepare)(world, p) {
        return Outcome::Skipped;
    }
    let old = world.get(p);
    if expected.is_some_and(|e| e != old) {
        return Outcome::Skipped;
    }
    if old == wanted {
        return Outcome::Written(None);
    }
    match effects::replace(world, p, old, wanted) {
        Ok(commit) => {
            commit.publish(world, sink.blocks, sink.chunks);
            Outcome::Written(Some(Change { p, old, new: wanted }))
        }
        Err(effects::Failure::Deferred) => Outcome::Deferred,
        Err(_) => Outcome::Skipped,
    }
}
