//! Generator 34 ore bodies. Each ore forms the way it does in rock: coal in
//! tilted sedimentary seams, iron in banded formations on old crust, copper
//! in zoned stocks around magmatic centres, tin in steep lodes along orogenic
//! belts, crystal in sealed geodes. All lengths are metres, so every voxel
//! scale shares the same deposits.
use crate::landscape::TectonicSample;
use crate::{BlockId, hash3, value_noise2, value_noise3};
use glam::Vec3;

/// Ore blocks the pre-34 generator scattered; gen 34 replaces them with host rock.
pub(crate) fn is_scattered_ore(b: BlockId) -> bool {
    matches!(b, BlockId::CoalOre | BlockId::IronOre | BlockId::CopperOre | BlockId::TinOre)
}

/// Deposit block at a solid voxel, or `None` to keep the host rock.
/// `rock` is the host the legacy stratigraphy chose.
pub(crate) fn deposit(
    seed: u32,
    tect: &TectonicSample,
    m: Vec3,
    below: f32,
    rock: BlockId,
    mineral_species: bool,
) -> Option<BlockId> {
    if rock == BlockId::Lava || below < 1.5 {
        return None;
    }
    // Cheap rejects first: the cell tests below cost a few hashes each.
    geode(seed, m, below, rock)
        .or_else(|| copper_stock(seed, tect, m, below).map(|ore| match (ore, mineral_species) {
            (BlockId::CopperOre, true) if below < 10.0 => BlockId::Malachite,
            (BlockId::CopperOre, true) => BlockId::Chalcopyrite,
            (BlockId::IronOre, true) => BlockId::Magnetite,
            _ => ore,
        }))
        .or_else(|| tin_lode(seed, tect, m, below).map(|ore| {
            if mineral_species { BlockId::Cassiterite } else { ore }
        }))
        .or_else(|| iron_bands(seed, tect, m, below).map(|ore| {
            if mineral_species { BlockId::Hematite } else { ore }
        }))
        .or_else(|| coal_seam(seed, tect, m, below))
}

/// Folded, tilted bedding coordinate shared by the sedimentary ores.
fn bed_coord(tect: &TectonicSample, m: Vec3) -> f32 {
    let dip = m.x * tect.boundary_normal[0] + m.z * tect.boundary_normal[1];
    m.y + (dip * 0.012).sin() * (6.0 + tect.boundary_proximity * 10.0)
        + tect.fold_wave * (4.0 + tect.orogeny * 12.0)
        + tect.fault_throw
}

/// Coal: thin, laterally continuous seams in young continental basins,
/// buried shallow enough not to have been cooked into graphite.
fn coal_seam(seed: u32, tect: &TectonicSample, m: Vec3, below: f32) -> Option<BlockId> {
    if tect.continentalness < 0.05 || tect.crust_age > 0.62 || tect.volcanism > 0.35 || !(3.0..60.0).contains(&below) {
        return None;
    }
    const PERIOD: f32 = 17.0;
    let bed = bed_coord(tect, m);
    let layer = (bed / PERIOD).floor();
    let phase = bed - layer * PERIOD;
    // Seam thickness 0.8-2.2 m, varying per layer and thinning at the edges.
    let thick = 0.5 + 1.0 * hash3(layer as i32, 3, 0, seed.wrapping_add(7001));
    if phase > thick {
        return None;
    }
    let basin = value_noise2(m.x / 140.0 + layer * 7.3, m.z / 140.0, seed.wrapping_add(7003));
    let edge = (basin - 0.52) * 4.0;
    (edge > 0.0 && phase < thick * edge.min(1.0)).then_some(BlockId::CoalOre)
}

/// Banded iron formation: alternating ore and chert bands on stable old crust.
fn iron_bands(seed: u32, tect: &TectonicSample, m: Vec3, below: f32) -> Option<BlockId> {
    if tect.crust_age < 0.42 || below < 4.0 {
        return None;
    }
    let bed = bed_coord(tect, m);
    let phase = (bed / 7.0).rem_euclid(1.0);
    if phase > 0.04 {
        return None;
    }
    let field = value_noise2(m.x / 170.0, m.z / 170.0, seed.wrapping_add(7011));
    // A basin of formation, with the richest bands near its middle.
    let richness = (field - 0.5) * 3.0 + (tect.crust_age - 0.45);
    let patch = hash3((m.x * 2.0).floor() as i32, (m.y * 2.0).floor() as i32, (m.z * 2.0).floor() as i32, seed.wrapping_add(7013));
    (richness > 0.0 && patch < 0.30 + richness.min(0.4)).then_some(BlockId::IronOre)
}

/// Porphyry stock: a vertical, zoned intrusion. Barren core, copper stockwork
/// shell, iron-sulphide halo. Cells are 96 m; the stock stays inside its cell
/// horizontally, but its taller y radius (r / 0.6) can reach the cell's y
/// plane, where wide stocks get a flat clip.
fn copper_stock(seed: u32, tect: &TectonicSample, m: Vec3, below: f32) -> Option<BlockId> {
    let magmatic = (tect.volcanism * 0.9 + tect.orogeny * 0.7 + tect.boundary_proximity * 0.3).min(1.0);
    if magmatic < 0.06 || below < 5.0 {
        return None;
    }
    const CELL: f32 = 96.0;
    let c = (m / CELL).floor();
    let (ix, iy, iz) = (c.x as i32, c.y as i32, c.z as i32);
    let s = seed.wrapping_add(7021);
    if hash3(ix, iy, iz, s) > 0.22 + magmatic * 0.5 {
        return None;
    }
    let pick = |k: i32| 0.28 + 0.44 * hash3(ix, iy, iz, s.wrapping_add(k as u32));
    let centre = (c + Vec3::new(pick(1), pick(2), pick(3))) * CELL;
    let radius = 13.0 + 9.0 * hash3(ix, iy, iz, s.wrapping_add(9));
    let off = m - centre;
    // Pipes are taller than wide.
    let d = (off.x * off.x + off.z * off.z + (off.y * 0.6) * (off.y * 0.6)).sqrt() / radius;
    if d > 1.0 {
        return None;
    }
    let veinlets = value_noise3(m.x * 0.55, m.y * 0.55, m.z * 0.55, s.wrapping_add(11));
    if d < 0.3 {
        None
    } else if d < 0.78 {
        (veinlets > 0.52).then_some(BlockId::CopperOre)
    } else {
        (veinlets > 0.74).then_some(BlockId::IronOre)
    }
}

/// Tin: steeply dipping sheet lodes parallel to the plate suture, patchy along strike.
fn tin_lode(seed: u32, tect: &TectonicSample, m: Vec3, below: f32) -> Option<BlockId> {
    if tect.crust_age < 0.25 || tect.orogeny + tect.boundary_proximity < 0.4 || below < 6.0 {
        return None;
    }
    let strike = [-tect.boundary_normal[1], tect.boundary_normal[0]];
    // Distance across the lodes, tilted by depth to give a dip.
    let across = m.x * tect.boundary_normal[0] + m.z * tect.boundary_normal[1] + m.y * 0.4;
    let warp = (value_noise3(m.x / 30.0, m.y / 30.0, m.z / 30.0, seed.wrapping_add(7031)) - 0.5) * 10.0;
    const SPACING: f32 = 64.0;
    let u = (across + warp).rem_euclid(SPACING);
    let dist = (u - SPACING * 0.5).abs();
    let width = 0.35 + 0.7 * value_noise3(m.x / 24.0, m.y / 24.0, m.z / 24.0, seed.wrapping_add(7033));
    if dist > width {
        return None;
    }
    let along = m.x * strike[0] + m.z * strike[1];
    let shoot = value_noise2(along / 60.0, m.y / 40.0, seed.wrapping_add(7035));
    let fleck = hash3((m.x * 2.0).floor() as i32, (m.y * 2.0).floor() as i32, (m.z * 2.0).floor() as i32, seed.wrapping_add(7037));
    (shoot > 0.56 && fleck > 0.3).then_some(BlockId::TinOre)
}

/// Geodes: a sealed hollow in basalt or limestone lined with crystal.
fn geode(seed: u32, m: Vec3, below: f32, rock: BlockId) -> Option<BlockId> {
    if below < 12.0 || !matches!(rock, BlockId::Basalt | BlockId::Limestone) {
        return None;
    }
    const CELL: f32 = 26.0;
    let c = (m / CELL).floor();
    let (ix, iy, iz) = (c.x as i32, c.y as i32, c.z as i32);
    let s = seed.wrapping_add(7041);
    if hash3(ix, iy, iz, s) > 0.07 {
        return None;
    }
    let radius = 1.7 + 1.9 * hash3(ix, iy, iz, s.wrapping_add(5));
    let j = |k: u32| 0.3 + 0.4 * hash3(ix, iy, iz, s.wrapping_add(k));
    let centre = (c + Vec3::new(j(1), j(2), j(3))) * CELL;
    let d = (m - centre).length() / radius;
    if d > 1.0 {
        None
    } else if d > 0.68 {
        Some(BlockId::Crystal)
    } else {
        Some(BlockId::Air)
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, WorldSampleConfig, WorldType};
    use engine_core::VoxelScalePreset;
    use glam::IVec3;

    fn census(version: u32, seed: u32, span: i32) -> ([u32; 5], u32, u32, u32) {
        let cfg = WorldSampleConfig {
            planet: None,
            generator_version: version,
            seed,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let (mut n, mut solid, mut clustered, mut ore) = ([0u32; 5], 0u32, 0u32, 0u32);
        let slot = |b: BlockId| match b {
            BlockId::CoalOre => Some(0),
            BlockId::IronOre => Some(1),
            BlockId::CopperOre => Some(2),
            BlockId::TinOre => Some(3),
            BlockId::Crystal => Some(4),
            _ => None,
        };
        for x in (-span..span).step_by(2) {
            for z in (-span..span).step_by(2) {
                let h = crate::sample_column(&cfg, x, z).height;
                for y in (h - 70).max(cfg.min_world_y() + 8)..h - 4 {
                    let b = crate::sample_block(&cfg, IVec3::new(x, y, z));
                    if b == BlockId::Air || b == BlockId::Water {
                        continue;
                    }
                    solid += 1;
                    if let Some(i) = slot(b) {
                        n[i] += 1;
                        ore += 1;
                        let nb = crate::sample_block(&cfg, IVec3::new(x + 1, y, z));
                        let nu = crate::sample_block(&cfg, IVec3::new(x, y + 1, z));
                        if slot(nb) == Some(i) || slot(nu) == Some(i) {
                            clustered += 1;
                        }
                    }
                }
            }
        }
        (n, solid, ore, clustered)
    }

    #[test]
    fn ore_is_rare_clustered_and_gated_by_version() {
        let (n, solid, ore, clustered) = census(34, 424242, 150);
        let pct = ore as f32 * 100.0 / solid as f32;
        assert!((0.2..6.0).contains(&pct), "ore share {pct:.2}%");
        assert!(clustered * 10 > ore * 6, "ore should form bodies, not flecks: {clustered}/{ore}");
        assert!(n[0] + n[2] > 0, "no coal or copper found: {n:?}");
        // Pre-34 worlds keep the scattered ores untouched.
        let (_, _, old_ore, _) = census(33, 424242, 150);
        assert_ne!(old_ore, ore);
    }

    #[test]
    #[ignore]
    fn ore_census() {
        for (v, seed) in [(33u32, 424242u32), (34, 424242), (34, 7), (34, 99)] {
            let (n, solid, ore, cl) = census(v, seed, 700);
            println!("v{v} s{seed}: solid={solid} ore={ore} ({:.3}%) clustered={cl} coal/iron/cu/sn/crystal={n:?}", ore as f32 * 100.0 / solid as f32);
        }
    }
}
