//! The M1 exit scenario (docs/MASTER_PLAN.md §3): a walled reservoir held by a one-cell dam that
//! an action log removes at a fixed tick, emptying into a basin twice its length.
//! The caller supplies the material indices, so this crate names no block.
use crate::cell::{Cell, split_voxel};
use crate::kernel::{StepStats, step};
use crate::store::{BrickStore, PoolFull};
use engine_core::reaction::SimRules;
use glam::IVec3;
use std::ops::Range;

/// Interior of the `dam_break_200k` reservoir: 64 × 50 × 64 = 204,800 liquid cells.
pub const DAM_BREAK_200K: IVec3 = IVec3::new(64, 50, 64);

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
fn fill(store: &mut BrickStore, lo: IVec3, hi: IVec3, cell: Cell) -> Result<(), PoolFull> {
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
/// actions sharing a tick apply in log order.
pub fn run_log(
    store: &mut BrickStore,
    rules: &SimRules,
    seed: u32,
    log: &[Action],
    ticks: Range<u32>,
) -> Result<Vec<StepStats>, PoolFull> {
    let mut stats = Vec::with_capacity(ticks.len());
    for tick in ticks {
        for action in log.iter().filter(|a| a.tick == tick) {
            action.apply(store)?;
        }
        stats.push(step(store, rules, seed, tick));
    }
    Ok(stats)
}

#[derive(Clone, Copy, Debug)]
pub struct DamBreak {
    /// Liquid-filled interior of the reservoir, cells per axis. Its near corner is the origin.
    pub reservoir: IVec3,
    pub wall: u8,
    pub liquid: u8,
    /// The tick whose step first sees the dam gone.
    pub release_tick: u32,
}

impl DamBreak {
    /// The dam's x; the basin runs from here to `basin_end`.
    fn dam_x(&self) -> i32 {
        self.reservoir.x
    }

    fn basin_end(&self) -> i32 {
        self.dam_x() + 1 + 2 * self.reservoir.x
    }

    /// The enclosure including its walls, half-open.
    pub fn bounds(&self) -> (IVec3, IVec3) {
        let r = self.reservoir;
        (IVec3::new(-1, 0, -1), IVec3::new(self.basin_end() + 1, r.y + 3, r.z + 1))
    }

    /// Bricks covering `bounds`: a pool this size never runs out, because nothing leaves the walls.
    pub fn capacity(&self) -> u32 {
        let (lo, hi) = self.bounds();
        let brick = |v: i32| v.div_euclid(crate::cell::BRICK_SIZE);
        let span = |a: i32, b: i32| (brick(b - 1) - brick(a) + 1) as u32;
        span(lo.x, hi.x) * span(lo.y, hi.y) * span(lo.z, hi.z)
    }

    /// Liquid cells at the start, each full.
    pub fn liquid_cells(&self) -> u32 {
        (self.reservoir.x * self.reservoir.y * self.reservoir.z) as u32
    }

    /// Floor, walls, dam and a full reservoir. The basin starts empty.
    pub fn build(&self, store: &mut BrickStore) -> Result<(), PoolFull> {
        let (lo, hi) = self.bounds();
        let r = self.reservoir;
        let wall = Cell::filled(self.wall);
        fill(store, lo, IVec3::new(hi.x, 1, hi.z), wall)?;
        fill(store, IVec3::new(lo.x, 1, lo.z), IVec3::new(hi.x, hi.y, 0), wall)?;
        fill(store, IVec3::new(lo.x, 1, r.z), IVec3::new(hi.x, hi.y, hi.z), wall)?;
        fill(store, IVec3::new(lo.x, 1, 0), IVec3::new(0, hi.y, r.z), wall)?;
        fill(store, IVec3::new(self.basin_end(), 1, 0), IVec3::new(hi.x, hi.y, r.z), wall)?;
        fill(store, IVec3::new(self.dam_x(), 1, 0), IVec3::new(self.dam_x() + 1, hi.y, r.z), wall)?;
        fill(store, IVec3::new(0, 1, 0), IVec3::new(r.x, 1 + r.y, r.z), Cell::filled(self.liquid))
    }

    /// The action log: one edit that clears the dam.
    pub fn actions(&self) -> Vec<Action> {
        let (_, hi) = self.bounds();
        vec![Action {
            tick: self.release_tick,
            lo: IVec3::new(self.dam_x(), 1, 0),
            hi: IVec3::new(self.dam_x() + 1, hi.y, self.reservoir.z),
            cell: Cell::EMPTY,
        }]
    }

    /// Half-open box of the basin floor layer, downstream of the dam.
    pub fn basin(&self) -> (IVec3, IVec3) {
        let (_, hi) = self.bounds();
        (IVec3::new(self.dam_x() + 1, 1, 0), IVec3::new(self.basin_end(), hi.y, self.reservoir.z))
    }
}
