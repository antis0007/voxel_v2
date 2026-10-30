//! Tick-stamped edits, replaying them into a store, and the two saved forms a replay needs: the
//! action log and a run checkpoint (tick, seed and store) to resume from.
use crate::cell::{Cell, split_voxel};
use crate::codec::{CheckpointError, Reader, put_cell, put_coord, put_u32};
use crate::kernel::{StepStats, step};
use crate::store::{BrickStore, PoolFull};
use engine_core::reaction::SimRules;
use glam::IVec3;
use std::ops::Range;

const LOG_MAGIC: &[u8; 8] = b"VXSIMLG1";
const RUN_MAGIC: &[u8; 8] = b"VXSIMRN1";
/// Bytes per logged action: tick, two corners, one cell.
const ACTION_BYTES: usize = 4 + 12 + 12 + size_of::<Cell>();
/// Largest box one action may cover, 2^24 cells: a corrupt log cannot ask for an endless fill.
const MAX_ACTION_CELLS: i64 = 1 << 24;

/// One logged edit: every cell in `lo..hi` becomes `cell` before tick `tick` steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    pub tick: u32,
    pub lo: IVec3,
    pub hi: IVec3,
    pub cell: Cell,
}

impl Action {
    pub fn apply(&self, store: &mut BrickStore) -> Result<(), PoolFull> {
        fill(store, self.lo, self.hi, self.cell)
    }
}

/// Write `cell` over the half-open box `lo..hi`.
pub(crate) fn fill(store: &mut BrickStore, lo: IVec3, hi: IVec3, cell: Cell) -> Result<(), PoolFull> {
    for z in lo.z..hi.z {
        for y in lo.y..hi.y {
            for x in lo.x..hi.x {
                let (chunk, local) = split_voxel(IVec3::new(x, y, z));
                store.set(chunk, local, cell)?;
            }
        }
    }
    Ok(())
}

/// Step `ticks`, applying each logged action at the start of its tick. The log need not be sorted;
/// actions sharing a tick apply in log order. Actions outside `ticks` are ignored.
pub fn run_log(
    store: &mut BrickStore,
    rules: &SimRules,
    seed: u32,
    log: &[Action],
    ticks: Range<u32>,
) -> Result<Vec<StepStats>, PoolFull> {
    let mut order: Vec<&Action> = log.iter().filter(|a| ticks.contains(&a.tick)).collect();
    order.sort_by_key(|a| a.tick); // stable: same-tick actions keep log order
    let mut next = order.into_iter().peekable();
    let mut stats = Vec::with_capacity(ticks.len());
    for tick in ticks {
        while let Some(action) = next.next_if(|a| a.tick == tick) {
            action.apply(store)?;
        }
        stats.push(step(store, rules, seed, tick));
    }
    Ok(stats)
}

/// The log as bytes, actions in the order given.
pub fn encode_log(log: &[Action]) -> Vec<u8> {
    let mut out = Vec::with_capacity(LOG_MAGIC.len() + 4 + log.len() * ACTION_BYTES);
    out.extend_from_slice(LOG_MAGIC);
    put_u32(&mut out, log.len() as u32);
    for a in log {
        put_u32(&mut out, a.tick);
        put_coord(&mut out, a.lo);
        put_coord(&mut out, a.hi);
        put_cell(&mut out, a.cell);
    }
    out
}

/// A log from `encode_log` bytes. Refuses inverted or oversized boxes.
pub fn decode_log(bytes: &[u8]) -> Result<Vec<Action>, CheckpointError> {
    let mut r = Reader::open(bytes, LOG_MAGIC)?;
    let n = r.count(ACTION_BYTES)?;
    let mut log = Vec::with_capacity(n);
    for _ in 0..n {
        let a = Action { tick: r.u32()?, lo: r.coord()?, hi: r.coord()?, cell: r.cell()? };
        let size = a.hi.as_i64vec3() - a.lo.as_i64vec3();
        if size.cmplt(glam::I64Vec3::ZERO).any() {
            return Err(CheckpointError::Inconsistent("action box is inverted"));
        }
        if size.x.saturating_mul(size.y).saturating_mul(size.z) > MAX_ACTION_CELLS {
            return Err(CheckpointError::Inconsistent("action box is too large"));
        }
        log.push(a);
    }
    r.finish()?;
    Ok(log)
}

/// Everything a replay needs to resume besides the log and the rules: the next tick to step, the
/// world seed and the store.
#[derive(Clone)]
pub struct RunCheckpoint {
    pub tick: u32,
    pub seed: u32,
    pub store: BrickStore,
}

impl RunCheckpoint {
    pub fn encode(&self) -> Vec<u8> {
        let store = self.store.encode_checkpoint();
        let mut out = Vec::with_capacity(RUN_MAGIC.len() + 16 + store.len());
        out.extend_from_slice(RUN_MAGIC);
        put_u32(&mut out, self.tick);
        put_u32(&mut out, self.seed);
        out.extend_from_slice(&(store.len() as u64).to_le_bytes());
        out.extend_from_slice(&store);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CheckpointError> {
        let mut r = Reader::open(bytes, RUN_MAGIC)?;
        let (tick, seed) = (r.u32()?, r.u32()?);
        let len = usize::try_from(r.u64()?).map_err(|_| CheckpointError::Truncated)?;
        let store = BrickStore::decode_checkpoint(r.take(len)?)?;
        r.finish()?;
        Ok(Self { tick, seed, store })
    }

    /// Step from `self.tick` up to `end`, applying the log, and advance `tick`.
    pub fn resume(&mut self, rules: &SimRules, log: &[Action], end: u32) -> Result<Vec<StepStats>, PoolFull> {
        let stats = run_log(&mut self.store, rules, self.seed, log, self.tick..end.max(self.tick))?;
        self.tick = end.max(self.tick);
        Ok(stats)
    }
}
