//! Shared by the kernel scenarios: a synthetic material table (so this crate never names a block)
//! and helpers to build, measure and settle small worlds.
use crate::*;
use engine_core::material::{MovementClass as C, SIM_MATERIAL_ROWS, SimMaterial};
use glam::IVec3;

pub(crate) const AIR: u8 = 0;
pub(crate) const STONE: u8 = 1;
pub(crate) const SAND: u8 = 2;
pub(crate) const WATER: u8 = 3;
pub(crate) const SMOKE: u8 = 4;
pub(crate) const DIRT: u8 = 5;
pub(crate) const LAVA: u8 = 6;

pub(crate) fn table() -> [SimMaterial; SIM_MATERIAL_ROWS] {
    let row = |class, kg_m3: f32, flow| SimMaterial { class, density: (kg_m3 * 100.0) as u32, flow, ..SimMaterial::INERT };
    let mut t = [SimMaterial::INERT; SIM_MATERIAL_ROWS];
    t[AIR as usize] = row(C::Void, 1.2, 0);
    t[STONE as usize] = row(C::Solid, 2600.0, 0);
    t[SAND as usize] = row(C::Powder, 1700.0, 0);
    t[WATER as usize] = row(C::Liquid, 1000.0, 255);
    t[SMOKE as usize] = row(C::Gas, 0.6, 255);
    t[DIRT as usize] = row(C::Solid, 1650.0, 0); // a cohesive powder resolves to Solid
    t[LAVA as usize] = row(C::Liquid, 3100.0, 21);
    t
}

pub(crate) fn put(s: &mut BrickStore, p: IVec3, c: Cell) {
    let (chunk, local) = split_voxel(p);
    s.set(chunk, local, c).unwrap();
}

pub(crate) fn at(s: &BrickStore, p: IVec3) -> Cell {
    let (chunk, local) = split_voxel(p);
    s.get(chunk, local)
}

pub(crate) fn fill(s: &mut BrickStore, lo: IVec3, hi: IVec3, c: Cell) {
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                put(s, IVec3::new(x, y, z), c);
            }
        }
    }
}

pub(crate) fn water(volume: u8) -> Cell {
    Cell { volume, ..Cell::filled(WATER) }
}

/// A stone box over [lo, hi) with the inside [lo + 1, hi - 1) left empty and an open top.
pub(crate) fn tank(s: &mut BrickStore, lo: IVec3, hi: IVec3) {
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                let wall = x == lo.x || x == hi.x - 1 || z == lo.z || z == hi.z - 1 || y == lo.y;
                if wall {
                    put(s, IVec3::new(x, y, z), Cell::filled(STONE));
                }
            }
        }
    }
}

/// Sum of `volume` per material over a box.
pub(crate) fn total(s: &BrickStore, lo: IVec3, hi: IVec3, material: u8) -> u32 {
    let mut sum = 0;
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                let c = at(s, IVec3::new(x, y, z));
                if c.material == material {
                    sum += c.volume as u32;
                }
            }
        }
    }
    sum
}

/// Step until no brick is awake. Returns the ticks it took; panics past `limit`.
pub(crate) fn settle(s: &mut BrickStore, t: &[SimMaterial], seed: u32, limit: u32) -> u32 {
    for tick in 0..limit {
        step(s, t, seed, tick);
        if s.active_bricks() == 0 {
            return tick + 1;
        }
    }
    panic!("still moving after {limit} ticks");
}
