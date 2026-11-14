//! Human-scale rooms with recognisable uses. Dimensions are metres, never
//! counts of voxels. Door throats, centre routes and gallery stairs win first.
use super::{Job, Kind, Room};
use crate::BlockId as B;

pub(super) fn programme(job: Job) -> &'static [Kind] {
    use Kind::*;
    match job {
        Job::Catacombs => &[Ossuary, Scriptorium, Crypt, Shrine, Armory],
        Job::Undercroft => &[Cistern, Refectory, Sluice, Store, Infirmary],
        Job::ForgeHalls => &[Forge, Armory, Foundry, Refectory, Scriptorium],
        Job::Warren => &[Garden, Infirmary, Store, Scriptorium, Chasm],
        Job::Quarantine => &[Cells, Infirmary, Barracks, Armory, Scriptorium],
        Job::Orrery => &[Library, Scriptorium, Orrery, Refectory, Store],
        Job::HollowFloor => &[Chasm, Library, Armory, Store, Shrine],
        Job::Sanctum => &[Library, Shrine, Scriptorium, Refectory, Crypt],
        Job::Workshop => &[Forge, Store, Cells, Cistern, Sluice],
    }
}

/// A three-metre case has the same footprint at every supported scale.
/// Coarse voxels retain the bookcase material, not just a solid plank frame.
/// Fine voxels resolve uprights. Shelf divisions stay in the material so
/// half-metre timber slabs cannot crop the lower half of every painted book.
fn bookcase(x: f32, z: f32, y: f32, width: f32, v: f32) -> Option<B> {
    if x < 0.0 || x >= width || z < 0.0 || z >= v.max(0.75) || !(0.0..3.0).contains(&y) {
        return None;
    }
    let frame = v < 0.75 && (x < v || x >= width - v);
    Some(if frame { B::Planks } else { B::Bookcase })
}

fn table(x: f32, z: f32, y: f32, length: f32, v: f32) -> Option<B> {
    if x.abs() >= length * 0.5 || z.abs() >= 0.7f32.max(v) || y < 0.0 { return None; }
    let top = v.max(1.0);
    if y >= top - v && y < top { return Some(B::Planks); }
    if y < top && x.abs() > length * 0.5 - v && z.abs() > 0.7f32.max(v) - v {
        return Some(B::Log);
    }
    None
}

pub(super) fn furnish(r: &Room, rx: f32, rz: f32, y: f32, v: f32, dwall: f32, version: u32) -> Option<B> {
    if dwall < 1.0 || y < 0.0 { return None; }
    let (a, b, len, wid) = if r.w() >= r.d() {
        (rx, rz, r.w(), r.d())
    } else {
        (rz, rx, r.d(), r.w())
    };
    // Four quadrants around the unblocked centre cross; a second two-metre
    // cross-aisle splits long rows so shelves never form a continuous maze.
    let end = a.min(len - a);
    let side = b.min(wid - b);
    let variant = r.salt % 3;
    match r.kind {
        Kind::Library | Kind::Scriptorium => {
            let wall_row = (1.0..1.0 + v.max(0.75)).contains(&side);
            let stack_row = r.kind == Kind::Library && variant != 1
                && (4.0..4.0 + v.max(0.75)).contains(&side) && wid >= 16.0;
            if wall_row || stack_row {
                let segment = (a - 2.0).rem_euclid(5.0);
                let z = side - if wall_row { 1.0 } else { 4.0 };
                if end >= 2.0
                    && let Some(block) = bookcase(segment, z, y, 3.0, v)
                {
                    return Some(block);
                }
            }
            // Reading desks in the gaps; the scriptorium uses paired long
            // benches, while library variants favour small reading carrels.
            let desk_a = if a < len * 0.5 { len * 0.25 } else { len * 0.75 };
            let desk_b = if b < wid * 0.5 { wid * 0.25 } else { wid * 0.75 };
            let length = if r.kind == Kind::Scriptorium { 3.0 } else { 2.0 };
            if let Some(block) = table(a - desk_a, b - desk_b, y, length, v) { return Some(block); }
            // Seats face the desk from the outer side of each quadrant.
            // Leave the shelving access strip and central cross to circulation.
            if version >= 86 && side >= 2.5 {
                let seat_b = desk_b + if b < wid * 0.5 { -1.7 } else { 1.7 };
                let half = v.max(0.8) * 0.5;
                if (a - desk_a).abs() < half && (b - seat_b).abs() < half && y < v.max(0.6) {
                    return Some(B::Planks);
                }
            }
            if (a - desk_a).abs() < v * 0.5 && (b - desk_b).abs() < v * 0.5
                && (v.max(1.0)..v.max(1.0) + v).contains(&y) {
                return Some(B::Torch);
            }
        }
        Kind::Refectory => {
            // Service cupboards bookend the dining quadrants; their open
            // central gap keeps the end doors readable from the tables.
            if version >= 86 && (1.0..2.5).contains(&end) && side >= 3.0 && y < v.max(1.0) {
                return Some(if y >= v.max(1.0) - v { B::Planks } else { B::SalvageCrate });
            }
            let centre = if a < len * 0.5 { len * 0.25 } else { len * 0.75 };
            let row = if b < wid * 0.5 { wid * 0.25 } else { wid * 0.75 };
            if let Some(block) = table(a - centre, b - row, y, 4.0, v) { return Some(block); }
            if (a - centre).abs() < 2.0 && ((b - row).abs() - 1.8).abs() < v.max(0.5) * 0.5 && y < v.max(0.6) {
                return Some(B::Planks);
            }
            if end < 2.0 && side < 3.0 && y < 2.0 { return Some(B::SalvageCrate); }
        }
        Kind::Infirmary => {
            let bay = (a - 2.0).rem_euclid(4.0);
            if side < 3.5 && bay < 1.5 && end >= 2.0 {
                if y < v.max(0.6) { return Some(B::Planks); }
                if side < 1.0 + v && y < v.max(0.6) + v { return Some(B::GlazedTiles); }
            }
            if end < 2.0 && side < 3.0 && y < 2.0 { return Some(B::Bookcase); }
        }
        Kind::Armory => {
            let bay = (a - 2.0).rem_euclid(4.0);
            if side < 2.0 && end >= 2.0 && bay < 2.0 && y < 2.5 {
                if y < v || y >= 2.5 - v { return Some(B::Planks); }
                return Some(if bay < v { B::Log } else { B::IronOre });
            }
            if end < 3.0 && side > 3.0 && side < 4.5 && y < v.max(1.0) {
                return Some(B::Workbench);
            }
        }
        _ => {}
    }
    None
}
