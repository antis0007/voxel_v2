//! Generator 122: the planet's spawn face is a solved drainage continent.
//!
//! The geo solve (plates, stream power, climate, glacial carving) owns height,
//! rivers and lakes on the atlas face that holds the origin, and the hills
//! column pipeline classifies it, so biomes, strata and woodland follow the
//! same drainage a river follows. The solve rings its face with ocean; a band
//! inside the face edge blends into the noise shell beyond it.
use crate::{BlockId, ColumnSample, WorldSampleConfig, WorldType};
use engine_core::cube_sphere::atlas;

pub const VERSION: u32 = 122;

/// Width of the band inside the face edge that blends into the noise shell.
const BLEND_M: f32 = 1500.0;

/// Distance in metres from a world column to the solved face edge, if inside it.
fn inside_m(cfg: &WorldSampleConfig, x: i32, z: i32) -> Option<f32> {
    if cfg.world_type != WorldType::Planet || cfg.generator_version < VERSION {
        return None;
    }
    let (wx, wz) = (cfg.world_x(x), cfg.world_z(z));
    let half = atlas::HALF_M as f32;
    let edge = (half - wx.abs()).min(half - wz.abs());
    (edge > 0.0).then_some(edge)
}

/// Whether the solved face owns this column's blocks.
pub fn covers(cfg: &WorldSampleConfig, x: i32, z: i32) -> bool {
    if crate::planetary::active(cfg) { return false; }
    inside_m(cfg, x, z).is_some_and(|e| e > BLEND_M * 0.5)
}

/// The solved column, eased toward `shell` across the face-edge band.
pub fn column(cfg: &WorldSampleConfig, x: i32, z: i32, shell: impl FnOnce() -> ColumnSample) -> Option<ColumnSample> {
    let edge = inside_m(cfg, x, z)?;
    let solved = crate::landscape::column(cfg, x, z);
    Some(blend_column(edge, solved, shell))
}

/// Shared by walked columns and orbital cover; ownership's midpoint is not a
/// height transition. Both paths must reach the shell at the actual face edge.
fn blend_column(edge: f32, solved: ColumnSample, shell: impl FnOnce() -> ColumnSample) -> ColumnSample {
    if edge >= BLEND_M {
        return solved;
    }
    let outer = shell();
    let t = edge / BLEND_M;
    let t = t * t * (3.0 - 2.0 * t);
    let height = (outer.height as f32 + (solved.height - outer.height) as f32 * t).round() as i32;
    let base = if t >= 0.5 { solved } else { outer };
    ColumnSample { height, ..base }
}

/// Coarse column for far geometry over the solved face, or `None` off the face.
/// Exact columns build a detail tile per sample; far cells take the filtered
/// landscape through the same material policy, so cover matches walked ground.
pub fn cover_column(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> Option<ColumnSample> {
    if crate::planetary::active(cfg) { return Some(crate::planetary::column(cfg, x, z, footprint)); }
    let edge = inside_m(cfg, x, z)?;
    let s = crate::landscape::preview_landscape_lod(cfg, x, z, footprint);
    let solved = crate::landscape::far_column(cfg, x, z, s);
    Some(blend_column(edge, solved, || crate::planet::shell_column(cfg, x, z)))
}

/// Ground label for a planet column the hills biome map does not describe:
/// off the solved face, or any planet before it. `None` defers to the biome map.
pub fn planet_label(cfg: &WorldSampleConfig, x: i32, z: i32) -> Option<&'static str> {
    if cfg.world_type != WorldType::Planet || covers(cfg, x, z) {
        return None;
    }
    let c = crate::sample_column(cfg, x, z);
    if c.height < cfg.water_level() {
        return Some("Ocean");
    }
    Some(match c.surface {
        BlockId::Ice => "Ice sheet",
        BlockId::SnowGrass if c.vegetation_density > 0.3 => "Taiga",
        BlockId::SnowGrass => "Tundra",
        BlockId::Sand if c.height <= cfg.water_level() + cfg.blocks_for_span(3.0) => "Coast",
        BlockId::Sand | BlockId::Sandstone => "Desert",
        BlockId::Mud | BlockId::Clay => "Wetlands",
        BlockId::Basalt => "Volcanic upland",
        BlockId::Gravel | BlockId::Stone => "Rocky upland",
        _ if c.vegetation_density > 0.6 => "Forest",
        _ if c.vegetation_density > 0.2 => "Woodland",
        _ => "Grassland",
    })
}

/// Whether atlas face 0 (the one holding the origin) is solved for this world.
pub fn solved_face(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Planet && cfg.generator_version >= VERSION && !crate::planetary::active(cfg)
}

#[cfg(test)]
#[path = "basin_tests.rs"]
mod tests;
