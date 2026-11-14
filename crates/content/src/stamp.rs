//! Multi-voxel plant stamps for voxels smaller than a metre.
//!
//! A plant block is authored inside one cell, so at 0.25 m voxels a wheat
//! plant stood 22 cm tall. A stamp keeps the plant at its authored metre
//! size: generation fills a box of cells with the plant's block, the root
//! cell (lowest, at the box's minimum x and z corner) draws the whole plant
//! enlarged to the box, and the other cells draw nothing but still occupy
//! space, so the plant can be aimed at and broken anywhere in its volume.
//!
//! A stamp is recognised from its neighbours alone: a plant cell with the
//! same block above or below it. A single cell, as a player or a farm plot
//! places it, keeps its one-cell shape.
use crate::BlockId as B;
use glam::IVec3;

/// Voxels below this size use stamps.
const STAMP_BELOW_METRES: f32 = 0.75;

/// Ground plants that form stamps. Kelp, cactus, vines and bamboo stack by
/// their own rules and stay out.
pub fn stampable(b: B) -> bool {
    matches!(b, B::SeaOats | B::Cattail | B::Agave | B::Lavender | B::Orchid | B::BerryBush | B::SweetBerries
        | B::Cabbage | B::Potato | B::Onion | B::Tomato | B::WheatCrop | B::CarrotCrop | B::BeetCrop)
}

/// Cells per metre for a voxel size, or 1 when stamps are not used.
pub fn cells_per_metre(metres: f32) -> i32 {
    if metres >= STAMP_BELOW_METRES { 1 } else { (1.0 / metres).round().max(1.0) as i32 }
}

/// Width of a stamp in cells. One cell stays free between neighbours in a
/// metre lattice, so every stamp keeps its own root.
pub fn footprint(metres: f32) -> i32 {
    (cells_per_metre(metres) - 1).max(1)
}

/// Authored height of the plant rooted at `root`, in metres.
pub fn height_metres(b: B, root: IVec3) -> f32 {
    let top = if let Some(parts) = crate::flora::parts(b, root) {
        parts.iter().map(|part| part.hi.y.max(part.leaf.map_or(0.0, |l| l.iter().fold(0.0f32, |m, v| m.max(v.y)))))
            .fold(0.0f32, f32::max)
    } else {
        crate::thin::boxes(b, IVec3::ZERO).iter().map(|(_, hi)| hi.y).fold(0.0f32, f32::max)
    };
    top.clamp(0.25, 2.0)
}

/// Height of the stamp in cells: at least two, so it reads as a stamp.
pub fn height_cells(b: B, root: IVec3, metres: f32) -> i32 {
    let n = cells_per_metre(metres);
    ((height_metres(b, root) * n as f32).ceil() as i32).clamp(2, 2 * n)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Role {
    /// One-cell plant drawn at cell size.
    Lone,
    /// Draws the whole plant, enlarged by this factor about the cell's minimum corner.
    Root(f32),
    /// Occupies space; the root draws it.
    Member,
}

/// How the plant cell `b` at `p` is drawn, from its neighbours.
pub fn role(b: B, p: IVec3, metres: f32, get: impl Fn(IVec3) -> B) -> Role {
    if metres >= STAMP_BELOW_METRES || !stampable(b) {
        return Role::Lone;
    }
    if get(p - IVec3::Y) == b {
        return Role::Member;
    }
    if get(p + IVec3::Y) != b {
        return Role::Lone;
    }
    // A stacked neighbour toward -x or -z is the same stamp's base row.
    let base = |d: IVec3| get(p + d) == b && get(p + d + IVec3::Y) == b && get(p + d - IVec3::Y) != b;
    if base(IVec3::NEG_X) || base(IVec3::NEG_Z) {
        return Role::Member;
    }
    Role::Root(cells_per_metre(metres) as f32)
}

/// Every cell of the stamp that contains `p`, `p` included; just `p` for a lone cell.
pub fn cells(b: B, p: IVec3, metres: f32, get: impl Fn(IVec3) -> B) -> Vec<IVec3> {
    let mut out = vec![p];
    if metres >= STAMP_BELOW_METRES || !stampable(b) || (get(p + IVec3::Y) != b && get(p - IVec3::Y) != b) {
        return out;
    }
    let n = cells_per_metre(metres);
    let reach = IVec3::new(footprint(metres) - 1, 2 * n - 1, footprint(metres) - 1);
    let mut next = 0;
    while next < out.len() {
        let at = out[next];
        next += 1;
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
            let q = at + d;
            if (q - p).abs().cmple(reach).all() && get(q) == b && !out.contains(&q) {
                out.push(q);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(root: IVec3, metres: f32) -> Vec<IVec3> {
        let (f, h) = (footprint(metres), height_cells(B::WheatCrop, root, metres));
        let mut cells = Vec::new();
        for y in 0..h { for z in 0..f { for x in 0..f { cells.push(root + IVec3::new(x, y, z)); } } }
        cells
    }

    #[test]
    fn metre_voxels_and_single_cells_keep_their_shape() {
        assert_eq!(cells_per_metre(1.0), 1);
        assert_eq!((cells_per_metre(0.5), footprint(0.5)), (2, 1));
        assert_eq!((cells_per_metre(0.25), footprint(0.25)), (4, 3));
        let lone = |q: IVec3| if q == IVec3::ZERO { B::WheatCrop } else { B::Air };
        assert_eq!(role(B::WheatCrop, IVec3::ZERO, 0.25, lone), Role::Lone);
        let column = |q: IVec3| if q.x == 0 && q.z == 0 && (0..2).contains(&q.y) { B::WheatCrop } else { B::Air };
        assert_eq!(role(B::WheatCrop, IVec3::ZERO, 1.0, column), Role::Lone);
        // Kelp stacks by its own rule.
        let kelp = |q: IVec3| if q.x == 0 && q.z == 0 { B::Kelp } else { B::Air };
        assert_eq!(role(B::Kelp, IVec3::ZERO, 0.25, kelp), Role::Lone);
    }

    #[test]
    fn each_stamp_has_one_root_even_beside_a_neighbour() {
        for metres in [0.5, 0.25] {
            let n = cells_per_metre(metres);
            let mut all = Vec::new();
            for (lx, lz) in [(0, 0), (1, 0), (0, 1), (-1, -1)] {
                all.extend(stamp(IVec3::new(lx * n, 5, lz * n), metres));
            }
            let get = |q: IVec3| if all.contains(&q) { B::WheatCrop } else { B::Air };
            let roots: Vec<_> = all.iter().filter(|&&q| matches!(role(B::WheatCrop, q, metres, get), Role::Root(_))).collect();
            assert_eq!(roots.len(), 4, "{metres}: {roots:?}");
            assert!(all.iter().all(|&q| role(B::WheatCrop, q, metres, get) != Role::Lone));
            // Breaking any cell finds exactly its own stamp.
            let own = stamp(IVec3::new(n, 5, 0), metres);
            for &q in &own {
                let mut found = cells(B::WheatCrop, q, metres, get);
                found.sort_by_key(|c| (c.y, c.z, c.x));
                assert_eq!(found, own, "{metres} from {q}");
            }
        }
    }

    #[test]
    fn every_stampable_plant_has_a_height() {
        for &b in crate::ALL_BLOCK_IDS.iter().filter(|&&b| stampable(b)) {
            assert!(crate::thin::is_thin(b), "{b:?} must be a thin plant");
            let h = height_cells(b, IVec3::new(3, 9, -4), 0.25);
            assert!((2..=8).contains(&h), "{b:?} {h}");
        }
    }
}
