//! Inhabited wings, readable records and long return circuits.
use super::{Complex, Job, Kind, Room};
use crate::BlockId as B;
use glam::Vec3;

pub const VERSION: u32 = 88;

pub fn purpose(job: Job, leaf: bool, hops: usize, index: usize) -> Kind {
    use Kind::*;
    let programme: &[Kind] = match job {
        Job::Catacombs | Job::Sanctum => &[Ossuary, Shrine, Crypt, Refectory],
        Job::Undercroft => &[Sluice, Cistern, Store, Barracks],
        Job::ForgeHalls | Job::Workshop => &[Forge, Foundry, Armory, Store],
        Job::Quarantine => &[Infirmary, Cells, Garden, Barracks],
        Job::Orrery => &[Library, Orrery, Scriptorium, Store],
        Job::Warren => &[Refectory, Barracks, Store, Garden],
        Job::HollowFloor => &[Chasm, Library, Shrine, Store],
    };
    if leaf {
        programme[2 + index % 2]
    } else {
        programme[(hops.saturating_sub(2) / 2) % 2]
    }
}

/// Off the central aisle and outside every corridor throat.
pub fn tablet(c: &Complex, r: &Room) -> Option<Vec3> {
    if c.version < VERSION || r.kind != Kind::Scriptorium || r.sealed {
        return None;
    }
    let (x, z) = super::treasure::cache_spot(c, r);
    Some(Vec3::new(r.x0 + x, r.floor, r.z0 + z))
}

pub fn fixture(c: &Complex, r: &Room, p: Vec3) -> Option<B> {
    let d = p - tablet(c, r)?;
    (d.x.abs() <= 0.6 && d.z.abs() <= 0.6 && (0.0..1.8).contains(&d.y)).then_some(if d.y < 0.5 {
        c.pal.trim
    } else {
        B::Bookcase
    })
}

/// Working infrastructure occupies quadrants; the door-to-door cross stays dry.
/// These are real water and soil cells, usable by the material simulation.
pub fn works(c: &Complex, r: &Room, x: f32, z: f32, y: f32, v: f32) -> Option<B> {
    if c.version < 90 || y < 0.0 || r.sealed { return None; }
    let (w, d) = (r.w(), r.d());
    if (x - w * 0.5).abs() < 1.5 || (z - d * 0.5).abs() < 1.5 { return None; }
    let cx = if x < w * 0.5 { w * 0.25 } else { w * 0.75 };
    let cz = if z < d * 0.5 { d * 0.25 } else { d * 0.75 };
    let (dx, dz) = ((x - cx).abs(), (z - cz).abs());
    match r.kind {
        Kind::Cistern | Kind::Garden if dx < w * 0.16 && dz < d * 0.16 => {
            let rim = dx >= w * 0.16 - v.max(0.75) || dz >= d * 0.16 - v.max(0.75);
            if y < v { return Some(c.pal.floor); }
            if y < v + 1.0 {
                return Some(if rim { c.pal.trim } else if r.kind == Kind::Cistern { B::Water } else { B::FungalSoil });
            }
            if y < 3.0 { return Some(B::Air); }
        }
        // Paired rows of vaulted supports frame side aisles in the large halls.
        Kind::Foundry | Kind::Infirmary | Kind::Barracks if dx < v.max(0.75) && dz < v.max(0.75) => {
            if y < r.h - 1.0 { return Some(c.pal.trim); }
        }
        _ => {}
    }
    None
}
