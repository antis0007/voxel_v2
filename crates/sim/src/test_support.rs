//! Shared by the kernel scenarios: a synthetic material table (so this crate never names a block)
//! and helpers to build, measure and settle small worlds.
use crate::*;
use engine_core::material::{MovementClass as C, SIM_MATERIAL_ROWS, SimMaterial};
use engine_core::reaction::{ReactionTable, SimRules};
use glam::IVec3;

pub(crate) const AIR: u8 = 0;
pub(crate) const STONE: u8 = 1;
pub(crate) const SAND: u8 = 2;
pub(crate) const WATER: u8 = 3;
pub(crate) const SMOKE: u8 = 4;
pub(crate) const DIRT: u8 = 5;
pub(crate) const LAVA: u8 = 6;
pub(crate) const FOAM: u8 = 7;
pub(crate) const HEAVY: u8 = 8;

/// The synthetic materials with no reactions.
pub(crate) fn table() -> SimRules {
    SimRules::new(materials(), ReactionTable::default())
}

pub(crate) fn materials() -> [SimMaterial; SIM_MATERIAL_ROWS] {
    // Thermal data as the real table packs it: conductivity in 0.01 W/(m K), specific heat in J/(kg K),
    // and a thermal mass per level of density x specific heat / 1024.
    let row = |class, kg_m3: f32, flow, k: f32, cp: u16| {
        let density = (kg_m3 * 100.0) as u32;
        SimMaterial {
            class,
            density,
            flow,
            conductivity: (k * 100.0).round() as u16,
            capacity: cp,
            thermal_mass: ((density as u64 * cp as u64) >> 10) as u32,
            ..SimMaterial::INERT
        }
    };
    let mut t = [SimMaterial::INERT; SIM_MATERIAL_ROWS];
    t[AIR as usize] = row(C::Void, 1.2, 0, 0.026, 1005);
    t[STONE as usize] = row(C::Solid, 2600.0, 0, 2.5, 790);
    t[SAND as usize] = row(C::Powder, 1700.0, 0, 0.3, 830);
    t[WATER as usize] = row(C::Liquid, 1000.0, 255, 0.6, 4186);
    t[SMOKE as usize] = row(C::Gas, 0.6, 255, 0.03, 1000);
    t[DIRT as usize] = row(C::Solid, 1650.0, 0, 1.0, 1200); // a cohesive powder resolves to Solid
    t[LAVA as usize] = row(C::Liquid, 3100.0, 21, 1.5, 1000);
    t[FOAM as usize] = row(C::Solid, 30.0, 0, 0.0, 1500); // an insulator: conductivity 0
    t[HEAVY as usize] = row(C::Solid, 8000.0, 0, 2.5, 4000); // stone's conductivity, 15x its heat capacity
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
pub(crate) fn settle(s: &mut BrickStore, t: &SimRules, seed: u32, limit: u32) -> u32 {
    for tick in 0..limit {
        step(s, t, seed, tick);
        if s.active_bricks() == 0 {
            return tick + 1;
        }
    }
    panic!("still moving after {limit} ticks");
}

/// Total heat energy over a box: every cell's heat capacity times its temperature.
pub(crate) fn energy(s: &BrickStore, t: &SimRules, lo: IVec3, hi: IVec3) -> u128 {
    let t = t.materials();
    let mut sum = 0u128;
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                let c = at(s, IVec3::new(x, y, z));
                // Empty space is the environment and holds no counted heat.
                sum += c.volume as u128 * t[c.material as usize].thermal_mass as u128 * c.temperature as u128;
            }
        }
    }
    sum
}

/// A full cell of `material` at `deci_kelvin`.
pub(crate) fn hot(material: u8, deci_kelvin: u16) -> Cell {
    Cell { temperature: deci_kelvin, ..Cell::filled(material) }
}
