//! Generator 126: planet cave zones, hazards and ore lodes worth hunting.
//!
//! Zones are picked per cell from depth, a 3D zone field and the surface
//! climate above. Only cells next to a carved cave change, so solid rock away
//! from caves costs one extra lode lookup.
use crate::{BlockId, ColumnSample, WorldSampleConfig, cave_carves, hash3, value_noise3};
use glam::IVec3;

pub const VERSION: u32 = 126;

/// Lode cell edge in metres; one lode at most per cell.
const LODE_CELL_M: f32 = 14.0;
/// Surface tells mark lodes whose top lies within this many metres of the surface.
const TELL_DEPTH_M: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Plain,
    Fungal,
    Frozen,
    /// Deep basalt galleries with lava pools on the floor.
    Magma,
    Crystal,
}

fn depth_m(cfg: &WorldSampleConfig, p: IVec3, column: &ColumnSample) -> f32 {
    (column.height - p.y) as f32 * cfg.voxel_meters()
}

pub fn zone(cfg: &WorldSampleConfig, p: IVec3, column: &ColumnSample) -> Zone {
    let depth = depth_m(cfg, p, column);
    let m = p.as_vec3() * cfg.voxel_meters();
    let n = value_noise3(m.x / 110.0, m.y / 70.0, m.z / 110.0, cfg.seed.wrapping_add(12_601));
    let cold = matches!(column.surface, BlockId::SnowGrass | BlockId::Ice | BlockId::SnowLayer);
    let damp = matches!(column.surface, BlockId::Grass | BlockId::Mud | BlockId::Clay | BlockId::FungalSoil);
    match () {
        _ if depth > 70.0 && n > 0.60 => Zone::Magma,
        _ if depth > 40.0 && n < 0.24 => Zone::Crystal,
        _ if cold && depth < 70.0 => Zone::Frozen,
        _ if damp && depth > 8.0 && (0.40..0.58).contains(&n) => Zone::Fungal,
        _ => Zone::Plain,
    }
}

/// Ore for a lode at this depth: deeper lodes pay more.
fn lode_ore(depth: f32, pick: f32) -> BlockId {
    match depth {
        d if d < 28.0 => if pick < 0.7 { BlockId::CoalOre } else { BlockId::CopperOre },
        d if d < 60.0 => if pick < 0.5 { BlockId::CopperOre } else { BlockId::TinOre },
        d if d < 100.0 => if pick < 0.8 { BlockId::IronOre } else { BlockId::GoldQuartz },
        _ => if pick < 0.55 { BlockId::GoldQuartz } else { BlockId::IronOre },
    }
}

/// Lode in the cell holding `m`: centre in metres, radius and a pick value.
fn lode_in_cell(seed: u32, cell: IVec3) -> Option<(glam::Vec3, f32, f32)> {
    let h = |salt: u32| hash3(cell.x, cell.y, cell.z, seed.wrapping_add(salt));
    if h(12_611) > 0.07 {
        return None;
    }
    // Margin keeps the ball inside its own cell, so one cell lookup suffices.
    let inner = LODE_CELL_M - 8.0;
    let centre = cell.as_vec3() * LODE_CELL_M
        + glam::Vec3::new(h(12_613), h(12_617), h(12_619)) * inner
        + glam::Vec3::splat(4.0);
    Some((centre, 2.0 + h(12_623) * 2.0, h(12_629)))
}

/// Ore of a lode covering `p`, if any.
pub fn lode(cfg: &WorldSampleConfig, p: IVec3, column: &ColumnSample) -> Option<BlockId> {
    let m = (p.as_vec3() + glam::Vec3::splat(0.5)) * cfg.voxel_meters();
    let cell = (m / LODE_CELL_M).floor().as_ivec3();
    let (centre, radius, pick) = lode_in_cell(cfg.seed, cell)?;
    // Ragged edge so lodes read as bodies, not spheres.
    let rag = value_noise3(m.x / 3.0, m.y / 3.0, m.z / 3.0, cfg.seed.wrapping_add(12_631)) * 1.4;
    let depth = depth_m(cfg, p, column);
    (m.distance(centre) < radius + rag - 0.7 && depth > 4.0).then(|| lode_ore(depth, pick))
}

/// Surface cover that marks a shallow lode below this column: rust clay over
/// iron and gold, loose gravel over the rest.
pub fn surface_tell(cfg: &WorldSampleConfig, x: i32, z: i32, column: &ColumnSample) -> Option<BlockId> {
    if !matches!(column.surface, BlockId::Grass | BlockId::Dirt | BlockId::Sand | BlockId::SnowGrass) {
        return None;
    }
    let vm = cfg.voxel_meters();
    let top = column.height as f32 * vm;
    let (mx, mz) = ((x as f32 + 0.5) * vm, (z as f32 + 0.5) * vm);
    let (cx, cz) = ((mx / LODE_CELL_M).floor() as i32, (mz / LODE_CELL_M).floor() as i32);
    let lowest = ((top - TELL_DEPTH_M - 8.0) / LODE_CELL_M).floor() as i32;
    for cy in lowest..=(top / LODE_CELL_M).floor() as i32 {
        let Some((centre, radius, pick)) = lode_in_cell(cfg.seed, IVec3::new(cx, cy, cz)) else { continue };
        let depth = top - centre.y;
        let reach = radius + 3.0;
        let flat = ((centre.x - mx).powi(2) + (centre.z - mz).powi(2)).sqrt();
        if depth > 4.0 && depth - radius < TELL_DEPTH_M && flat < reach {
            return Some(match lode_ore(depth, pick) {
                BlockId::IronOre | BlockId::GoldQuartz => BlockId::Clay,
                _ => BlockId::Gravel,
            });
        }
    }
    None
}

/// Carved, dry cave cell: lava pools, caps, snow, or plain air.
pub fn cave_air(cfg: &WorldSampleConfig, p: IVec3, column: &ColumnSample) -> BlockId {
    let floor = !cave_carves(cfg, p - IVec3::Y, column);
    if !floor {
        return BlockId::Air;
    }
    let h = hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(12_641));
    let m = p.as_vec3() * cfg.voxel_meters();
    match zone(cfg, p, column) {
        Zone::Magma => {
            // Broad pools rather than single hot cells on a bumpy floor.
            let pool = value_noise3(m.x / 16.0, 0.0, m.z / 16.0, cfg.seed.wrapping_add(12_643));
            if pool > 0.55 { BlockId::Lava } else { BlockId::Air }
        }
        Zone::Fungal if h > 0.80 => [BlockId::GlowCap, BlockId::RedCap, BlockId::AmberCap][(h * 1000.0) as usize % 3],
        Zone::Frozen if h > 0.45 => BlockId::SnowLayer,
        _ => BlockId::Air,
    }
}

/// Solid cell below the soil: zone lining where it borders a cave, lodes elsewhere.
pub fn rock(cfg: &WorldSampleConfig, p: IVec3, column: &ColumnSample, host: BlockId) -> BlockId {
    if let Some(ore) = lode(cfg, p, column) {
        return ore;
    }
    let ceiling = cave_carves(cfg, p - IVec3::Y, column);
    let floor = !ceiling && cave_carves(cfg, p + IVec3::Y, column);
    if !ceiling && !floor {
        return host;
    }
    let m = p.as_vec3() * cfg.voxel_meters();
    // Metre-scale fields group linings into patches; a per-voxel hash reads as speckle.
    let patch = |scale: f32, salt: u32| value_noise3(m.x / scale, m.y / scale, m.z / scale, cfg.seed.wrapping_add(salt));
    match zone(cfg, p, column) {
        Zone::Magma if floor && patch(4.0, 12_655) > 0.55 => BlockId::Ash,
        Zone::Magma => if patch(2.5, 12_657) > 0.80 { BlockId::GoldQuartz } else { BlockId::Basalt },
        Zone::Crystal if patch(2.0, 12_659) > 0.58 => BlockId::Crystal,
        Zone::Crystal if patch(3.0, 12_661) > 0.78 => BlockId::GoldQuartz,
        Zone::Fungal if floor => if patch(3.0, 12_663) < 0.6 { BlockId::FungalSoil } else { BlockId::GroveMoss },
        Zone::Frozen => BlockId::Ice,
        Zone::Plain if ceiling => {
            // Loose lenses drop when the rock beside them is mined.
            let lens = value_noise3(m.x / 12.0, m.y / 6.0, m.z / 12.0, cfg.seed.wrapping_add(12_653));
            if lens > 0.72 { BlockId::Gravel } else { host }
        }
        _ => host,
    }
}

#[cfg(test)]
#[path = "deep_tests.rs"]
mod tests;
