//! The Gloaming: a forest of giant trees under a sky stuck at dusk. Rolling
//! moss floor, still dark pools in the hollows, flared-root trunks with
//! layered canopies, some hollow at the base, and glowcap clusters that are
//! the forest's own light. Everything is in world metres.
//!
//! No historical world is a Gloaming realm, so this first layout needs no
//! generator gate. Any later change to it must gate on `generator_version`.
use crate::{hash3, value_noise2, BlockId, ColumnSample, WorldSampleConfig};
use glam::{IVec3, Vec2, Vec3};

/// Pools fill hollows below this height.
pub const POOL_M: f32 = 22.0;
/// Giant tree cell edge.
const TREE_CELL_M: f32 = 22.0;
/// No trunk stands within this of the origin: the arrival clearing.
const CLEARING_M: f32 = 14.0;

const GROUND: u32 = 0x474c_4f41;
const TREE: u32 = 0x4749_414e;
const CAP: u32 = 0x4341_5053;

/// Forest floor height in metres.
pub fn ground(seed: u32, x: f32, z: f32) -> f32 {
    let base = 30.0
        + 10.0 * (value_noise2(x / 120.0, z / 120.0, seed ^ GROUND) - 0.5) * 2.0
        + 4.0 * (value_noise2(x / 30.0, z / 30.0, seed ^ GROUND.rotate_left(5)) - 0.5) * 2.0
        + 1.2 * value_noise2(x / 7.0, z / 7.0, seed ^ GROUND.rotate_left(11));
    // The clearing is flattened towards a gentle rise.
    let c = (Vec2::new(x, z).length() / CLEARING_M).min(1.0);
    let flat = 31.0;
    flat + (base - flat) * (c * c * (3.0 - 2.0 * c))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Giant {
    pub trunk: Vec2,
    pub base: f32,
    pub radius: f32,
    pub height: f32,
    pub hollow: bool,
}

/// The giant of a tree cell, if one grows there.
pub fn giant(seed: u32, cell: (i32, i32)) -> Option<Giant> {
    let j = |k: i32| hash3(cell.0, k, cell.1, seed ^ TREE);
    if j(0) < 0.3 {
        return None;
    }
    let trunk = Vec2::new((cell.0 as f32 + 0.2 + 0.6 * j(1)) * TREE_CELL_M, (cell.1 as f32 + 0.2 + 0.6 * j(2)) * TREE_CELL_M);
    if trunk.length() < CLEARING_M + 4.0 {
        return None;
    }
    let base = ground(seed, trunk.x, trunk.y);
    if base < POOL_M + 0.5 {
        return None;
    }
    Some(Giant { trunk, base, radius: 1.6 + 1.6 * j(3), height: 34.0 + 24.0 * j(4), hollow: j(5) > 0.72 })
}

fn canopy_shape(height: f32, lobe: i32) -> (f32, f32) {
    (height * (0.62 + 0.15 * lobe as f32), height * (0.42 - 0.09 * lobe as f32))
}

/// Conservative upper bound for trees whose crowns can reach this block column.
pub fn tree_top_near(cfg: &WorldSampleConfig, lo: (i32, i32), hi: (i32, i32)) -> Option<i32> {
    let scale = cfg.voxel_meters();
    let min = Vec2::new(lo.0 as f32 + 0.5, lo.1 as f32 + 0.5) * scale;
    let max = Vec2::new(hi.0 as f32 + 0.5, hi.1 as f32 + 0.5) * scale;
    let first = (min / TREE_CELL_M).floor().as_ivec2() - 2;
    let last = (max / TREE_CELL_M).floor().as_ivec2() + 2;
    let mut top = None;
    for z in first.y..=last.y {
        for x in first.x..=last.x {
            let Some(g) = giant(cfg.seed, (x, z)) else { continue };
            // The lowest lobe is widest, with at most 3 m offset per axis.
            // At every giant height its reach also covers the roots and limbs.
            let reach = canopy_shape(g.height, 0).1 + 3.0;
            if (g.trunk - g.trunk.clamp(min, max)).abs().max_element() > reach {
                continue;
            }
            let (rise, width) = canopy_shape(g.height, 2);
            let upper = g.base + g.height.max(rise + width * 0.38);
            top = top.max(Some((upper / scale).ceil() as i32));
        }
    }
    top
}

fn tree_block(seed: u32, g: &Giant, m: Vec3) -> Option<BlockId> {
    let flat = Vec2::new(m.x, m.z);
    let d = flat.distance(g.trunk);
    let up = m.y - g.base;
    if up < -3.0 || up > g.height + 12.0 || d > g.height * 0.55 + 6.0 {
        return None;
    }
    // Trunk with a root flare; a hollow giant has a den with a gap facing out.
    if up < g.height {
        let flare = 1.0 + 1.6 * (-(up.max(0.0)) / 2.4).exp();
        let r = g.radius * flare;
        if d < r && up > -3.0 {
            if g.hollow && up >= 0.0 && up < 4.0 && d < r - 0.9 {
                return Some(BlockId::Air);
            }
            if g.hollow && up >= 0.0 && up < 3.0 {
                let toward = (flat - g.trunk).normalize_or_zero();
                let door = (g.trunk - Vec2::ZERO).normalize_or_zero() * -1.0;
                if toward.dot(door) > 0.9 {
                    return Some(BlockId::Air);
                }
            }
            return Some(BlockId::GloamingLog);
        }
    }
    // Canopy: three stacked lobes, widest at the bottom, each a flattened sphere.
    for k in 0..3 {
        let (rise, width) = canopy_shape(g.height, k);
        let centre = Vec3::new(g.trunk.x, g.base + rise, g.trunk.y)
            + Vec3::new(
                (hash3(k, 7, 0, seed ^ TREE ^ g.trunk.x.to_bits()) - 0.5) * 6.0,
                0.0,
                (hash3(k, 8, 0, seed ^ TREE ^ g.trunk.y.to_bits()) - 0.5) * 6.0,
            );
        let e = (m - centre) * Vec3::new(1.0 / width, 1.0 / (width * 0.38), 1.0 / width);
        if e.length_squared() < 1.0 {
            // Ragged edge: leaves thin out towards the rim.
            let rim = e.length_squared();
            if rim < 0.75 || hash3(m.x as i32, m.y as i32, m.z as i32, seed ^ TREE) > rim {
                return Some(BlockId::GloamingLeaves);
            }
        }
    }
    // A few great limbs out to the lowest lobe.
    if up > g.height * 0.45 && up < g.height * 0.66 {
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_2 + hash3(k, 9, 0, seed ^ TREE ^ g.trunk.x.to_bits()) * 1.2;
            let dir = Vec2::new(a.cos(), a.sin());
            let along = (flat - g.trunk).dot(dir);
            let off = (flat - g.trunk - dir * along).length();
            let rise = g.height * 0.45 + along * 0.5;
            if along > 0.0 && along < g.height * 0.3 && off < 0.9 && (up - rise).abs() < 0.9 {
                return Some(BlockId::GloamingLog);
            }
        }
    }
    None
}

pub fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    let vm = cfg.voxel_meters();
    let (mx, mz) = ((x as f32 + 0.5) * vm, (z as f32 + 0.5) * vm);
    let g = ground(cfg.seed, mx, mz);
    ColumnSample {
        deposit_m: 0.0,
        deposit_bed_surface: BlockId::Dirt,
        height: cfg.blocks_for_height(g.max(POOL_M)),
        surface: if g < POOL_M { BlockId::Water } else { BlockId::GloamingMoss },
        filler: BlockId::Dirt,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

/// The return gate: a 1 m block set into the clearing floor at the origin.
pub fn gate_at(seed: u32, m: Vec3) -> bool {
    if !(0.0..1.0).contains(&m.x) || !(0.0..1.0).contains(&m.z) {
        return false;
    }
    let top = ground(seed, 0.5, 0.5);
    (top - 1.0..top).contains(&m.y)
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3) -> BlockId {
    let vm = cfg.voxel_meters();
    let m = (p.as_vec3() + Vec3::splat(0.5)) * vm;
    let seed = cfg.seed;
    if gate_at(seed, m) {
        return BlockId::GloamingGate;
    }
    let (cx, cz) = ((m.x / TREE_CELL_M).floor() as i32, (m.z / TREE_CELL_M).floor() as i32);
    for dz in -2..=2 {
        for dx in -2..=2 {
            if let Some(g) = giant(seed, (cx + dx, cz + dz))
                && let Some(b) = tree_block(seed, &g, m)
            {
                return b;
            }
        }
    }
    let g = ground(seed, m.x, m.z);
    let depth = g - m.y;
    if depth > 0.0 {
        return if depth < 1.0 {
            if g < POOL_M + 0.4 { BlockId::Mud } else { BlockId::GloamingMoss }
        } else if depth < 4.0 {
            BlockId::Dirt
        } else {
            BlockId::Stone
        };
    }
    if m.y < POOL_M {
        return BlockId::Water;
    }
    // Glowcaps: clusters on the moss, the forest's own light.
    if depth > -vm * 1.01 && g > POOL_M + 0.4 {
        let patch = value_noise2(m.x / 11.0, m.z / 11.0, seed ^ CAP);
        let cell = (Vec2::new(m.x, m.z) / 1.3).floor();
        if patch > 0.74 && hash3(cell.x as i32, 0, cell.y as i32, seed ^ CAP) > 0.55 {
            return BlockId::AmberCap;
        }
    }
    BlockId::Air
}

/// Feet position in voxels: on the clearing floor beside the gate.
pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    let at = Vec3::new(3.5, 0.0, 0.5);
    let vm = cfg.voxel_meters();
    let centre = ((at / vm).floor() + Vec3::splat(0.5)) * vm;
    let top = (ground(cfg.seed, centre.x, centre.z) / vm - 0.5).ceil();
    Vec3::new(at.x / vm, top + 0.02 / vm, at.z / vm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(scale: crate::VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: crate::CURRENT_GENERATOR_VERSION,
            seed: 7,
            world_type: crate::WorldType::Realm(crate::RealmKind::Gloaming),
            voxel_scale: scale,
            planet: None,
        }
    }

    /// Arrival stands on moss in the clearing beside the return gate.
    #[test]
    fn arrival_is_in_the_clearing() {
        for scale in [crate::VoxelScalePreset::OneMeter, crate::VoxelScalePreset::QuarterMeter] {
            let c = cfg(scale);
            let feet = spawn(&c);
            let cell = feet.floor().as_ivec3();
            for dy in 0..=(2.0 / c.voxel_meters()).ceil() as i32 {
                assert!(!block(&c, cell + IVec3::Y * dy).is_solid(), "headroom at {scale:?}");
            }
            assert!(block(&c, cell - IVec3::Y).is_solid(), "footing at {scale:?}");
            let gate = (Vec3::new(0.5, ground(c.seed, 0.5, 0.5) - 0.5, 0.5) / c.voxel_meters()).floor().as_ivec3();
            assert_eq!(block(&c, gate), BlockId::GloamingGate);
        }
    }

    /// The woods are dense: most tree cells hold a giant.
    #[test]
    fn giants_fill_the_forest() {
        let n = (-8..8).flat_map(|z| (-8..8).map(move |x| (x, z))).filter(|&c| giant(7, c).is_some()).count();
        assert!((110..230).contains(&n), "{n} of 256 cells hold a giant");
    }
}
