//! The M1 exit scenario (docs/MASTER_PLAN.md §3): a walled reservoir held by a one-cell dam that
//! an action log removes at a fixed tick, emptying into a basin twice its length.
//! The caller supplies the material indices, so this crate names no block.
use crate::cell::{BRICK_SIZE, Cell};
use crate::replay::{Action, fill};
use crate::store::{BrickStore, PoolFull};
use glam::IVec3;

/// Interior of the `dam_break_200k` reservoir: 64 × 50 × 64 = 204,800 liquid cells.
pub const DAM_BREAK_200K: IVec3 = IVec3::new(64, 50, 64);

#[derive(Clone, Copy, Debug)]
pub struct DamBreak {
    /// Near bottom corner of the reservoir interior, in cells. The floor is the layer below it.
    pub origin: IVec3,
    /// Liquid-filled interior of the reservoir, cells per axis.
    pub reservoir: IVec3,
    pub wall: u8,
    pub liquid: u8,
    /// The tick whose step first sees the dam gone.
    pub release_tick: u32,
}

impl DamBreak {
    /// Scenario-local box `lo..hi` in world cells.
    fn at(&self, lo: IVec3, hi: IVec3) -> (IVec3, IVec3) {
        (self.origin + lo, self.origin + hi)
    }

    /// Local x of the dam; the basin runs from just past it to `basin_end`.
    fn dam_x(&self) -> i32 {
        self.reservoir.x
    }

    fn basin_end(&self) -> i32 {
        self.dam_x() + 1 + 2 * self.reservoir.x
    }

    /// Local enclosure including floor and walls, half-open.
    fn local_bounds(&self) -> (IVec3, IVec3) {
        let r = self.reservoir;
        (IVec3::new(-1, -1, -1), IVec3::new(self.basin_end() + 1, r.y + 2, r.z + 1))
    }

    /// The enclosure including its floor and walls, half-open, in world cells.
    pub fn bounds(&self) -> (IVec3, IVec3) {
        let (lo, hi) = self.local_bounds();
        self.at(lo, hi)
    }

    /// Bricks covering `bounds`: a pool this size never runs out, because nothing leaves the walls.
    pub fn capacity(&self) -> u32 {
        let (lo, hi) = self.bounds();
        let brick = |v: i32| v.div_euclid(BRICK_SIZE);
        let span = |a: i32, b: i32| (brick(b - 1) - brick(a) + 1) as u32;
        span(lo.x, hi.x) * span(lo.y, hi.y) * span(lo.z, hi.z)
    }

    /// Liquid cells at the start, each full.
    pub fn liquid_cells(&self) -> u32 {
        (self.reservoir.x * self.reservoir.y * self.reservoir.z) as u32
    }

    /// Floor, walls, dam and a full reservoir. The basin starts empty.
    pub fn build(&self, store: &mut BrickStore) -> Result<(), PoolFull> {
        let (lo, hi) = self.local_bounds();
        let r = self.reservoir;
        let wall = Cell::filled(self.wall);
        let boxes = [
            (lo, IVec3::new(hi.x, 0, hi.z)),                                    // floor
            (IVec3::new(lo.x, 0, lo.z), IVec3::new(hi.x, hi.y, 0)),             // near side
            (IVec3::new(lo.x, 0, r.z), hi),                                     // far side
            (IVec3::new(lo.x, 0, 0), IVec3::new(0, hi.y, r.z)),                 // back
            (IVec3::new(self.basin_end(), 0, 0), IVec3::new(hi.x, hi.y, r.z)),  // basin end
            (IVec3::new(self.dam_x(), 0, 0), IVec3::new(self.dam_x() + 1, hi.y, r.z)), // dam
        ];
        for (a, b) in boxes {
            let (a, b) = self.at(a, b);
            fill(store, a, b, wall)?;
        }
        let (a, b) = self.at(IVec3::ZERO, r);
        fill(store, a, b, Cell::filled(self.liquid))
    }

    /// The action log: one edit that clears the dam.
    pub fn actions(&self) -> Vec<Action> {
        let (_, hi) = self.local_bounds();
        let (lo, hi) = self.at(IVec3::new(self.dam_x(), 0, 0), IVec3::new(self.dam_x() + 1, hi.y, self.reservoir.z));
        vec![Action { tick: self.release_tick, lo, hi, cell: Cell::EMPTY }]
    }

    /// The basin's whole volume downstream of the dam, half-open, in world cells.
    pub fn basin(&self) -> (IVec3, IVec3) {
        let (_, hi) = self.local_bounds();
        self.at(IVec3::new(self.dam_x() + 1, 0, 0), IVec3::new(self.basin_end(), hi.y, self.reservoir.z))
    }
}
